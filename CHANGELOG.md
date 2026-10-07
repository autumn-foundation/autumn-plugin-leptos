# Changelog

## 0.1.0 (unreleased)

First release. Targets `autumn-web` 0.8 and Leptos 0.8.

### Added

- `LeptosPlugin`: serves the island loader and wasm-bindgen app bundles
  through `plugin_assets` (hashed URLs, `immutable` cache, SRI).
- `leptos_script()` and `leptos_bundle()`: head tags. The glue tag
  carries the hashed wasm URL and its SRI hash.
- `Island`: the mount element, with typed props, mount strategies
  (`load`, `idle`, `visible`), fallback, extra attributes and inline mode.
- `Island::fallback_view` (`ssr` feature): the server HTML of a Leptos
  view as the fallback.
- `PropsUpdate`: new props in an `HX-Trigger` header.
- `wasm_csp()`, `add_wasm_unsafe_eval()` and `WasmCspLoader`: a CSP that
  allows wasm compilation.
- `leptos-islands.js`: the loader. It loads bundles with SRI, checks the
  ABI, mounts and unmounts islands with htmx swaps, sends props updates
  and puts fallbacks back after a panic.
- `autumn-plugin-leptos-client`: `register` with reactive props, ABI
  version 1 and a panic hook with a private loader callback.
- The loader accepts only same-origin `modulepreload` bundle links with
  SRI hashes, keeps a fallback copy for htmx history, and gives `error`
  to an island whose name no loaded bundle registers.
- Demo app, demo wasm crate and its committed bundle.
