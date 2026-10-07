//! `Island::fallback_view` renders a Leptos view on the server.

#![cfg(feature = "ssr")]
#![allow(clippy::expect_used, clippy::panic)]

use autumn_plugin_leptos::Island;
use leptos::prelude::*;
use maud::Render as _;

#[component]
fn Greeting(name: String, count: u32) -> impl IntoView {
    let (n, _set_n) = signal(count);
    view! { <p class="greeting">"Hello, " {name} "! " <b>{move || n.get()}</b></p> }
}

#[test]
fn fallback_is_the_server_html_of_the_view() {
    let html = Island::new("Greeting")
        .id("g")
        .fallback_view(|| view! { <Greeting name="Zoë".to_owned() count=3 /> })
        .render()
        .into_string();
    assert!(
        html.starts_with(r#"<div data-leptos-island="Greeting" id="g"><p class="greeting">"#),
        "{html}"
    );
    assert!(html.contains("Hello, "), "{html}");
    assert!(html.contains("Zoë"), "{html}");
    assert!(html.contains("<b>3</b>"), "{html}");
    assert!(html.ends_with("</p></div>"), "{html}");
}

#[test]
fn view_text_is_escaped() {
    let html = Island::new("X")
        .fallback_view(|| view! { <p>{"<script>alert(1)</script>"}</p> })
        .render()
        .into_string();
    assert!(!html.contains("<script>"), "{html}");
    assert!(html.contains("&lt;script&gt;"), "{html}");
}

#[test]
fn a_later_fallback_replaces_the_view() {
    let html = Island::new("X")
        .fallback_view(|| view! { <p>"view"</p> })
        .fallback(maud::html! { i { "plain" } })
        .render()
        .into_string();
    assert_eq!(html, r#"<div data-leptos-island="X"><i>plain</i></div>"#);
}
