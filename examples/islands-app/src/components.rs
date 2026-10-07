//! The demo components. The wasm crate mounts them. The server demo
//! (`examples/leptos_demo.rs`) renders `counter` as the SSR fallback.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

/// The props of the counter island.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CounterData {
    /// The first count.
    pub start: i32,
    /// The text before the count.
    pub label: String,
}

/// No props.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NoProps {}

/// A counter. The count is local state. The label comes from the props, so
/// a props update keeps the count.
pub fn counter(props: Signal<CounterData>) -> impl IntoView {
    let (count, set_count) = signal(props.get_untracked().start);
    view! {
        <div class="counter">
            <span class="label">{move || props.get().label}</span>
            ": "
            <output>{count}</output>
            " "
            <button type="button" on:click=move |_| set_count.update(|n| *n += 1)>"+1"</button>
        </div>
    }
}

/// A clock that ticks each second. Its cleanup stops the timer.
pub fn clock(_props: Signal<NoProps>) -> impl IntoView {
    let (ticks, set_ticks) = signal(0_u32);
    // Effects run only in the browser.
    Effect::new(move |_| {
        let timer = set_interval_with_handle(
            move || set_ticks.update(|t| *t += 1),
            std::time::Duration::from_secs(1),
        )
        .ok();
        on_cleanup(move || {
            if let Some(timer) = timer {
                timer.clear();
            }
            leptos::logging::log!("autumn-leptos-demo: clock stopped");
        });
    });
    view! { <p class="clock">"Ticks: " {ticks}</p> }
}

/// A button that panics. The loader then shows the fallback of each island
/// of this bundle.
#[allow(clippy::panic, reason = "the demo panics on purpose")]
pub fn boom(_props: Signal<NoProps>) -> impl IntoView {
    view! {
        <button type="button" class="boom" on:click=move |_| panic!("boom: the demo panics on purpose")>
            "Panic"
        </button>
    }
}
