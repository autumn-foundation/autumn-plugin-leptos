//! The wasm side of `autumn-plugin-leptos`: register Leptos components as
//! islands.
//!
//! Call [`register`] in the start function of your wasm crate:
//!
//! ```rust,no_run
//! use autumn_plugin_leptos_client::register;
//! use leptos::prelude::*;
//! use serde::Deserialize;
//!
//! #[derive(Clone, Deserialize)]
//! struct CounterProps {
//!     start: i32,
//! }
//!
//! fn counter(props: Signal<CounterProps>) -> impl IntoView {
//!     let (count, set_count) = signal(props.get_untracked().start);
//!     view! { <button on:click=move |_| set_count.update(|n| *n += 1)>{count}</button> }
//! }
//!
//! // In the app: `#[wasm_bindgen(start)]`.
//! pub fn start() {
//!     register("Counter", counter);
//! }
//! ```
//!
//! Then build with `wasm-bindgen --target web`. The glue module exports
//! the ABI of this crate. The loader of `autumn-plugin-leptos` calls it:
//!
//! | Export | Use |
//! | --- | --- |
//! | `autumn_leptos_abi()` | The ABI version, [`ABI_VERSION`]. |
//! | `autumn_leptos_start(bundle)` | Installs the panic hook. Returns the names. |
//! | `autumn_leptos_mount(name, element, props)` | Mounts one island. |
//! | `AutumnLeptosIsland.update(props)` | Sets new props. |
//! | `AutumnLeptosIsland.unmount()` | Unmounts the island. |
//!
//! # Panics
//!
//! A Rust panic stops the wasm instance. The panic hook writes the message
//! to the console and sends [`PANIC_EVENT`] on `document`. The loader then
//! puts back the fallback of each island of this bundle.

use std::any::Any;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use leptos::prelude::*;
use serde::de::DeserializeOwned;
use wasm_bindgen::prelude::*;

/// The ABI version. The loader refuses a bundle with another version.
pub const ABI_VERSION: u32 = 1;

/// The DOM event that the panic hook sends on `document`. `detail` has
/// `bundle` (the glue URL) and `message`.
pub const PANIC_EVENT: &str = "autumn:leptos:panic";

/// The error from a mount or a props update.
///
/// The loader gets it as a JavaScript `Error` with this message.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum IslandError {
    /// No component has this name.
    #[error("no island component is registered as `{0}`")]
    UnknownName(String),
    /// The props are not a JSON object of the props type.
    #[error("island `{name}` has bad props: {message}")]
    BadProps {
        /// The component name.
        name: String,
        /// The reason.
        message: String,
    },
}

type Updater = Box<dyn Fn(&str) -> Result<(), IslandError>>;
type Mounter = Box<dyn FnOnce(web_sys::HtmlElement) -> Box<dyn Any>>;
type Factory = Rc<dyn Fn(&str) -> Result<Prepared, IslandError>>;

/// An island with checked props, before the mount.
pub(crate) struct Prepared {
    pub(crate) mount: Mounter,
    pub(crate) update: Updater,
}

thread_local! {
    static REGISTRY: RefCell<BTreeMap<String, Factory>> = RefCell::new(BTreeMap::new());
    static BUNDLE: RefCell<String> = const { RefCell::new(String::new()) };
}

/// Registers a component as the island `name`.
///
/// `view` gets the props as a `Signal<P>`. A props update from the server
/// sets the signal. The component keeps its state. Without a
/// `data-leptos-props` attribute, the props are `{}`.
///
/// Returns `false`, and changes nothing, when `name` is empty or is
/// already registered. The first registration stays.
pub fn register<P, V, F>(name: &str, view: F) -> bool
where
    P: DeserializeOwned + Send + Sync + 'static,
    V: IntoView + 'static,
    F: Fn(Signal<P>) -> V + 'static,
{
    if name.is_empty() || REGISTRY.with(|r| r.borrow().contains_key(name)) {
        return false;
    }
    let view = Rc::new(view);
    let owned = name.to_owned();
    let factory: Factory = Rc::new(move |text| {
        let signal = ArcRwSignal::new(parse_props::<P>(&owned, text)?);
        let update: Updater = {
            let (signal, name) = (signal.clone(), owned.clone());
            Box::new(move |text| {
                signal.set(parse_props(&name, text)?);
                Ok(())
            })
        };
        let view = Rc::clone(&view);
        let mount: Mounter = Box::new(move |element| {
            let handle = leptos::mount::mount_to(element, move || view(Signal::from(signal)));
            Box::new(handle)
        });
        Ok(Prepared { mount, update })
    });
    REGISTRY.with(|r| r.borrow_mut().insert(name.to_owned(), factory));
    true
}

/// The registered names, sorted.
#[must_use]
pub fn names() -> Vec<String> {
    REGISTRY.with(|r| r.borrow().keys().cloned().collect())
}

/// Reads props text into `P`. The text must be a JSON object.
pub(crate) fn parse_props<P: DeserializeOwned>(name: &str, text: &str) -> Result<P, IslandError> {
    let bad = |message: String| IslandError::BadProps {
        name: name.to_owned(),
        message,
    };
    // `serde` reads a struct from a JSON array too. Props are objects.
    if !text.trim_start().starts_with('{') {
        return Err(bad("props must be a JSON object".to_owned()));
    }
    serde_json::from_str(text).map_err(|error| bad(error.to_string()))
}

/// Checks the name and the props. It does not touch the DOM.
pub(crate) fn prepare(name: &str, props: &str) -> Result<Prepared, IslandError> {
    // Release the borrow before the factory runs.
    let factory = REGISTRY.with(|r| r.borrow().get(name).cloned());
    factory.ok_or_else(|| IslandError::UnknownName(name.to_owned()))?(props)
}

/// A mounted island. A drop unmounts it and runs the Leptos cleanup.
#[wasm_bindgen(js_name = AutumnLeptosIsland)]
pub struct IslandHandle {
    _handle: Box<dyn Any>,
    update: Updater,
}

#[wasm_bindgen(js_class = AutumnLeptosIsland)]
impl IslandHandle {
    /// Sets new props. On an error, the old props stay.
    ///
    /// # Errors
    ///
    /// [`IslandError::BadProps`], as a JavaScript `Error`.
    pub fn update(&self, props: &str) -> Result<(), JsError> {
        (self.update)(props).map_err(JsError::from)
    }

    /// Unmounts the island and runs the Leptos cleanup.
    pub fn unmount(self) {
        drop(self);
    }
}

/// ABI: the version, [`ABI_VERSION`].
#[wasm_bindgen]
#[must_use]
#[allow(
    clippy::missing_const_for_fn,
    reason = "wasm-bindgen cannot export a const fn"
)]
pub fn autumn_leptos_abi() -> u32 {
    ABI_VERSION
}

/// ABI: keeps the bundle URL for panic reports, installs the panic hook
/// and returns the registered names.
#[wasm_bindgen]
#[must_use]
pub fn autumn_leptos_start(bundle: String) -> Vec<String> {
    BUNDLE.with(|b| *b.borrow_mut() = bundle);
    install_panic_hook();
    names()
}

/// ABI: mounts the island `name` into `element` with the JSON `props`.
///
/// # Errors
///
/// [`IslandError`], as a JavaScript `Error`. The DOM does not change.
#[wasm_bindgen]
pub fn autumn_leptos_mount(
    name: &str,
    element: web_sys::HtmlElement,
    props: &str,
) -> Result<IslandHandle, JsError> {
    let prepared = prepare(name, props)?;
    Ok(IslandHandle {
        _handle: (prepared.mount)(element),
        update: prepared.update,
    })
}

fn install_panic_hook() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            previous(info);
            // Only a browser has a console and a `document`.
            if cfg!(target_arch = "wasm32") {
                report_panic(&info.to_string());
            }
        }));
    });
}

/// Writes the panic to the console and sends [`PANIC_EVENT`].
fn report_panic(message: &str) {
    web_sys::console::error_1(&JsValue::from_str(&format!("autumn-leptos: {message}")));
    let bundle = BUNDLE
        .try_with(|b| b.try_borrow().map(|b| b.clone()).unwrap_or_default())
        .unwrap_or_default();
    let detail = js_sys::Object::new();
    let _ = js_sys::Reflect::set(&detail, &"bundle".into(), &bundle.into());
    let _ = js_sys::Reflect::set(&detail, &"message".into(), &message.into());
    let init = web_sys::CustomEventInit::new();
    init.set_detail(&detail);
    let event = web_sys::CustomEvent::new_with_event_init_dict(PANIC_EVENT, &init);
    if let (Ok(event), Some(document)) = (event, web_sys::window().and_then(|w| w.document())) {
        let _ = document.dispatch_event(&event);
    }
}

#[cfg(test)]
mod tests;
