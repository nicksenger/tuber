const COMMANDS: &[&str] = &["tx", "rx"];

fn main() {
    tauri_plugin::Builder::new(COMMANDS).build();
}
