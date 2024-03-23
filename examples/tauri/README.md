# Tuber-tauri

Example of using tuber to send messages between the native host, web guest main UI thread, and web guest worker in a [tauri](https://github.com/tauri-apps/tauri) application.

To run this example you'll need a few things:

- Tauri's CLI: https://crates.io/crates/tauri-cli
- Trunk CLI to build/serve the web app portion: https://github.com/trunk-rs/trunk
- (optional) cargo make: https://github.com/sagiegurari/cargo-make

If you have cargo make, just use `cargo make run` from this directory to serve the web portion & run the tauri app. Otherwise, serve the webapp from `/web` using `trunk serve` and run the tauri app from `/native` using `cargo tauri dev`.
