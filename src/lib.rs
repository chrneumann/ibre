#![warn(missing_docs)]
//! IBRE - In Browser Routing Engine

mod debug;
mod geo_types;
mod routing;
mod tile;

extern crate console_error_panic_hook;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
/// Sets up hooks so that panics are forwarded to console.error.
///
/// If you want this behaviour, call the function one time in your code.
pub fn init_hooks() {
    std::panic::set_hook(Box::new(console_error_panic_hook::hook));
}
