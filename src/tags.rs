//! Tag helpers for the page `<head>`.

use autumn_web::assets::PluginAssets;
use maud::Markup;

use crate::assets::{LEPTOS_ASSETS, LOADER_JS};
use crate::plugin::wasm_pairs;

/// Renders the `<script defer>` tag of the island loader.
///
/// The tag has the hashed URL, `integrity` and `crossorigin="anonymous"`.
/// Put it in the `<head>`.
///
/// ```rust
/// let html = autumn_plugin_leptos::leptos_script().into_string();
/// assert!(html.contains("/static/_plugins/leptos/leptos-islands."), "{html}");
/// assert!(html.contains(" defer"), "{html}");
/// ```
#[must_use]
pub fn leptos_script() -> Markup {
    LEPTOS_ASSETS.deferred_script_tag(LOADER_JS)
}

/// Renders the tags of an app bundle, in this order:
///
/// 1. `<link rel="stylesheet">` for each `.css` file.
/// 2. For each wasm-bindgen pair (`x.js` and `x_bg.wasm`):
///    - `<link rel="modulepreload">` for the glue module. Its
///      `data-leptos-wasm` and `data-leptos-wasm-integrity` attributes give
///      the hashed wasm URL and its SRI hash. The loader reads them.
///    - `<link rel="preload" as="fetch">` for the wasm module.
///
/// Files in a group are in logical-path order. Other files (snippets,
/// fonts, source maps) get no tag. Each tag has the hashed URL and
/// `integrity`.
#[must_use]
pub fn leptos_bundle(bundle: &PluginAssets) -> Markup {
    let css = bundle.iter().filter(|asset| {
        std::path::Path::new(asset.logical_path())
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("css"))
    });
    maud::html! {
        @for asset in css { (bundle.stylesheet_tag(asset.logical_path())) }
        @for (glue, wasm) in wasm_pairs(bundle) {
            link rel="modulepreload" href=(glue.url()) integrity=(glue.integrity())
                crossorigin="anonymous" data-leptos-wasm=(wasm.url())
                data-leptos-wasm-integrity=(wasm.integrity());
            link rel="preload" href=(wasm.url()) as="fetch" type="application/wasm"
                integrity=(wasm.integrity()) crossorigin="anonymous";
        }
    }
}
