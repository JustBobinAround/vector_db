use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use serde::{Serialize, Deserialize};
use serde_json::{Value, json};
use vector_node::prelude::*;
use openai_api::prelude::*;

lazy_static::lazy_static! {
    static ref PARENT_NODE: MutexWrapper<Node> = Node::new(0, Vec::<f64>::new(), String::new());
    static ref DB_PATH: String = String::from("./serialized_vector_db.json");
}

#[derive(Debug,Serialize, Deserialize)]
struct ApiQuery {
    #[serde(default)]
    add: Option<AddQuery>,
    #[serde(default)]
    search:  Option<SearchQuery>,
}

#[derive(Debug,Serialize, Deserialize)]
struct SearchQuery {
    #[serde(default)]
    prompt: Option<String>,
    content: String,
    min_sim: f64,
    max_results: usize
}

#[derive(Debug,Serialize, Deserialize)]
struct AddQuery {
    content: String,
    url: String
}

fn handle_add_request(add_query: AddQuery) {
    let embeddings = get_add_embeddings(add_query.content);
    if let Ok(mut parent_node) = PARENT_NODE.0.lock() {
        if let Ok(embeddings) = embeddings {
            parent_node.add_child(embeddings, add_query.url);
            parent_node.save_to_file(DB_PATH.to_owned());
        }
    };


}

fn handle_search_request(search_query: SearchQuery) {
    let embeddings = get_search_embeddings(search_query.prompt, search_query.content);
    if let Ok(parent_node) = PARENT_NODE.0.lock() {
        if let Ok(embeddings) = embeddings {
            parent_node.search(search_query.min_sim, search_query.max_results, &embeddings);
        }
    };
}

fn handle_client(mut stream: TcpStream) {
    // Read the incoming request
    let mut buffer = [0; 1024];
    stream.read(&mut buffer).expect("Failed to read from stream");
    let request = String::from_utf8_lossy(&buffer[..]);

    println!("Received request:\n{}", request);
    let mut parts = request.split("\r\n\r\n");
    let header = parts.next().expect("Didnt find header");
    let header: Vec<&str> = header.split(':').into_iter().collect();
    println!("{:?}", header);
    let content_len = header.last().unwrap().trim().parse::<usize>().unwrap();
    let body = format!( "{}", parts.next().expect("Didn't get json body").trim());
    let body = body[0..content_len].to_owned();


    let api_query: ApiQuery = match serde_json::from_str(&body) {
        Ok(data) => data,
        Err(e) => {
            eprintln!("Error parsing JSON: {}", e);
            return;
        }
    };
    

    if let Some(add_query) = api_query.add {
        handle_add_request(add_query);
    }

    if let Some(search_query) = api_query.search {
        handle_search_request(search_query);
    }

    let response_body = serde_json::to_string(&Option::<i32>::None).unwrap();

    println!("{}", response_body.len());
    let response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        response_body.len(),
        response_body
    );


    // Send the response
    stream.write_all(response.as_bytes()).expect("Failed to write to stream");
}

pub fn get_search_embeddings(prompt: Option<String>, search_term: String) -> Result< Vec<f64>, NodeError> {
    match prompt {
        Some(prompt) => {
            let chat_request = gpt35!(
                system!(prompt),
                user!(search_term)
                ).get();

            match chat_request {
                Ok(chat_request) => {
                    let choice = chat_request.default_choice();
                    println!("{}", choice);
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


pub fn get_add_embeddings(content: String) -> Result< Vec<f64>, NodeError> {
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



fn main() {
    // Bind the server to an address
    let listener = TcpListener::bind("127.0.0.1:3000").expect("Failed to bind to address");

    println!("Vector DB REST API running on http://127.0.0.1:3000/");

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

