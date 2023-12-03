use reqwest::blocking::Client;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use serde::{Serialize, Deserialize};
use vector_node::prelude::*;
use openai_api::prelude::*;
use std::sync::{Arc, Mutex};
use pyo3::prelude::*;


lazy_static::lazy_static! {
    static ref PARENT_NODE: MutexWrapper<Node> = Node::new(0, Vec::<f64>::new(), String::new());
    static ref DB_PATH: Arc<Mutex<String>> = Arc::new(Mutex::new("./serialized_vector_db.json".to_string()));
}

#[derive(Debug,Serialize, Deserialize)]
pub enum QueryState {
    Added,
    Searched(Vec<(f64, String, u32)>),
    AddSearch(Vec<(f64, String, u32)>),
    ParseFailed,
    DidNothing,
}


#[derive(Debug,Serialize, Deserialize)]
pub struct SearchResult{
    cos_sim: f64,
    url: String,
    search_tally: u32
}

#[derive(Clone, Debug,Serialize, Deserialize)]
pub struct ApiResponse {
    state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<Vec<(f64, String, u32)>>
}

impl ApiResponse {
    pub fn from(state: QueryState) -> ApiResponse {
        let mut content: Option<Vec<(f64, String, u32)>> = None;
        let state = match state{
            QueryState::Added => {"Add response was sucessful"},
            QueryState::Searched(search_content) => {
                content = Some(search_content);
                "Search response was sucessful"
            },
            QueryState::AddSearch(search_content) => {
                content = Some(search_content);
                "AddSearch response was sucessful"
            },
            QueryState::ParseFailed => {"Parsing request failed"},
            QueryState::DidNothing => {"No Add or Search request found, what are you trying to do?"},
        };
        ApiResponse { state: state.to_owned(), content}
    }

    pub fn send(&self, mut stream: TcpStream) {

        let response_body = serde_json::to_string(&self).unwrap();

        let response = format!(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
            response_body.len(),
            response_body
        );

        stream.write_all(response.as_bytes()).expect("Failed to write to stream");
    }

}

#[derive(Debug,Serialize, Deserialize)]
pub struct ApiQuery {
    #[serde(default)]
    add: Option<AddQuery>,
    #[serde(default)]
    search:  Option<SearchQuery>,
}

impl ApiQuery {
    pub fn add_query(content: String, url: String) -> ApiQuery {
        ApiQuery { 
            add: Some(AddQuery{content, url}), 
            search: None 
        }
    }
    pub fn search_query(prompt: Option<String>,
                            content: String, 
                            min_sim: f64,
                            max_results: usize
    ) -> ApiQuery {
        ApiQuery { 
            add: None,
            search: Some(SearchQuery { 
                prompt,
                content,
                min_sim,
                max_results 
            }) 
        }
    }

    pub fn get_as_string(&self, url: String) -> Option<String>{
        let client = Client::new();

        let response = client
            .post(url)
            .header("Content-Type", "application/json")
            .json(&self) // Serialize the JSON body
            .send();

        match response {
            Ok(response) => {
                match response.json::<ApiResponse>() {
                    Ok(response) => {
                            match serde_json::to_string(&response) {
                                Ok(response) => {Some(response)},
                                Err(_) => { None }
                            }
                    }
                    Err(_) => { None }
                }
            },
            Err(_) => { None }
        }
    }
}

#[derive(Debug,Serialize, Deserialize)]
pub struct SearchQuery {
    #[serde(default)]
    prompt: Option<String>,
    content: String,
    min_sim: f64,
    max_results: usize
}

#[derive(Debug,Serialize, Deserialize)]
pub struct AddQuery {
    content: String,
    url: String
}

fn handle_add_request(add_query: AddQuery) {
    let embeddings = get_add_embeddings(add_query.content);
    if let Ok(mut parent_node) = PARENT_NODE.0.lock() {
        if let Ok(embeddings) = embeddings {
            parent_node.add_child(embeddings, add_query.url);
            if let Ok(db_path) = DB_PATH.lock() {
                parent_node.save_to_file(db_path.to_owned());
            }
        }
        //TODO: add error handling for failed get_add_embeddings
    };

}

fn handle_search_request(search_query: SearchQuery) -> Vec<(f64, String, u32)>{
    let embeddings = get_search_embeddings(search_query.prompt, search_query.content);

    match PARENT_NODE.0.lock() {
        Ok(parent_node) => {
            match embeddings {
                Ok(embeddings) => {
                    parent_node.search(search_query.min_sim, search_query.max_results, &embeddings)
                },
                Err(_) => {Vec::new()}
            }
        },
        Err(_) => {Vec::new()}
    }
}

fn handle_client(mut stream: TcpStream) {
    // Read the incoming request
    let mut buffer = [0; 10000];
    stream.read(&mut buffer).expect("Failed to read from stream");
    let request = String::from_utf8_lossy(&buffer[..]);

    println!("Received request:\n{}", request);
    let mut parts = request.split("\r\n\r\n");
    let header = parts.next().expect("Didnt find header");
    let header: Vec<&str> = header.split('\n').into_iter().collect();
    //println!("{:?}", header);
    let content_len: Vec<usize> = header
        .into_iter()
        .filter_map(|s|{
            if s.starts_with("content-length:") {
                let s = s.replace("content-length:", "");
                match s.trim().parse::<usize>() {
                    Ok(size) => { 
                        println!("Found content len: {}", size);
                        Some(size) 
                    },
                    Err(_) => { None }
                }
            } else {
                None
            }
        })
        .collect();
    let content_len = match content_len.last() {
        Some(content_len) => {
            content_len
        },
        None => { return }
    };
    let body = format!( "{}", parts.next().expect("Didn't get json body").trim());
    let body = body[0..(content_len+0)].to_owned();


    let mut query_state: QueryState = QueryState::DidNothing;

    let api_query: ApiQuery = match serde_json::from_str(&body) {
        Ok(data) => data,
        Err(e) => {
            let response = ApiResponse::from(QueryState::ParseFailed);
            response.send(stream);
            eprintln!("Error parsing JSON: {}", e);
            return
        }
    };
    

    if let Some(add_query) = api_query.add {
        query_state = QueryState::Added;
        handle_add_request(add_query);
    }

    if let Some(search_query) = api_query.search {
        let search_results = handle_search_request(search_query);
        query_state = match query_state {
            QueryState::Added => { QueryState::AddSearch(search_results) },
            _ => { QueryState::Searched(search_results) }
        };
    }

    let response = ApiResponse::from(query_state);
    response.send(stream);
}

fn get_search_embeddings(prompt: Option<String>, search_term: String) -> Result< Vec<f64>, NodeError> {
    match prompt {
        Some(prompt) => {
            let chat_request = gpt35!(
                system!(prompt),
                user!(search_term)
                ).get();

            match chat_request {
                Ok(chat_request) => {
                    let choice = chat_request.default_choice();
                    //println!("{}", choice);
                    let embeddings = EmbeddingRequest::new(choice).get();
                    match embeddings {
                        Ok(embeddings) => {
                            match embeddings.get_embeddings() {
                                Some(embeddings) => {Ok(embeddings.clone())},
                                None => {Err(NodeError::from("No search embeddings were found")) }
                            }
                        },
                        Err(err_msg) => { Err(NodeError { msg: err_msg.message })}
                    }
                },
                Err(err_msg) => {Err(NodeError{ msg: err_msg.message})}
            }
        },
        None => { get_add_embeddings(search_term) }
    }
}


fn get_add_embeddings(content: String) -> Result< Vec<f64>, NodeError> {
    let embeddings = EmbeddingRequest::new(content).get();
    match embeddings {
        Ok(embeddings) => {
            match embeddings.get_embeddings() {
                Some(embeddings) => {Ok(embeddings.clone())},
                None => {Err(NodeError::from("No search embeddings were found")) }
            }
        },
        Err(err_msg) => { Err(NodeError { msg: err_msg.message })}
    }
}
#[pyfunction]
pub fn run_server(addr: String, db_path: String) {
    std::thread::spawn(||{run_server_blocking(addr, db_path)});
}

#[pyfunction]
pub fn run_server_blocking(addr: String, new_db_path: String) {
    if let Ok(mut db_path) = DB_PATH.lock() {
        *db_path = new_db_path.clone();
        if let Ok(mut parent_node) = PARENT_NODE.0.lock() {
            if let Ok(node) = Node::load_model(&new_db_path){
                *parent_node = node
            } else {
                println!("No serialized DB found. Starting new DB");
            };
        }
    };
    
    let listener = TcpListener::bind(&addr).expect("Failed to bind to address");

    println!("Vector DB REST API running on http://{}/", &addr);

    // Accept and handle incoming connections
    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                std::thread::spawn(|| {
                    handle_client(stream);
                });
            }
            Err(e) => eprintln!("Error accepting connection: {}", e),
        }
    }
}
#[pyfunction]
pub fn prompted_search(
    url: String,
    prompt: String,
    content: String,
    min_sim: f64,
    max_results: usize
) -> Option<String> {
    ApiQuery::search_query(Some(prompt), content, min_sim, max_results).get_as_string(url)
}

#[pyfunction]
pub fn vector_search(
    url: String,
    content: String,
    min_sim: f64,
    max_results: usize
) -> Option<String> {
    ApiQuery::search_query(None, content, min_sim, max_results).get_as_string(url)
}

#[pyfunction]
pub fn add_vector(
    url: String,
    content: String,
    link: String
) -> Option<String> {
    ApiQuery::add_query(content, link).get_as_string(url)
}

#[pymodule]
fn rs_vector_db(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(run_server, m)?)?;
    m.add_function(wrap_pyfunction!(run_server_blocking, m)?)?;
    m.add_function(wrap_pyfunction!(prompted_search, m)?)?;
    m.add_function(wrap_pyfunction!(vector_search, m)?)?;
    m.add_function(wrap_pyfunction!(add_vector, m)?)?;
    Ok(())
}

