//! Source checks for `assets/leptos-islands.js`. The browser tests check
//! the behavior. These checks keep the CSP and safety rules in the file.

const LOADER: &str = include_str!("../assets/leptos-islands.js");

/// The file text without whole-line `//` comments. A `//` inside a line
/// can be part of a string, so the check keeps those lines.
fn code() -> String {
    LOADER
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn uses_no_eval_or_html_strings() {
    let code = code();
    for banned in [
        "eval(",
        "Function(",
        "innerHTML",
        "outerHTML",
        "insertAdjacentHTML",
        "document.write",
        "srcdoc",
        "DOMParser",
        "createContextualFragment",
        "setHTMLUnsafe",
        "parseHTMLUnsafe",
    ] {
        assert!(!code.contains(banned), "leptos-islands.js uses {banned}");
    }
}

#[test]
fn imports_only_bundle_urls() {
    // The one dynamic `import` loads the glue URL of a bundle link.
    let code = code();
    assert_eq!(code.matches("import(").count(), 1, "one import");
    assert!(
        code.contains("import(bundle.url)"),
        "imports the bundle URL"
    );
}

#[test]
fn fetches_wasm_with_its_integrity() {
    let code = code();
    assert!(code.contains("integrity"), "SRI for the wasm fetch");
    assert!(code.contains("module_or_path"), "wasm-bindgen init option");
}

#[test]
fn timers_get_functions_not_strings() {
    let code = code();
    for (i, _) in code.match_indices("setTimeout(") {
        let argument = code[i + "setTimeout(".len()..].trim_start();
        assert!(
            argument.starts_with("run") || argument.starts_with("()"),
            "setTimeout gets a function: {}",
            &code[i..(i + 40).min(code.len())]
        );
    }
}

#[test]
fn is_strict_and_wrapped() {
    let code = code();
    assert!(code.contains("'use strict';"), "strict mode");
    assert!(code.trim_start().starts_with("(function"), "an IIFE");
}

#[test]
fn keeps_the_registry_in_a_map() {
    assert!(code().contains("new Map()"), "the registry is a Map");
}

#[test]
fn names_the_public_contract() {
    let code = code();
    for name in [
        "autumnLeptos",
        "data-leptos-island",
        "data-leptos-props",
        "data-leptos-mount",
        "data-leptos-state",
        "data-leptos-ignore",
        "data-leptos-wasm",
        "data-leptos-wasm-integrity",
        "autumn:leptos:",
        "autumn:leptos:props",
        "autumn:leptos:panic",
        "autumn_leptos_abi",
        "autumn_leptos_start",
        "autumn_leptos_mount",
        "MutationObserver",
    ] {
        assert!(code.contains(name), "leptos-islands.js names {name}");
    }
}

#[test]
fn abi_version_matches_the_client_crate() {
    let client = include_str!("../client/src/lib.rs");
    assert!(
        client.contains("pub const ABI_VERSION: u32 = 1;"),
        "client ABI is 1"
    );
    assert!(code().contains("const ABI = 1;"), "loader ABI is 1");
}
