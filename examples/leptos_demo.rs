//! Demo app for `autumn-plugin-leptos`.
//!
//! ```sh
//! cargo run --features ssr --example leptos_demo
//! ```
//!
//! Then open <http://127.0.0.1:3000>. The page has a counter, a clock that
//! htmx adds and removes, and a panic button.
//!
//! The wasm bundle (`examples/islands/`) is the committed output of
//! `examples/islands-app/build.sh`. You need no wasm toolchain to run the
//! demo. The server renders the counter from the same component file as
//! the bundle (`ssr` feature).

// The server renders only `counter`. The wasm crate uses the others.
#[path = "islands-app/src/components.rs"]
#[allow(dead_code)]
mod components;

use autumn_plugin_leptos::{
    Island, LeptosPlugin, MountWhen, PropsUpdate, WasmCspLoader, leptos_bundle, leptos_script,
};
use autumn_web::Route;
use autumn_web::assets::{PluginAssets, asset_url};
use autumn_web::prelude::*;
use components::CounterData;
use leptos::prelude::Signal;

/// The demo islands, built from `examples/islands-app/`.
static ISLANDS: PluginAssets = PluginAssets::from_files(
    "leptos-demo",
    &[
        (
            "leptos_demo_islands.js",
            include_bytes!("islands/leptos_demo_islands.js"),
        ),
        (
            "leptos_demo_islands_bg.wasm",
            include_bytes!("islands/leptos_demo_islands_bg.wasm"),
        ),
    ],
);

#[autumn_web::main]
async fn main() {
    autumn_web::app()
        // The Autumn CSP plus `'wasm-unsafe-eval'`.
        .with_config_loader(WasmCspLoader::default())
        .plugin(plugin())
        .routes(routes())
        .run()
        .await;
}

/// The plugin with the demo bundle.
pub fn plugin() -> LeptosPlugin {
    LeptosPlugin::new().bundle(&ISLANDS)
}

/// The demo routes.
#[must_use]
pub fn routes() -> Vec<Route> {
    routes![index, rename, add_clock, remove_clock]
}

fn layout(content: &Markup) -> Markup {
    html! {
        (maud::DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "Leptos islands demo" }
                script src=(asset_url("js/htmx.min.js")) defer {}
                (leptos_script())
                (leptos_bundle(&ISLANDS))
            }
            body { (content) }
        }
    }
}

#[get("/")]
async fn index() -> AutumnResult<Markup> {
    let data = CounterData {
        start: 3,
        label: "Visits".to_owned(),
    };
    let counter = Island::new("Counter")
        .id("counter")
        .props(&data)?
        // The server HTML of the same component.
        .fallback_view(move || components::counter(Signal::stored(data)));
    let boom = Island::new("Boom")
        .id("boom")
        .mount_when(MountWhen::Visible)
        .fallback(html! { p { "The panic button needs wasm." } });
    Ok(layout(&html! {
        h1 { "Leptos islands" }
        section {
            h2 { "Counter" }
            (counter)
            button #rename hx-get="/rename" hx-swap="none" { "Rename (props update)" }
        }
        section {
            h2 { "Clock" }
            div #clock-slot {}
            button #add-clock hx-get="/clock" hx-target="#clock-slot" { "Add clock" }
            button #remove-clock hx-get="/clock/remove" hx-target="#clock-slot" { "Remove clock" }
        }
        section {
            h2 { "Panic" }
            p { "A panic stops the wasm bundle. Each island of the bundle shows its fallback." }
            (boom)
        }
    }))
}

/// Sends a new label. The counter keeps its count.
#[get("/rename")]
async fn rename() -> AutumnResult<(PropsUpdate, Markup)> {
    let data = CounterData {
        start: 0,
        label: "Hits".to_owned(),
    };
    Ok((PropsUpdate::new().set("#counter", &data)?, html! {}))
}

#[get("/clock")]
async fn add_clock() -> Markup {
    html! { (Island::new("Clock").id("clock").fallback(html! { p { "Ticks: 0" } })) }
}

#[get("/clock/remove")]
async fn remove_clock() -> Markup {
    html! {}
}

#[cfg(test)]
mod tests {
    use autumn_web::test::TestApp;

    use super::{ISLANDS, plugin, routes};

    fn client() -> autumn_web::test::TestClient {
        TestApp::new().plugin(plugin()).routes(routes()).build()
    }

    #[tokio::test]
    async fn index_renders_islands_and_bundle_tags() {
        let response = client().get("/").send().await;
        response.assert_ok();
        let html = response.text();
        assert!(html.contains(r#"data-leptos-island="Counter""#), "{html}");
        assert!(html.contains(r#"<div class="counter">"#), "{html}");
        assert!(html.contains(r#"data-leptos-mount="visible""#), "{html}");
        assert!(html.contains(&ISLANDS.url("leptos_demo_islands_bg.wasm")));
    }

    #[tokio::test]
    async fn bundle_files_are_served() {
        let client = client();
        for asset in ISLANDS.iter() {
            client.get(asset.url()).send().await.assert_ok();
        }
    }

    #[tokio::test]
    async fn rename_sends_a_props_update() {
        let response = client().get("/rename").send().await;
        response.assert_ok();
        let header = response.header("hx-trigger").unwrap_or_default();
        assert!(header.contains(r#""label":"Hits""#), "{header}");
    }

    #[tokio::test]
    async fn clock_fragments_add_and_remove() {
        let client = client();
        let html = client.get("/clock").send().await.text();
        assert!(html.contains(r#"data-leptos-island="Clock""#), "{html}");
        let response = client.get("/clock/remove").send().await;
        response.assert_ok();
        assert_eq!(response.text(), "");
    }
}
