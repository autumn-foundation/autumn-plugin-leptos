//! `LeptosPlugin` serves the loader and app bundles through `plugin_assets`.

#![allow(clippy::expect_used, clippy::panic)]

use autumn_plugin_leptos::{ASSETS_NAMESPACE, LEPTOS_ASSETS, LOADER_JS, LeptosPlugin, PLUGIN_NAME};
use autumn_web::assets::{PLUGIN_ASSETS_ROUTE_MARKER, PluginAsset, PluginAssets, asset_url};
use autumn_web::plugin::Plugin as _;
use autumn_web::plugin_conformance::{ConformanceConfig, run_conformance};
use autumn_web::route_listing::{RouteClassification, RouteSource};
use autumn_web::test::{TestApp, TestClient};
use base64::Engine as _;
use sha2::{Digest as _, Sha384};

const JS: &str = "text/javascript; charset=utf-8";
const WASM: &str = "application/wasm";
const IMMUTABLE: &str = "public, max-age=31536000, immutable";
const REVALIDATE: &str = "public, max-age=0, must-revalidate";
const PLAIN: &str = "/static/_plugins/leptos/leptos-islands.js";
const WASM_BYTES: &[u8] = b"\0asm\x01\0\0\0";

static APP: PluginAssets = PluginAssets::from_files(
    "leptos-plugin-test",
    &[
        ("app.js", b"export default async function init() {}"),
        ("app_bg.wasm", WASM_BYTES),
        ("app.css", b".a{}"),
    ],
);

static OTHER: PluginAssets = PluginAssets::from_files(
    "leptos-plugin-other",
    &[("other.js", b"export {};"), ("other_bg.wasm", WASM_BYTES)],
);

fn client() -> TestClient {
    TestApp::new()
        .plugin(LeptosPlugin::new().bundle(&APP))
        .build()
}

fn sri(bytes: &[u8]) -> String {
    format!(
        "sha384-{}",
        base64::engine::general_purpose::STANDARD.encode(Sha384::digest(bytes))
    )
}

#[test]
fn loader_bundle_holds_only_the_loader() {
    let files: Vec<&str> = LEPTOS_ASSETS
        .iter()
        .map(PluginAsset::logical_path)
        .collect();
    assert_eq!(files, [LOADER_JS]);
    assert_eq!(LOADER_JS, "leptos-islands.js");
    assert_eq!(ASSETS_NAMESPACE, "leptos");
    assert_eq!(LEPTOS_ASSETS.namespace(), ASSETS_NAMESPACE);
    assert_eq!(LEPTOS_ASSETS.mount_path(), "/static/_plugins/leptos");
}

#[test]
fn loader_is_the_file_on_disk_with_its_sri() {
    let asset = LEPTOS_ASSETS.get(LOADER_JS).expect("bundled");
    assert_eq!(asset.bytes(), include_bytes!("../assets/leptos-islands.js"));
    assert_eq!(asset.integrity(), sri(asset.bytes()));
    assert_eq!(asset.content_type(), JS);
    assert_eq!(asset.plain_url(), PLAIN);
}

#[test]
fn loader_url_is_fingerprinted() {
    let url = LEPTOS_ASSETS.url(LOADER_JS);
    let hash = url
        .strip_prefix("/static/_plugins/leptos/leptos-islands.")
        .and_then(|rest| rest.strip_suffix(".js"))
        .unwrap_or_else(|| panic!("{url} is fingerprinted"));
    assert_eq!(hash.len(), 8, "{url}");
    assert!(hash.bytes().all(|b| b.is_ascii_hexdigit()), "{url}");
}

#[tokio::test]
async fn serves_the_loader_at_its_fingerprinted_url_immutable() {
    let response = client().get(&LEPTOS_ASSETS.url(LOADER_JS)).send().await;
    response
        .assert_ok()
        .assert_header("content-type", JS)
        .assert_header("cache-control", IMMUTABLE);
    assert_eq!(
        response.body.as_slice(),
        include_bytes!("../assets/leptos-islands.js")
    );
}

#[tokio::test]
async fn serves_the_plain_url_with_revalidation_and_etag() {
    let client = client();
    let response = client.get(PLAIN).send().await;
    response
        .assert_ok()
        .assert_header("cache-control", REVALIDATE);
    let etag = response.header("etag").expect("etag").to_owned();
    client
        .get(PLAIN)
        .header("if-none-match", &etag)
        .send()
        .await
        .assert_status(304);
}

#[tokio::test]
async fn serves_the_app_bundle_with_the_same_rules() {
    let client = client();
    for asset in APP.iter() {
        let response = client.get(asset.url()).send().await;
        response
            .assert_ok()
            .assert_header("cache-control", IMMUTABLE)
            .assert_header("content-type", asset.content_type());
        assert_eq!(response.body.as_slice(), asset.bytes());
        client
            .get(asset.plain_url())
            .send()
            .await
            .assert_ok()
            .assert_header("cache-control", REVALIDATE);
    }
}

#[tokio::test]
async fn serves_wasm_as_application_wasm() {
    let asset = APP.get("app_bg.wasm").expect("wasm");
    assert_eq!(asset.content_type(), WASM);
    client()
        .get(asset.url())
        .send()
        .await
        .assert_ok()
        .assert_header("content-type", WASM);
}

#[tokio::test]
async fn unknown_and_stale_paths_are_not_found() {
    let client = client();
    for path in [
        "/static/_plugins/leptos/leptos-islands.00000000.js",
        "/static/_plugins/leptos/nope.js",
        "/static/_plugins/leptos/",
        "/static/_plugins/leptos-plugin-test/nope.wasm",
    ] {
        client.get(path).send().await.assert_status(404);
    }
}

#[tokio::test]
async fn asset_url_resolves_both_bundles() {
    let _client = client();
    assert_eq!(
        asset_url("_plugins/leptos/leptos-islands.js"),
        LEPTOS_ASSETS.url(LOADER_JS)
    );
    assert_eq!(
        asset_url("_plugins/leptos-plugin-test/app_bg.wasm"),
        APP.url("app_bg.wasm")
    );
}

#[test]
fn routes_are_public_and_plugin_attributed() {
    let plugin = LeptosPlugin::new().bundle(&APP);
    let name = plugin.name().into_owned();
    let app = autumn_web::app().plugin(plugin);
    let infos = app.plugin_route_infos().expect("route infos");
    let routes: Vec<_> = infos
        .iter()
        .filter(|info| info.path.starts_with("/static/_plugins/leptos"))
        .collect();
    // Loader: 1 file. App: 3 files. Two URLs for each file.
    assert_eq!(routes.len(), 8, "{infos:?}");
    for info in routes {
        assert_eq!(info.method, "GET");
        assert_eq!(info.classification, RouteClassification::Public);
        assert_eq!(info.middleware, [PLUGIN_ASSETS_ROUTE_MARKER]);
        assert_eq!(info.source, RouteSource::Plugin(name.clone()));
    }
}

#[test]
fn plugin_passes_conformance() {
    for plugin in [LeptosPlugin::new(), LeptosPlugin::new().bundle(&APP)] {
        let name = plugin.name().into_owned();
        let app = autumn_web::app().plugin(plugin);
        let infos = app.plugin_route_infos().expect("route infos");
        let report = run_conformance(&ConformanceConfig::new(&name), &infos);
        assert!(report.passed(), "{}", report.to_text_report());
    }
}

#[test]
fn plugin_declares_an_autumn_web_0_8_contract() {
    let contract = LeptosPlugin::new().contract().expect("contract");
    assert_eq!(contract.plugin, PLUGIN_NAME);
    assert_eq!(PLUGIN_NAME, "autumn-plugin-leptos");
    assert_eq!(
        contract.plugin_version.as_deref(),
        Some(env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(contract.autumn_web.as_deref(), Some("0.8"));
    assert_eq!(contract.experimental_surfaces, Vec::<String>::new());
    let app = autumn_web::app().plugin(LeptosPlugin::new());
    assert_eq!(app.plugin_contracts().len(), 1);
}

/// Returns the 8 hex digits after `ns@` in `name`.
fn fingerprint<'a>(name: &'a str, ns: &str) -> &'a str {
    let start = name.find(&format!("{ns}@")).expect("namespace in name") + ns.len() + 1;
    &name[start..start + 8]
}

#[test]
fn name_lists_the_bundles_with_a_fingerprint() {
    assert_eq!(LeptosPlugin::new().name(), PLUGIN_NAME);
    let name = LeptosPlugin::new()
        .bundle(&OTHER)
        .bundle(&APP)
        .bundle(&APP)
        .name()
        .into_owned();
    assert!(
        name.starts_with("autumn-plugin-leptos[leptos-plugin-other@"),
        "{name}"
    );
    assert!(name.contains(",leptos-plugin-test@"), "{name}");
    assert!(name.ends_with(']'), "{name}");
    for ns in ["leptos-plugin-other", "leptos-plugin-test"] {
        let hash = fingerprint(&name, ns);
        assert!(hash.bytes().all(|b| b.is_ascii_hexdigit()), "{name}");
    }
    // The same bundles give the same name, in any order.
    assert_eq!(LeptosPlugin::new().bundle(&APP).bundle(&OTHER).name(), name);
}

static SAME_NS_A: PluginAssets = PluginAssets::from_files(
    "leptos-plugin-same",
    &[("a.js", b"a"), ("a_bg.wasm", WASM_BYTES)],
);
static SAME_NS_B: PluginAssets = PluginAssets::from_files(
    "leptos-plugin-same",
    &[("a.js", b"b"), ("a_bg.wasm", WASM_BYTES)],
);

#[test]
fn different_bundles_in_one_namespace_get_different_names() {
    let a = LeptosPlugin::new().bundle(&SAME_NS_A).name();
    let b = LeptosPlugin::new().bundle(&SAME_NS_B).name();
    assert_ne!(a, b);
}

#[test]
#[should_panic(expected = "leptos-plugin-same")]
fn different_bundles_in_one_namespace_stop_the_app() {
    // Autumn does not skip the second plugin, so its namespace check runs.
    let _ = autumn_web::app()
        .plugin(LeptosPlugin::new().bundle(&SAME_NS_A))
        .plugin(LeptosPlugin::new().bundle(&SAME_NS_B));
}

#[tokio::test]
async fn installing_twice_is_harmless() {
    let client = TestApp::new()
        .plugin(LeptosPlugin::new().bundle(&APP))
        .plugin(LeptosPlugin::new().bundle(&APP))
        .plugin(LeptosPlugin::new())
        .build();
    client
        .get(&LEPTOS_ASSETS.url(LOADER_JS))
        .send()
        .await
        .assert_ok();
    client.get(&APP.url("app.js")).send().await.assert_ok();
}

#[tokio::test]
async fn two_plugins_with_different_bundles_both_serve() {
    let client = TestApp::new()
        .plugin(LeptosPlugin::new().bundle(&APP))
        .plugin(LeptosPlugin::new().bundle(&OTHER))
        .build();
    client.get(&APP.url("app.js")).send().await.assert_ok();
    client.get(&OTHER.url("other.js")).send().await.assert_ok();
    client
        .get(&LEPTOS_ASSETS.url(LOADER_JS))
        .send()
        .await
        .assert_ok();
}

#[test]
#[should_panic(expected = "the namespace `leptos` belongs to autumn-plugin-leptos")]
fn an_app_bundle_in_the_loader_namespace_is_refused() {
    static CLASH: PluginAssets =
        PluginAssets::from_files("leptos", &[("x.js", b""), ("x_bg.wasm", WASM_BYTES)]);
    let _ = LeptosPlugin::new().bundle(&CLASH);
}

#[test]
#[should_panic(expected = "has no wasm-bindgen pair")]
fn a_bundle_with_no_wasm_pair_is_refused() {
    // `x.js` needs `x_bg.wasm`. `y_bg.wasm` does not match.
    static NO_PAIR: PluginAssets = PluginAssets::from_files(
        "leptos-plugin-no-pair",
        &[("x.js", b""), ("y_bg.wasm", WASM_BYTES)],
    );
    let _ = LeptosPlugin::new().bundle(&NO_PAIR);
}

#[test]
fn plugin_is_default_and_debug() {
    let plugin = LeptosPlugin::default().bundle(&APP);
    let text = format!("{plugin:?}");
    assert!(text.contains("LeptosPlugin"), "{text}");
    assert_eq!(LeptosPlugin::default().name(), LeptosPlugin::new().name());
}
