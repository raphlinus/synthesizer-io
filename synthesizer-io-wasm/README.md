Generally following [wasm-bindgen tutorial](https://rustwasm.github.io/wasm-bindgen/basic-usage.html).

```
cargo build --target=wasm32-unknown-unknown --release
wasm-bindgen target/wasm32-unknown-unknown/release/synthesizer_io_wasm.wasm --out-dir .
npm install
npm run serve
```
