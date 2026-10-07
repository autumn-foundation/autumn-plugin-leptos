//! Leptos islands for Autumn apps: Leptos 0.8 components (wasm) inside
//! Maud + htmx pages.
//!
//! 1. Write the components in a wasm crate. Register them with
//!    `autumn-plugin-leptos-client` in the start function:
//!
//!    ```rust,ignore
//!    use autumn_plugin_leptos_client::register;
//!    use leptos::prelude::*;
//!    use wasm_bindgen::prelude::*;
//!
//!    #[wasm_bindgen(start)]
//!    pub fn start() {
//!        register("Counter", counter);
//!    }
//!    ```
//!
//! 2. Build it with `cargo build --target wasm32-unknown-unknown` and
//!    `wasm-bindgen --target web`. Embed the output as a `PluginAssets`
//!    bundle and install it:
//!
//! ```rust,no_run
//! use autumn_plugin_leptos::{Island, LeptosPlugin, leptos_bundle, leptos_script};
//! use autumn_web::assets::PluginAssets;
//! use autumn_web::prelude::*;
//!
//! static ISLANDS: PluginAssets = PluginAssets::from_files(
//!     "app-islands",
//!     &[("app.js", b"/* glue */"), ("app_bg.wasm", b"\0asm\x01\0\0\0")],
//! );
//!
//! #[get("/")]
//! async fn index() -> AutumnResult<Markup> {
//!     let counter = Island::new("Counter")
//!         .props(&serde_json::json!({ "start": 3 }))?
//!         .fallback(html! { p { "Count: 3" } });
//!     Ok(html! {
//!         html {
//!             head { (leptos_script()) (leptos_bundle(&ISLANDS)) }
//!             body { (counter) }
//!         }
//!     })
//! }
//!
//! # async fn run() {
//! autumn_web::app()
//!     .plugin(LeptosPlugin::new().bundle(&ISLANDS))
//!     .routes(routes![index])
//!     .run()
//!     .await;
//! # }
//! ```
//!
//! 3. Set a CSP that allows wasm: [`WasmCspLoader`] or [`wasm_csp()`].
//!
//! The loader (`leptos-islands.js`) imports each bundle and mounts each
//! island. It mounts islands that htmx swaps in and unmounts islands that
//! htmx swaps out. A `data-leptos-props` change sets the props signal of
//! the same instance, so the component keeps its state. It uses no inline
//! script and no `eval`.
//!
//! # Features
//!
//! - `ssr`: [`Island::fallback_view`] renders a Leptos view on the server
//!   as the fallback.
//!
//! # Security
//!
//! The loader mounts each `data-leptos-island` element in the page and
//! imports each accepted bundle link. If your app shows user HTML, the
//! sanitizer must remove `data-leptos-*`, `hx-*` and `data-hx-*`
//! attributes and `<link>` and `<template>` elements. `data-leptos-ignore`
//! does not give full protection. An htmx out-of-band swap can move an
//! element out of it.

mod assets;
mod csp;
mod island;
mod plugin;
mod tags;
mod update;

pub use assets::{ASSETS_NAMESPACE, LEPTOS_ASSETS, LOADER_JS};
pub use csp::{WASM_UNSAFE_EVAL, WasmCspLoader, add_wasm_unsafe_eval, wasm_csp};
pub use island::{AttrError, Island, JsonKind, MountWhen, PropsError};
pub use plugin::{LeptosPlugin, PLUGIN_NAME};
pub use tags::{leptos_bundle, leptos_script};
pub use update::{PROPS_EVENT, PropsUpdate};
