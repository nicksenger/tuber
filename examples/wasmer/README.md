# Tuber-wasmer

Example of using tuber to send messages between a web app's main UI thread and a [web worker](https://developer.mozilla.org/en-US/docs/Web/API/Web_Workers_API) configured to use the [wasmer-js](https://github.com/wasmerio/wasmer-js) runtime for true multithreading.

This example uses [trunk](https://github.com/trunk-rs/trunk). Use `trunk build --release` to build assets to `/dist`. Since the wasmer-js runtime makes use of `SharedArrayBuffer`, you'll need to serve the page with cross-origin isolation. I generally use [simple-http-server](https://crates.io/crates/simple-http-server) for this with the `--coop` and `--coep` flags.
