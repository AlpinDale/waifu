# Waifu

REST API server for serving waifu images. Work in progress, come back later.


## Build and Run

```sh
cargo run
```

## Add an image

```sh
curl -X POST http://localhost:8000/image -H "Content-Type: application/json" -d '{"path": "path/to/image.jpg", "type": "local"}'
```


## Get a random image

```sh
curl http://localhost:8000/random
```


## Configuration

We use dotenv to manage configuration. Copy `.env_example` to `.env` and set the variables as needed.