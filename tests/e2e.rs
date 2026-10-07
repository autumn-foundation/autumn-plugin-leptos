//! End to end: the demo app, its real wasm bundle and real htmx in
//! Chromium.
//!
//! Run: `cargo test --features ssr --test e2e -- --ignored --test-threads=1`.

#![cfg(feature = "ssr")]
#![allow(clippy::expect_used, clippy::panic)]

#[path = "../examples/leptos_demo.rs"]
#[allow(dead_code)]
mod demo;

use std::time::Duration;

use autumn_plugin_leptos::wasm_csp;
use autumn_web::config::AutumnConfig;
use autumn_web::reexports::axum;
use autumn_web::system_test::{Page, SystemTest};
use autumn_web::test::TestApp;

fn app() -> TestApp {
    let mut config = AutumnConfig::default();
    config.security.headers.content_security_policy = wasm_csp();
    TestApp::new()
        .config(config)
        .plugin(demo::plugin())
        .routes(demo::routes())
}

async fn wait_for(page: &Page, js: &str) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        if page
            .evaluate(js)
            .await
            .ok()
            .and_then(|r| r.into_value::<bool>().ok())
            == Some(true)
        {
            return;
        }
        assert!(tokio::time::Instant::now() < deadline, "timed out: {js}");
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

#[tokio::test]
async fn the_server_renders_the_leptos_view_as_the_fallback() {
    let response = app().build().get("/").send().await;
    response.assert_ok();
    let html = response.text();
    assert!(html.contains(r#"data-leptos-island="Counter""#), "{html}");
    assert!(
        html.contains(r#"<div class="counter">"#),
        "SSR view: {html}"
    );
    assert!(html.contains("leptos_demo_islands"), "bundle tags: {html}");
}

#[tokio::test]
#[ignore = "requires Chromium"]
async fn the_demo_works_end_to_end() {
    let router = app().build().into_router();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        axum::serve(listener, router).await.expect("serve");
    });
    let runner = SystemTest::attach(format!("http://{addr}"))
        .await
        .expect("Chromium");
    let page = runner.page().await.expect("page");
    page.visit("/").await.expect("visit");
    page.expect_attribute("#counter", "data-leptos-state", "mounted")
        .await
        .expect("mounted");
    page.click("#counter button").await.expect("click");
    page.expect_text("Visits: 4").await.expect("reactive");
    // `PropsUpdate` through htmx: a new label, the same count.
    page.click("#rename").await.expect("click");
    page.expect_text("Hits: 4").await.expect("props update");
    // htmx adds a clock island, then removes it.
    page.click("#add-clock").await.expect("click");
    page.expect_attribute("#clock", "data-leptos-state", "mounted")
        .await
        .expect("clock mounted");
    page.click("#remove-clock").await.expect("click");
    wait_for(&page, "document.getElementById('clock') === null").await;
    page.expect_no_console_errors()
        .await
        .expect("clean console");
}
