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
- Without `data-leptos-props`, the props are `{}`. For a component with
  no props, use `()` or an empty struct.
- `register` returns `false` when the name is empty or already
  registered. The first registration stays.
- The crate exports ABI version 1 (`autumn_leptos_*`) through
  wasm-bindgen. The loader of the server crate calls it.
- The first `register` installs a panic hook. The hook calls a private
  callback of the loader. The islands of the bundle then show their
  fallback. After a panic, the crate refuses new mounts and updates.

Build with `wasm-bindgen --target web`. See the main README for the full
setup.

## License

Apache-2.0.
