//! `leptos_script` and `leptos_bundle` render tags with SRI.

#![allow(clippy::expect_used, clippy::panic)]

use autumn_plugin_leptos::{LEPTOS_ASSETS, LOADER_JS, leptos_bundle, leptos_script};
use autumn_web::assets::PluginAssets;

const WASM: &[u8] = b"\0asm\x01\0\0\0";

static APP: PluginAssets = PluginAssets::from_files(
    "leptos-tags-test",
    &[
        ("b.js", b"b"),
        ("b_bg.wasm", WASM),
        ("a.js", b"a"),
        ("a_bg.wasm", b"\0asm\x01\0\0\0\0"),
        ("style.css", b"s"),
        ("snippets/x/inline0.js", b"i"),
        ("font.woff2", b"f"),
        ("a.js.map", b"{}"),
    ],
);

fn glue_tags(glue: &str, wasm: &str) -> String {
    let g = APP.get(glue).expect(glue);
    let w = APP.get(wasm).expect(wasm);
    format!(
        concat!(
            r#"<link rel="modulepreload" href="{}" integrity="{}" crossorigin="anonymous" "#,
            r#"data-leptos-wasm="{}" data-leptos-wasm-integrity="{}">"#,
            r#"<link rel="preload" href="{}" as="fetch" type="application/wasm" "#,
            r#"integrity="{}" crossorigin="anonymous">"#,
        ),
        g.url(),
        g.integrity(),
        w.url(),
        w.integrity(),
        w.url(),
        w.integrity()
    )
}

#[test]
fn loader_tag_is_deferred_with_sri() {
    let asset = LEPTOS_ASSETS.get(LOADER_JS).expect("loader");
    assert_eq!(
        leptos_script().into_string(),
        format!(
            r#"<script src="{}" integrity="{}" crossorigin="anonymous" defer></script>"#,
            asset.url(),
            asset.integrity()
        )
    );
}

#[test]
fn bundle_tags_are_css_then_each_wasm_pair() {
    let css = APP.get("style.css").expect("css");
    let expected = [
        format!(
            r#"<link rel="stylesheet" href="{}" integrity="{}" crossorigin="anonymous">"#,
            css.url(),
            css.integrity()
        ),
        glue_tags("a.js", "a_bg.wasm"),
        glue_tags("b.js", "b_bg.wasm"),
    ]
    .concat();
    assert_eq!(leptos_bundle(&APP).into_string(), expected);
}

#[test]
fn glue_tag_carries_the_hashed_wasm_url() {
    let html = leptos_bundle(&APP).into_string();
    let wasm = APP.url("a_bg.wasm");
    assert!(wasm.contains("a_bg."), "{wasm}");
    assert_ne!(wasm, APP.get("a_bg.wasm").expect("wasm").plain_url());
    assert!(
        html.contains(&format!(r#"data-leptos-wasm="{wasm}""#)),
        "{html}"
    );
}

#[test]
fn bundle_tags_skip_other_files() {
    let html = leptos_bundle(&APP).into_string();
    // Snippets load through the glue `import`. Fonts and maps get no tag.
    for skipped in ["inline0", "font.", ".map", "<script"] {
        assert!(!html.contains(skipped), "{skipped}: {html}");
    }
}

#[test]
fn empty_bundle_renders_nothing() {
    static EMPTY: PluginAssets = PluginAssets::from_files("leptos-tags-empty", &[]);
    assert_eq!(leptos_bundle(&EMPTY).into_string(), "");
}
