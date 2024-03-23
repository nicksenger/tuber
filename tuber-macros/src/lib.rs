#[allow(unused_extern_crates)]
extern crate proc_macro;

use proc_macro::TokenStream;

mod common;
mod tauri;
mod worker;

#[cfg(feature = "worker")]
#[proc_macro_attribute]
pub fn worker(args: TokenStream, item: TokenStream) -> TokenStream {
    worker::worker(args.into(), item.into()).into()
}

#[cfg(feature = "tauri")]
#[proc_macro_attribute]
pub fn tauri_web(args: TokenStream, item: TokenStream) -> TokenStream {
    tauri::tauri_web(args.into(), item.into()).into()
}
