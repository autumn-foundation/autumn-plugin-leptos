//! The demo islands of `autumn-plugin-leptos`.

mod components;

use autumn_plugin_leptos_client::register;
use wasm_bindgen::prelude::*;

/// Registers the islands. wasm-bindgen calls it at `init`.
#[wasm_bindgen(start)]
pub fn start() {
    register("Counter", components::counter);
    register("Clock", components::clock);
    register("Boom", components::boom);
}
