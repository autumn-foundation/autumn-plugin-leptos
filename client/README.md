# autumn-plugin-leptos-client

The wasm side of
[autumn-plugin-leptos](https://github.com/autumn-foundation/autumn-plugin-leptos).
Register Leptos 0.8 components as islands for Autumn pages.

```rust,ignore
use autumn_plugin_leptos_client::register;
use leptos::prelude::*;
use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() {
    register("Counter", counter); // fn counter(props: Signal<CounterData>) -> impl IntoView
}
```

- The component gets its props as a `Signal<P>`. `P` is a `serde` type.
  New props set the signal. The component keeps its state.
- `register` returns `false` for an empty or a used name. The first
  registration stays.
- The crate exports ABI version 1 (`autumn_leptos_*`) through
  wasm-bindgen. The loader of the server crate calls it.
- A panic hook tells the loader about a panic. The islands of the bundle
  then show their fallback.

Build with `wasm-bindgen --target web`. See the main README for the full
setup.

## License

Apache-2.0.
