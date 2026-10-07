# autumn-plugin-leptos

Leptos islands for [Autumn](https://github.com/autumn-foundation/autumn)
0.8 apps. Put Leptos 0.8 components (Rust, compiled to wasm) into Maud +
htmx pages.

- The plugin serves a small loader and your wasm bundle through the Autumn
  `plugin_assets` API: hashed URLs, `immutable` cache, SRI. The loader
  also checks the SRI hash of the `.wasm` file.
- The client crate registers your components. You write no JavaScript.
- Props are typed `serde` structs. The component gets them as a
  `Signal<P>`. New props from the server keep the component state.
- The loader mounts islands that htmx swaps in, and unmounts islands that
  htmx swaps out. Leptos cleanup runs.
- A Rust panic affects only its own bundle. The islands of that bundle
  show their fallback again.
- With the `ssr` feature, the server renders the same component as the
  fallback.

## Install

```toml
# The server crate.
[dependencies]
autumn-plugin-leptos = { version = "0.1", features = ["ssr"] }

# The wasm crate (`crate-type = ["cdylib"]`).
[dependencies]
autumn-plugin-leptos-client = "0.1"
leptos = { version = "0.8", features = ["csr"] }
wasm-bindgen = "=0.2.129"
```

Install the wasm tools one time. The `wasm-bindgen-cli` version must be
the `wasm-bindgen` version in your `Cargo.lock`:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129
```

## Use

### 1. Write and register the components

```rust,ignore
// islands/src/lib.rs
use autumn_plugin_leptos_client::register;
use leptos::prelude::*;
use serde::Deserialize;
use wasm_bindgen::prelude::*;

#[derive(Clone, Deserialize)]
pub struct CounterData {
    pub start: i32,
    pub label: String,
}

pub fn counter(props: Signal<CounterData>) -> impl IntoView {
    let (count, set_count) = signal(props.get_untracked().start);
    view! {
        <span>{move || props.get().label} ": " {count}</span>
        <button on:click=move |_| set_count.update(|n| *n += 1)>"+1"</button>
    }
}

#[wasm_bindgen(start)]
pub fn start() {
    register("Counter", counter);
}
```

- Without `data-leptos-props`, the props are `{}`.
- `#[component] fn Counter` makes a `CounterProps` type. Do not give your
  props struct the same name.

### 2. Build the bundle

```sh
cargo build --release --target wasm32-unknown-unknown
wasm-bindgen --target web --no-typescript --out-dir static/islands \
  target/wasm32-unknown-unknown/release/islands.wasm
```

The output is a glue module (`islands.js`) and a wasm module
(`islands_bg.wasm`). Keep both in one bundle.
[`examples/islands-app/build.sh`](examples/islands-app/build.sh) is a
complete script.

### 3. Install the plugin and render islands

```rust,ignore
use autumn_plugin_leptos::{Island, LeptosPlugin, WasmCspLoader, leptos_bundle, leptos_script};
use autumn_web::assets::PluginAssets;
use autumn_web::prelude::*;

static ISLANDS: PluginAssets = PluginAssets::from_files(
    "app-islands",
    &[
        ("islands.js", include_bytes!("../static/islands/islands.js")),
        ("islands_bg.wasm", include_bytes!("../static/islands/islands_bg.wasm")),
    ],
);

#[get("/")]
async fn index() -> AutumnResult<Markup> {
    let counter = Island::new("Counter")
        .props(&serde_json::json!({ "start": 3, "label": "Hits" }))?
        .fallback(html! { p { "Hits: 3" } });
    Ok(html! {
        html {
            head { (leptos_script()) (leptos_bundle(&ISLANDS)) }
            body { (counter) }
        }
    })
}

#[autumn_web::main]
async fn main() {
    autumn_web::app()
        .with_config_loader(WasmCspLoader::default())
        .plugin(LeptosPlugin::new().bundle(&ISLANDS))
        .routes(routes![index])
        .run()
        .await;
}
```

- Give the same bundle to `LeptosPlugin::bundle` and `leptos_bundle`.
- `leptos_bundle` renders a `<link rel="modulepreload">` for the glue and
  a `<link rel="preload">` for the wasm. The loader imports the glue.
- `Island::inline()` renders a `<span>`. `Island::attr` adds attributes
  such as `role` or `aria-label`. It refuses `id`, `class`,
  `data-leptos-*` and `on*`.
- A bundle with no `x.js` + `x_bg.wasm` pair, or with the namespace
  `leptos`, stops the app at start-up.

### 4. Allow wasm in the CSP

A browser compiles wasm only when `script-src` has
`'wasm-unsafe-eval'`. This token does not allow `eval()`. The Autumn
default CSP does not have it. Use one of these:

- `WasmCspLoader` (above) adds the token to the policy from
  `autumn.toml` and `AUTUMN_*` variables. Wrap your own loader with
  `WasmCspLoader::new(loader)`.
- Put `wasm_csp()` (the default policy plus the token) in
  `security.headers.content_security_policy`.
- `add_wasm_unsafe_eval(policy)` adds the token to any policy.

An explicit policy turns off Autumn nonce injection. Without the token,
the islands keep their fallback.

### 5. Send new props from the server

```rust,ignore
use autumn_plugin_leptos::PropsUpdate;

#[post("/rename")]
async fn rename() -> AutumnResult<(PropsUpdate, Markup)> {
    let update = PropsUpdate::new().set("#counter", &serde_json::json!({ "start": 0, "label": "Score" }))?;
    Ok((update, html! {}))
}
```

`PropsUpdate` writes the `HX-Trigger` header. htmx sends the
`autumn:leptos:props` event. The loader sets `data-leptos-props`, and the
client sets the props signal of the same instance.

- htmx handles `HX-Trigger` before the swap. To update an island that the
  same response swaps in, use `.after_settle()`.
- Each selector updates the first matching island. Build selectors from
  trusted values. Keep header props small.

App code can send the same event:

```js
island.dispatchEvent(new CustomEvent("autumn:leptos:props", {
  bubbles: true,
  detail: { props: { start: 0, label: "Score" } },
}));
```

### 6. Render the fallback on the server (`ssr`)

```rust,ignore
let counter = Island::new("Counter")
    .props(&data)?
    .fallback_view(move || counter(Signal::stored(data)));
```

Share the component file between the server and the wasm crate (the
demo uses `#[path]`). The server HTML shows before the wasm loads. The
loader replaces it with the live component. There is no hydration.

## The island element

```html
<div data-leptos-island="Counter" data-leptos-props='{"start":3}'
     data-leptos-mount="visible">fallback</div>
```

| Attribute | Set by | Meaning |
| --- | --- | --- |
| `data-leptos-island` | `Island::new` | The registered name. A change mounts a new instance. |
| `data-leptos-props` | `Island::props` | A JSON object. A change sets the props signal. |
| `data-leptos-mount` | `Island::mount_when` | `idle` or `visible`. No attribute: mount at load. |
| `data-leptos-state` | the loader | `waiting`, `pending` (name not registered yet), `mounted` or `error`. |
| `data-leptos-ignore` | you | No island in this element mounts. |

The loader sends these events. They bubble from the island. An island
that left the page sends `unmount` on `document`. `detail` has `name` and
`element` (and `error` for `error`):

| Event | When |
| --- | --- |
| `autumn:leptos:mount` | The component mounts. |
| `autumn:leptos:update` | New props are in the DOM. |
| `autumn:leptos:unmount` | The component unmounts. |
| `autumn:leptos:error` | Bad props, a mount error or a panic. A bundle that cannot load sends it on `document` with `detail.bundle`. |

`window.autumnLeptos.names()` and `window.autumnLeptos.bundles()` show
the registered names and the bundle states (`loading`, `ready`, `failed`,
`crashed`).

## Behavior

- **Load.** The loader imports each glue module. It gives `init` a
  `fetch` of the `.wasm` with its SRI hash. Then it checks the ABI version
  of the client crate and reads the names.
- **Mount.** The loader moves the fallback out of the island. Then the
  client calls Leptos `mount_to`.
- **Unknown name.** The island stays `pending` and shows its fallback. A
  later bundle can register the name.
- **Errors.** Bad props or a failed mount keep the fallback. Bad props on
  a mounted island keep the last good render. A bundle that cannot load
  (import error, SRI mismatch, ABI mismatch, CSP) writes a console error.
  Each error affects one island or one bundle only.
- **Panics.** A Rust panic stops the wasm instance. The panic hook tells
  the loader. Each island of that bundle shows its fallback and gets
  `error`. The loader makes no more calls into the bundle. Split bundles
  to isolate components.
- **htmx.** One `MutationObserver` mounts added islands and unmounts
  removed islands. An island that moves in one task keeps its instance.
- **History.** After an htmx history restore, the loader removes the old
  output and mounts the island again.
- **Nesting.** An island inside another island does not mount.

## Security

- **User HTML.** The loader mounts each `data-leptos-island` element in
  the page. If your app shows user HTML, the sanitizer **must** remove
  `data-leptos-*`, `hx-*` and `data-hx-*` attributes. `data-leptos-ignore`
  alone is not sufficient: an htmx out-of-band swap can move an element
  out of it.
- **Props.** Props are in an HTML attribute. Maud escapes them. Rust
  `serde` reads them into your type. Do not put secrets in props.
- **Wasm.** `'wasm-unsafe-eval'` allows wasm compilation only. An island
  is first-party code with the full authority of the page. Do not use
  islands for untrusted code.
- **Updates.** Any same-origin script and any `HX-Trigger` header can send
  `autumn:leptos:props`. Do not copy user input into a trigger header.
- **Names.** The first registration of a name stays. A library bundle
  must use a prefix in its names (`Acme.Chart`).

## Demo

```sh
cargo run --features ssr --example leptos_demo
```

Open <http://127.0.0.1:3000>. The demo has a counter with an SSR fallback
and a props update, a clock that htmx adds and removes, and a panic
button. The bundle in `examples/islands/` is committed, so you need no
wasm toolchain to run it. Rebuild it with `examples/islands-app/build.sh`.

## Development

```sh
cargo test --workspace --all-features
cargo test --all-features --test browser --test e2e -- --ignored --test-threads=1
cargo llvm-cov --workspace --all-features --all-targets --summary-only
examples/islands-app/build.sh          # rebuild the demo bundle
examples/islands-app/build.sh --check  # is the bundle fresh?
```

See the [plan](docs/plan.md) and
[ADR 0001](docs/adr/0001-leptos-islands.md).

## License

Apache-2.0.
