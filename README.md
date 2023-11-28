# Vector DB REST API

This is a Rust library for a simple Vector Database with REST API. It allows you to
add vectors with associated metadata and perform similarity searches on those
vectors.

## Features

- **Add Vectors**: Add vectors to the database with associated metadata (e.g.,
  content and URL).

- **Search Vectors**: Perform similarity searches on vectors based on a given
  prompt and search term.

## Dependencies

- `serde::{Serialize, Deserialize}`
- `serde_json::{Value, json}`
- `vector_node::prelude::*`
- `openai_api::prelude::*`
- `lazy_static`

## Usage

```rust
use vector_db_api::{ApiQuery, run_server};

fn main() {
    let addr = "127.0.0.1:3000".to_string();
    let new_db_path = "./new_serialized_vector_db.json".to_string();

    // Run the server
    vector_db_api::run_server(addr, new_db_path);
}
```

## API Endpoints

### `POST /query`

#### Request Body

```json
{
  "add": {
    "content": "Example content",
    "url": "https://example.com"
  },
  "search": {
    "prompt": "Optional prompt for search",
    "content": "Example search term",
    "min_sim": 0.8,
    "max_results": 5
  }
}
```

#### Response

```json
{
  "body": "Add response was successful",
  "state": "Added"
}
```

## Configuration
- **DB_PATH**: The path to the serialized vector database file.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
