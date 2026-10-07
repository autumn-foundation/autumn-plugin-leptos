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
//! | `autumn_leptos_start(on_panic)` | Keeps the panic callback. Returns the names. |
//! | `autumn_leptos_mount(name, element, props)` | Mounts one island. |
//! | `AutumnLeptosIsland.update(props)` | Sets new props. |
//! | `AutumnLeptosIsland.unmount()` | Unmounts the island. |
//!
//! # Props
//!
//! `P` is a `serde` type. Props are always a JSON object. Without a
//! `data-leptos-props` attribute, the props are `{}`. Use `()` or an empty
//! struct for a component with no props.
//!
//! # Panics
//!
//! A Rust panic leaves the wasm instance in an unknown state. The first
//! [`register`] installs a panic hook. The hook writes the message to the
//! console and calls the panic callback of the loader. The loader then
//! puts back the fallback of each island of this bundle. After a panic,
//! this crate refuses each mount and props update, and a dropped handle
//! makes no call into Leptos. Timers and event listeners that Leptos
//! installed can still run.

use std::any::{Any, TypeId};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use leptos::prelude::*;
use serde::de::DeserializeOwned;
use wasm_bindgen::prelude::*;

/// The ABI version. The loader refuses a bundle with another version.
pub const ABI_VERSION: u32 = 1;

/// The error from a mount or a props update. The loader gets it as a
/// JavaScript `Error` with this message.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub(crate) enum IslandError {
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
    /// The instance panicked before. Its state is unknown.
    #[error("the wasm bundle panicked before: {0}")]
    Panicked(String),
}

type Updater = Box<dyn Fn(&str) -> Result<(), IslandError>>;
type Mounter = Box<dyn FnOnce(web_sys::HtmlElement) -> Box<dyn Any>>;
type Factory = Rc<dyn Fn(&str) -> Result<Prepared, IslandError>>;

/// An island with checked props, before the mount.
pub(crate) struct Prepared {
    pub(crate) mount: Mounter,
    pub(crate) update: Updater,
    /// The `ArcRwSignal<P>`, for tests.
    #[cfg(test)]
    pub(crate) signal: Box<dyn Any>,
}

thread_local! {
    static REGISTRY: RefCell<BTreeMap<String, Factory>> = RefCell::new(BTreeMap::new());
    /// The panic callback of the loader.
    static ON_PANIC: RefCell<Option<js_sys::Function>> = const { RefCell::new(None) };
    /// The message of the first panic.
    pub(crate) static PANICKED: RefCell<Option<String>> = const { RefCell::new(None) };
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
    install_panic_hook();
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
                check_panic()?;
                signal.set(parse_props(&name, text)?);
                Ok(())
            })
        };
        #[cfg(test)]
        let test_signal: Box<dyn Any> = Box::new(signal.clone());
        let view = Rc::clone(&view);
        let mount: Mounter = Box::new(move |element| {
            let handle = leptos::mount::mount_to(element, move || view(Signal::from(signal)));
            Box::new(handle)
        });
        Ok(Prepared {
            mount,
            update,
            #[cfg(test)]
            signal: test_signal,
        })
    });
    REGISTRY.with(|r| r.borrow_mut().insert(name.to_owned(), factory));
    true
}

/// The registered names, sorted.
#[must_use]
pub fn names() -> Vec<String> {
    REGISTRY.with(|r| r.borrow().keys().cloned().collect())
}

/// Reads props text into `P`. The text must be a JSON object. For `P` =
/// `()`, it must be `{}`.
pub(crate) fn parse_props<P: DeserializeOwned + 'static>(
    name: &str,
    text: &str,
) -> Result<P, IslandError> {
    let bad = |message: String| IslandError::BadProps {
        name: name.to_owned(),
        message,
    };
    // `serde` reads a struct from a JSON array too. Props are objects.
    if !text.trim_start().starts_with('{') {
        return Err(bad("props must be a JSON object".to_owned()));
    }
    if TypeId::of::<P>() == TypeId::of::<()>() {
        // `serde` reads `()` from `null` only.
        let map: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(text).map_err(|error| bad(error.to_string()))?;
        if !map.is_empty() {
            return Err(bad("this component takes no props".to_owned()));
        }
        return serde_json::from_str("null").map_err(|error| bad(error.to_string()));
    }
    serde_json::from_str(text).map_err(|error| bad(error.to_string()))
}

/// Returns [`IslandError::Panicked`] after a panic.
fn check_panic() -> Result<(), IslandError> {
    PANICKED
        .with(|p| p.borrow().clone())
        .map_or(Ok(()), |message| Err(IslandError::Panicked(message)))
}

/// Checks the name and the props. It does not touch the DOM.
pub(crate) fn prepare(name: &str, props: &str) -> Result<Prepared, IslandError> {
    check_panic()?;
    // Release the borrow before the factory runs.
    let factory = REGISTRY.with(|r| r.borrow().get(name).cloned());
    factory.ok_or_else(|| IslandError::UnknownName(name.to_owned()))?(props)
}

/// A mounted island. A drop unmounts it and runs the Leptos cleanup.
#[wasm_bindgen(js_name = AutumnLeptosIsland)]
pub struct IslandHandle {
    /// The Leptos `UnmountHandle`. `None` only during `drop`.
    handle: Option<Box<dyn Any>>,
    update: Updater,
}

impl Drop for IslandHandle {
    fn drop(&mut self) {
        // After a panic, the Leptos state is unknown. Leak the handle and
        // make no call into Leptos.
        if PANICKED.with(|p| p.borrow().is_some()) {
            std::mem::forget(self.handle.take());
        }
    }
}

#[wasm_bindgen(js_class = AutumnLeptosIsland)]
impl IslandHandle {
    /// Sets new props. On an error, the old props stay.
    ///
    /// # Errors
    ///
    /// Bad props, or a panic before, as a JavaScript `Error`.
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

/// ABI: keeps the panic callback of the loader and returns the registered
/// names.
///
/// # Errors
///
/// A panic before this call (for example, in an async task of the start
/// function), as a JavaScript `Error`. The loader then marks the bundle as
/// failed.
#[wasm_bindgen]
pub fn autumn_leptos_start(on_panic: js_sys::Function) -> Result<Vec<String>, JsError> {
    start(Some(on_panic)).map_err(JsError::from)
}

/// Keeps the panic callback and returns the names.
pub(crate) fn start(on_panic: Option<js_sys::Function>) -> Result<Vec<String>, IslandError> {
    check_panic()?;
    ON_PANIC.with(|c| *c.borrow_mut() = on_panic);
    Ok(names())
}

/// ABI: mounts the island `name` into `element` with the JSON `props`.
///
/// # Errors
///
/// An unknown name, bad props or a panic before, as a JavaScript `Error`.
/// The DOM does not change.
#[wasm_bindgen]
pub fn autumn_leptos_mount(
    name: &str,
    element: web_sys::HtmlElement,
    props: &str,
) -> Result<IslandHandle, JsError> {
    let prepared = prepare(name, props)?;
    Ok(IslandHandle {
        handle: Some((prepared.mount)(element)),
        update: prepared.update,
    })
}

fn install_panic_hook() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            previous(info);
            on_panic(&info.to_string());
        }));
    });
}

/// Keeps the first panic message. In a browser, writes it to the console
/// and calls the panic callback of the loader.
fn on_panic(message: &str) {
    let _ = PANICKED.try_with(|p| {
        if let Ok(mut p) = p.try_borrow_mut() {
            p.get_or_insert_with(|| message.to_owned());
        }
    });
    // Only a browser has a console and a loader.
    if !cfg!(target_arch = "wasm32") {
        return;
    }
    let text = JsValue::from_str(&format!("autumn-leptos: {message}"));
    web_sys::console::error_1(&text);
    let callback = ON_PANIC
        .try_with(|c| c.try_borrow().ok().and_then(|c| c.clone()))
        .ok()
        .flatten();
    if let Some(callback) = callback {
        let _ = callback.call1(&JsValue::NULL, &JsValue::from_str(message));
    }
}

#[cfg(test)]
mod tests;
