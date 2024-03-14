# Install Dev deps. 
* rust - 1.75.0
* [wasm-pack](https://rustwasm.github.io/wasm-pack/)

Compiling wasm with wasm-pack
```
cargo update
cargo build --target wasm32-unknown-unknown --release
wasm-pack build --target web --release
```
