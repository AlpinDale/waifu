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

```sh
cargo run -- --host 0.0.0.0 --port 8000 --images-path /images
```
