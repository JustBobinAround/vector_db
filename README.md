# Vector_DB Python Integration

## Overview

rs_vector_db is a Rust-based project designed for efficient vector storage and
similarity search. This README provides guidance on integrating Vector_DB with
Python. The integration allows you to run the Vector_DB server and perform
vector addition and similarity searches using Python scripts.

## **THIS REQUIRES AN OPENAPI KEY**
```
export OPENAI_API_KEY="your-key-here";
```

## Installation

```bash
pip install rs_vector_db
```

## Example Usage

```python
from rs_vector_db import run_server, vector_search, add_vector
import time

URL = "http://127.0.0.1:3000"

# Start Vector_DB server on its own thread
run_server("127.0.0.1:3000", "./metal_gear_db.json")

# Wait for the server to initialize - this might not be necessary
time.sleep(1)

# Add a vector to the DB
results = add_vector(URL, "Why are we still here to suffer", "https://www.youtube.com/watch?v=N_vJMHMBzLM")
print(results)

results = vector_search(URL, "I can still feel the pain in my leg and in my arm", 0.2, 9000)

print(results)
```

## Contributing

If you encounter any issues or have suggestions for improvements, feel free to
open an issue or submit a pull request.

## Notes for myself
```
maturin publish
__token__
key
```

## License

This project is licensed under the [MIT License](LICENSE).
