# Plan: autumn-plugin-leptos 0.1.0

Target: `autumn-web` 0.8.0 and Leptos 0.8. Writing rule: ASD-STE100.
Short sentences. Active voice. One topic in each sentence.

No GitHub issue exists for this plugin. The acceptance criteria in
section 8 replace the issue.

## 1. Problem

Autumn renders HTML on the server with Maud and htmx. Some parts of a page
need rich client state: an editor, a chart, a game. Leptos is a Rust UI
framework. It compiles to WebAssembly (wasm). Autumn has no standard way
to put a Leptos component into a Maud page. The Autumn guide
`wasm-islands.md` shows a manual Yew recipe. It says: "Leptos (CSR
`mount_to`) is a higher-momentum alternative with a smaller per-island
runtime if this ever graduates." This plugin is that step.

## 2. Goal

Let an app put Leptos components ("islands") into Maud pages:

- Serve a small JavaScript loader through the Autumn 0.8 `plugin_assets`
  API.
- Serve the app's wasm bundle (wasm-bindgen output) through the same API.
- Give a typed Rust `Island` builder for the server.
- Give a small client crate. The app registers its Leptos components with
  it.
- Mount and unmount islands when htmx changes the page.
- Keep the CSP strict. Add only `'wasm-unsafe-eval'`.

## 3. Brainstorming

Ideas, not filtered:

1. A loader file (`leptos-islands.js`) in a `PluginAssets` bundle. It gets
   a hashed URL, an `immutable` cache and SRI.
2. A Rust `Island` builder: name, JSON props, fallback, `id`, `class`,
   extra attributes, mount strategy, inline mode.
3. A client crate (`autumn-plugin-leptos-client`). The app calls
   `register("Counter", |props: Signal<CounterProps>| view! { … })`.
4. The client crate exports a small, versioned ABI through wasm-bindgen:
   `autumn_leptos_abi`, `autumn_leptos_start`, `autumn_leptos_mount` and an
   `AutumnLeptosIsland` handle with `update` and `unmount`. wasm-bindgen exports
   from a dependency appear in the app's glue module. The app writes no
   JavaScript.
5. The loader imports the glue module with `import()`. It calls `init`
   with a `fetch` that has the SRI hash of the `.wasm` file. Thus the
   browser checks the wasm bytes.
6. Props are a reactive `Signal<P>`. New props set the signal. Leptos
   keeps the component state.
7. One `MutationObserver` mounts added islands and unmounts removed
   islands. It covers htmx swaps, history restore and manual DOM edits.
8. Mount strategies: `load`, `idle`, `visible`.
9. DOM events (`autumn:leptos:mount`, `:update`, `:unmount`, `:error`) for
   htmx `hx-trigger` and for app code.
10. `PropsUpdate`: a handler sends new props in an `HX-Trigger` header.
11. A CSP helper adds `'wasm-unsafe-eval'` to a policy. It changes nothing
    else.
11a. `WasmCspLoader` wraps the app's `ConfigLoader` and adds the token to
    the loaded policy. Found during GREEN: `AppBuilder::with_config_loader`
    is the only hook that can change the CSP. The plugin does not call it
    itself: an app has one loader, and the app selects it.
12. Server-side fallback from the same Leptos component (`ssr` feature).
    Leptos is Rust, so the server can render the real view as the first
    paint. The client replaces it at mount.
13. A panic hook in the client crate. A Rust panic stops the wasm
    instance. The hook tells the loader, and the loader puts the fallbacks
    of that bundle back.
14. Hydrate the server HTML with `hydrate_from`. **Deferred**: the
    `hydrate` feature changes the client renderer for the full build. A
    markup mismatch panics and stops the bundle. CSR mount over an SSR
    fallback is safe and simple.
15. Use Leptos "islands" mode (`#[island]`, `hydrate_islands`).
    **Rejected**: it needs Leptos to render the full page. Maud owns the
    page here.
16. Write the loader in Rust (web-sys) inside the client crate.
    **Rejected**: each bundle would get its own observer. One JS loader
    serves all bundles and shows fallbacks when wasm cannot load.
17. A `trunk` integration. **Rejected**: trunk wants to own
    `index.html`. Maud owns the page.

## 4. Reverse brainstorming

Question: "How can this plugin fail its users?" Each answer gives a control.

| How to fail | Control |
| --- | --- |
| htmx removes an island and the Leptos owner leaks. | The observer unmounts each island that is not connected after the task. `unmount` drops the `UnmountHandle` and the owner. |
| A morph moves an island (remove, then add). The state is lost. | The loader unmounts only when the element is not connected. |
| A re-scan mounts one island two times. | One record for each element (`WeakMap`). |
| Props inject HTML or script. | Props go only in an attribute. Maud escapes it. Rust `serde` reads them into a typed struct. The loader uses no `innerHTML` and no `eval`. |
| The CSP blocks wasm. | The CSP helpers and `WasmCspLoader` add `'wasm-unsafe-eval'` only. The fallbacks stay when the CSP blocks wasm. `WasmCspLoader` refuses nonce mode, because a changed policy gets no nonces. |
| The glue and the committed demo bundle drift from their source. | `build.sh` writes a hash of the sources. CI checks it, builds the bundle and runs the browser tests with the new bundle. Byte equality is not possible: cargo puts the checkout path in symbol hashes. |
| A browser or a proxy changes the wasm bytes. | The loader fetches the `.wasm` with its SRI hash. The glue gets SRI through `modulepreload`. |
| A browser keeps old bytes after an upgrade. | Hashed URLs from `PluginAssets`. |
| The glue and the wasm come from different builds. | `leptos_bundle` pairs `x.js` with `x_bg.wasm` from one bundle. The demo build pins `wasm-bindgen-cli` to the lockfile version. |
| The client crate and the loader disagree on the ABI. | `autumn_leptos_abi()` returns a version. The loader refuses an unknown version. |
| A Rust panic stops the wasm instance. Islands freeze. | The panic hook calls a private loader callback. The loader marks the bundle as crashed and puts the fallbacks back. Other bundles keep working. |
| Page code fakes a panic and crashes a bundle. | The panic signal is a callback that only the bundle has, not a DOM event. |
| User HTML adds a bundle link, and the loader imports it as code. | The loader accepts only same-origin `modulepreload` links with SRI hashes, outside `[data-leptos-ignore]` and islands. |
| The glue frees a lost handle of a crashed bundle with a call into wasm. | The loader keeps the handles of a crashed bundle alive. After a panic, a dropped client handle leaks its Leptos state. |
| An htmx history snapshot has Leptos output and no fallback. | The loader keeps a fallback copy in `<template data-leptos-fallback>`. |
| A failed bundle leaves islands `pending` forever. | When no bundle loads, an unknown name gets `error`. A later bundle can still mount it. |
| Bad props JSON stops all islands. | Rust returns an error for that island only. No panic. |
| A props change resets the component state. | The loader calls `update` on the same handle. It sets the props signal. |
| Two bundles register one name. | The first registration stays. The loader writes a console error. |
| The loader mounts an island inside another island. Leptos and the loader fight over one DOM. | The loader skips islands inside an island. |
| The observer does work for each Leptos DOM change. The page gets slow. | The loader ignores mutations inside islands. |
| User HTML contains an island and mounts a real component. | `data-leptos-ignore` blocks islands. The docs tell the app to remove `data-leptos-*` and `hx-*` in the sanitizer. |
| An element with `id="autumnLeptos"` or `name="documentElement"` clobbers a global. | The loader uses prototype getters and checks the global. |
| The loader script loads two times. | The second copy does nothing. |
| The island goes away before `idle` or `visible` fires. | Teardown cancels the pending trigger. |
| Props are not a JSON object (`5`, `[1]`). | `Island::props` returns `PropsError::NotAnObject`. |
| The app bundle uses the namespace `leptos`, or has no wasm pair. | `LeptosPlugin::bundle` stops with a clear message. |
| No JavaScript or no wasm support. | The fallback content stays. |

## 5. Six thinking hats

- **White (facts).** `autumn-web` 0.8.0 has `PluginAssets::from_files`,
  `deferred_script_tag`, `stylesheet_tag` and `AppBuilder::plugin_assets`.
  It maps `.wasm` to `application/wasm`. The default CSP has
  `script-src 'self'`. Wasm needs `'wasm-unsafe-eval'`. Only the config
  loader can change the CSP, and an app has one loader, so the app opts
  in. Leptos 0.8.22 has
  `mount::mount_to(HtmlElement, f) -> UnmountHandle`. A drop of the handle
  unmounts. `view!{…}.to_html()` works on the server with the `ssr`
  feature. wasm-bindgen 0.2.129 glue `init` takes `{ module_or_path }`,
  also a `Promise<Response>`. A spike showed that exports from a
  dependency crate appear in the glue. `autumn_web::system_test` drives
  real Chromium. Verus is not on this machine.
- **Red (feelings).** Developers want to write Leptos in Rust and put it
  in a Maud page with one line. A wasm toolchain at build time is
  acceptable. JavaScript glue that they must write by hand is not. A loss
  of component state after an htmx swap feels like a bug.
- **Black (risks).** `cargo test` does not test JavaScript or wasm.
  Control: Chromium tests with a fake ABI bundle and a real Leptos bundle.
  Risk: a panic stops the wasm instance. Control: the panic hook and
  bundle-level fallback. Risk: wasm size. Control: release profile with
  `opt-level = "z"` and LTO in the demo. Risk: Leptos API change. Control:
  pin `leptos = "0.8"` in the client crate.
- **Yellow (benefits).** Rich Rust islands in an htmx app. One language
  on both sides. Typed props with shared `serde` structs. Cache and SRI
  with no work. One binary to deploy. The same shape as the React and
  Svelte plugins.
- **Green (alternatives).** SSR fallback from the real component. Reactive
  props through a signal. A CSP helper. A bundle-level panic guard.
- **Blue (process).** Plan → RED (failing Rust and browser tests) → GREEN
  (minimum code) → REFACTOR → review from several angles → fix → AC
  evidence. Verus is not available. Property tests (`proptest`), unit
  tests of the client registry and a state table with browser tests cover
  the invariants.

## 6. Island state machine

The loader keeps one record for each island element.

| From | Event | To |
| --- | --- | --- |
| (none) | scan finds a connected element, strategy `idle`/`visible` | `waiting` |
| (none) or `waiting` | trigger fires, name not registered, a bundle loads | `pending` |
| (none), `waiting` or `pending` | name not registered, no bundle loads | `error` (fallback stays) |
| `pending`, or `error` for an unknown name | a bundle registers the name | `mounted` |
| (none) or `waiting` | trigger fires, name registered | `mounted` |
| `waiting` | `data-leptos-mount` changes | start again |
| `mounted` | `data-leptos-props` changes | `mounted` (same handle, new props) |
| `mounted` | `data-leptos-props` is bad | `error` (last good render stays) |
| any | the bundle of the name crashed | `error` (fallback back) |
| `error` | `data-leptos-props` changes | start again |
| any | `data-leptos-island` changes or goes away | start again, or (none) |
| any | `mount` returns an error or throws | `error` (fallback stays) |
| any | the element moves into `[data-leptos-ignore]` or an island | (none), fallback back |
| any | the element is not connected after the task | (none), fallback back |

"Start again" means: tear down (fallback back), then scan the element.

Bundle states: `loading` → `ready` or `failed`; `ready` → `crashed`.
Only `ready` bundles mount islands. A refused link gets no bundle.

Invariants:

- One element has one record and one handle at most.
- Only a connected element gets a record.
- An element inside another island, or inside `[data-leptos-ignore]`, has
  no live record.
- A record in `pending`, or in `error` with no handle, shows the fallback.
- A torn-down element has its fallback and no `data-leptos-state`.
- A crashed bundle has no live handle. The loader makes no call into it.

## 7. Decisions

See [ADR 0001](adr/0001-leptos-islands.md).

## 8. Acceptance criteria

| ID | Criterion |
| --- | --- |
| AC1 | `LeptosPlugin::new()` installs the loader bundle (namespace `leptos`) through `AppBuilder::plugin_assets`. The loader is served at a hashed URL (`immutable`) and a plain URL (`must-revalidate`), with `ETag`/`304`. Unknown paths give `404`. |
| AC2 | `LeptosPlugin::bundle(&BUNDLE)` installs an app bundle with the same URL and cache rules. `.wasm` files are `application/wasm`. The namespace `leptos` and a bundle with no `x.js` + `x_bg.wasm` pair are refused. |
| AC3 | `leptos_script()` and `leptos_bundle(&BUNDLE)` render tags with hashed URLs, SRI and `crossorigin="anonymous"`. The glue tag carries the hashed wasm URL and its SRI hash. |
| AC4 | `Island` renders a Maud element with name, JSON props, mount strategy, fallback, `id`, `class`, checked extra attributes and an inline mode. Props that are not a JSON object give a typed error. All values are escaped. |
| AC5 | The client crate registers Leptos components with typed, reactive props (`Signal<P>`). It exports ABI version 1. An unknown name or bad props give an error, not a panic. |
| AC6 | The loader loads a real Leptos 0.8 bundle and mounts a component with its props in Chromium. The CSP is the Autumn default plus `'wasm-unsafe-eval'`. The console has no errors. |
| AC7 | The loader mounts islands that htmx swaps in. It unmounts islands that htmx swaps out, and Leptos cleanup runs. A moved island keeps its instance. |
| AC8 | A `data-leptos-props` change updates the same instance. The component keeps its local state. |
| AC9 | `idle` and `visible` wait for their trigger. Teardown cancels a pending trigger. |
| AC10 | Bad props or a failed mount affect one island only. The island gets `data-leptos-state="error"` and sends `autumn:leptos:error`. The fallback stays. Bad props on a mounted island keep the last good render. An unknown name is `pending` while a bundle loads, then `error` with its fallback; a later bundle can still mount it. A bundle that cannot load (import error, SRI mismatch, ABI mismatch, CSP block) writes a console error and sends `autumn:leptos:error` on `document`. Other bundles keep working. |
| AC11 | A Rust panic affects its own bundle only. Each island of that bundle gets its fallback back, `error` state and an `autumn:leptos:error` event. Only the bundle can signal its panic (a private callback). The loader makes no call into a crashed bundle. |
| AC12 | `data-leptos-ignore` and nested islands do not mount. One element mounts one time only. A second loader copy does nothing. The loader uses no `eval`, `Function`, `innerHTML` or `document.write`. It resists DOM clobbering. It imports only same-origin `modulepreload` bundle links with SRI hashes, outside ignored regions and islands. |
| AC13 | `wasm_csp()` gives the Autumn default CSP plus `'wasm-unsafe-eval'` in `script-src`. `add_wasm_unsafe_eval(policy)` adds the token to any policy. A second call does not change the result. It keeps the other directives. `WasmCspLoader` adds the token to the policy that a `ConfigLoader` loads. |
| AC14 | With the `ssr` feature, `Island::fallback_view(…)` renders a Leptos view on the server as the fallback. |
| AC15 | `PropsUpdate` sends new props in an `HX-Trigger` header. The header is visible ASCII. It merges with other trigger events and can use `HX-Trigger-After-Settle`. The loader applies the `autumn:leptos:props` event. |
| AC16 | The plugin passes `autumn_web::plugin_conformance` and declares a `PluginContract` for `autumn-web` 0.8. A second install is harmless. |
| AC17 | A runnable example, an example wasm crate and its committed bundle exist. The example runs with no wasm toolchain. An end-to-end browser test drives it with real htmx. |
| AC18 | `cargo fmt`, `cargo clippy` (pedantic, nursery, `-D warnings`), `cargo test`, doc tests and browser tests pass. Rust line coverage is 85% or more. |
| AC19 | README, CHANGELOG, ADR, CLAUDE.md, doc comments and a CI workflow exist. Text uses ASD-STE100. |

## 9. Review round 1

Four review agents checked the work: loader runtime, security, Rust and
Leptos API, and tests with docs and CI. The fixes:

- **Panics.** The panic signal is now a private callback, not a DOM
  event, so page code cannot crash a bundle. The first `register`
  installs the hook, so a panic before `start` marks the bundle as
  failed. After a panic, the client refuses mounts and updates and leaks
  dropped handles. The loader keeps crashed handles alive (the glue frees
  a lost handle with a call into wasm). `mount` and `update` check for a
  crash during their own call.
- **Security.** The loader accepts only same-origin `modulepreload`
  bundle links with SRI hashes, outside ignored regions and islands.
  `leptos_bundle` preloads snippets with SRI. `Island::attr` refuses
  `hx-on*`. `WasmCspLoader` refuses Autumn nonce mode.
- **Leptos.** The client crate turns on `leptos/csr`. Without it, effects
  do not run. `()` props read `{}`.
- **Lifecycle.** The loader keeps a fallback copy for htmx history. An
  unknown name gets `error` when no bundle loads. An `update` event never
  follows a later error. `Island::attr` matches names in any case.
- **Tests.** New browser tests cover the attribute branches, old browsers,
  glue SRI, refused links, `after_settle`, spoofed panics and crashes
  during calls. Fixed sleeps became sentinels. The browser tests use
  `WasmCspLoader`. The client tests check the signal value.
- **CI and docs.** CI lints the wasm crates on `wasm32`. The bundle hash
  includes `build.sh` and the root `Cargo.toml`. The README setup is
  complete. ASD-STE100 rewrites.

Not changed, with the reason:

- Two bundles that register one name: the bundle that loads first gets
  the name. A fix by document order adds much code for an app bug. The
  README tells apps to register a name in one bundle only.
- Server constants for the loader event names: more API for little gain.
  The README lists the names.
- A crashed bundle can still run Leptos timers and listeners. Wasm has no
  way to stop them from outside. The docs say this.

## 10. Not in scope

- Hydration of server HTML (see ADR 0001).
- An `autumn generate leptos` command. It needs a change in `autumn-cli`.
- Rebuild of the wasm bundle in the Autumn dev loop.
- An entry in the Autumn plugin index. It lists first-party crates only.
