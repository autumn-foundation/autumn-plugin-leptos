# CLAUDE.md - autumn-plugin-leptos

Leptos (wasm) islands for Autumn 0.8 apps, served through the
`plugin_assets` API. Read `docs/plan.md` and `docs/adr/` first.

## Layout

| Path | Content |
| --- | --- |
| `assets/leptos-islands.js` | The loader. The only file in `LEPTOS_ASSETS`. |
| `src/plugin.rs` | `LeptosPlugin` and the wasm-bindgen pair check. |
| `src/island.rs` | `Island`, `MountWhen`, `PropsError`, `AttrError`, `props_json`, `fallback_view` (`ssr`). |
| `src/update.rs` | `PropsUpdate`: ASCII-safe `HX-Trigger` header. |
| `src/tags.rs` | `leptos_script()`, `leptos_bundle()`. |
| `src/csp.rs` | `wasm_csp()`, `add_wasm_unsafe_eval()`, `WasmCspLoader`. |
| `client/` | `autumn-plugin-leptos-client`: `register` and the wasm ABI. |
| `tests/browser.rs` | Loader tests in Chromium: fake ABI bundles and the real bundle. |
| `tests/e2e.rs` | The demo in Chromium with real htmx. |
| `tests/fixtures/` | Fake glue modules and the test probe. |
| `examples/leptos_demo.rs` | Demo app (`ssr` feature). |
| `examples/islands-app/` | Demo wasm crate (own workspace) and `build.sh`. |
| `examples/islands/` | Committed demo bundle and `SOURCE.sha256`. |

## Commands

- Lint: `cargo fmt --all && cargo clippy --workspace --all-targets --all-features -- -D warnings`
- Tests: `cargo test --workspace --all-features`
- Browser tests: `cargo test --all-features --test browser --test e2e -- --ignored --test-threads=1`
- Coverage: `cargo llvm-cov --workspace --all-features --all-targets --summary-only` (keep ≥ 85%)
- Rebuild the demo bundle: `examples/islands-app/build.sh` (commit the output)
- Check the bundle: `examples/islands-app/build.sh --check`

## Rules

- A change to `client/src/lib.rs` or `examples/islands-app/` needs a
  bundle rebuild. CI checks `SOURCE.sha256`.
- The loader must not use `eval`, `new Function`, `innerHTML`,
  `outerHTML`, `insertAdjacentHTML` or `document.write`. It has one
  `import()`: the glue URL. `tests/loader_source.rs` checks this.
- The loader makes no call into a crashed bundle.
- The loader never throws out of a scan. One bad island or bundle must
  not stop the others.
- DOM calls in the loader use prototype methods and getters, so an
  element cannot clobber them.
- Props are always a JSON object. `Island` and `PropsUpdate` share
  `props_json`. The client checks it too.
- A change to the ABI, the attribute names or the events is a breaking
  change. Change `ABI_VERSION`, `const ABI` and the README together.
- A new loader branch needs a browser test that fails without it.
- Native client code must not call `web_sys` outside `wasm32`.
- Write docs and comments in ASD-STE100: short sentences, active voice.
- Never bump the crate version unless the user asks for a release.
