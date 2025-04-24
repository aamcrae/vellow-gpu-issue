# Vello GPU test

Running this vello GPU client in a browser causes a 3 second GPU freeze.

To build and run:

```
cargo build -r --target=wasm32-unknown-unknown
wasm-bindgen target/wasm32-unknown-unknown/release/vellowasm.wasm --out-dir www --target web
cd www
devserver --address 127.0.0.1:8400
```

Then navigate your local browser to the URL.
