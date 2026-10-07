//! Browser tests: the island loader in real headless Chromium.
//!
//! Run: `cargo test --test browser -- --ignored --test-threads=1`.
//! The tests need Chromium (see `autumn_web::system_test`).
//!
//! `FAKE`, `OTHER`, `DUP` and `BADABI` are fake glue modules with the
//! client ABI. `REAL` is the demo bundle: real Leptos 0.8 in wasm.

// Test helpers stop the test on a failure.
#![allow(clippy::expect_used, clippy::panic)]

use std::time::Duration;

use autumn_plugin_leptos::{
    Island, LeptosPlugin, MountWhen, PropsUpdate, leptos_bundle, leptos_script, wasm_csp,
};
use autumn_web::assets::PluginAssets;
use autumn_web::config::AutumnConfig;
use autumn_web::prelude::*;
use autumn_web::reexports::axum;
use autumn_web::reexports::axum::extract::Path;
use autumn_web::system_test::{Page, SystemTest, SystemTestRunner};
use autumn_web::test::TestApp;
use serde::de::DeserializeOwned;

const WASM: &[u8] = include_bytes!("fixtures/fake_bg.wasm");

static FAKE: PluginAssets = PluginAssets::from_files(
    "leptos-fake",
    &[
        ("fake.js", include_bytes!("fixtures/fake.js")),
        ("fake_bg.wasm", WASM),
    ],
);

static OTHER: PluginAssets = PluginAssets::from_files(
    "leptos-other",
    &[
        ("other.js", include_bytes!("fixtures/other.js")),
        ("other_bg.wasm", WASM),
    ],
);

static DUP: PluginAssets = PluginAssets::from_files(
    "leptos-dup",
    &[
        ("dup.js", include_bytes!("fixtures/dup.js")),
        ("dup_bg.wasm", WASM),
    ],
);

static BADABI: PluginAssets = PluginAssets::from_files(
    "leptos-badabi",
    &[
        ("badabi.js", include_bytes!("fixtures/badabi.js")),
        ("badabi_bg.wasm", WASM),
    ],
);

/// The demo bundle: real Leptos components, built from
/// `examples/islands-app/`.
static REAL: PluginAssets = PluginAssets::from_files(
    "leptos-real",
    &[
        (
            "leptos_demo_islands.js",
            include_bytes!("../examples/islands/leptos_demo_islands.js"),
        ),
        (
            "leptos_demo_islands_bg.wasm",
            include_bytes!("../examples/islands/leptos_demo_islands_bg.wasm"),
        ),
    ],
);

fn page_with(head: &Markup, body: &Markup) -> Markup {
    html! {
        (maud::DOCTYPE)
        html {
            head {
                script src="/probe.js" {}
                script src="/static/js/htmx.min.js" defer {}
                (head)
            }
            body { (body) }
        }
    }
}

/// A page with the loader and the `FAKE` bundle.
fn page(body: &Markup) -> Markup {
    page_with(&html! { (leptos_script()) (leptos_bundle(&FAKE)) }, body)
}

fn echo(id: &str, key: &str) -> Island {
    Island::new("Echo")
        .id(id)
        .props(&serde_json::json!({ "k": key }))
        .expect("props")
        .fallback(html! { i { "fallback " (key) } })
}

#[get("/probe.js")]
async fn probe_js() -> ([(&'static str, &'static str); 1], &'static str) {
    (
        [("content-type", "text/javascript")],
        include_str!("fixtures/probe.js"),
    )
}

#[get("/empty")]
async fn empty() -> Markup {
    html! { "cleared" }
}

#[get("/fake")]
async fn fake() -> Markup {
    page(&html! {
        (echo("e1", "1"))
        div #other {}
    })
}

#[get("/real")]
async fn real() -> AutumnResult<Markup> {
    let counter = Island::new("Counter")
        .id("counter")
        .props(&serde_json::json!({ "start": 3, "label": "Hits" }))?
        .fallback(html! { p { "Hits: 3 (server)" } });
    let clock = Island::new("Clock")
        .id("clock")
        .fallback(html! { p { "clock (server)" } });
    let boom = Island::new("Boom")
        .id("boom")
        .fallback(html! { p { "boom (server)" } });
    Ok(page_with(
        &html! { (leptos_script()) (leptos_bundle(&REAL)) (leptos_bundle(&FAKE)) },
        &html! {
            (counter)
            div #clock-slot { (clock) }
            button #clear-clock hx-get="/empty" hx-target="#clock-slot" { "Clear" }
            (boom)
            (echo("e1", "fake"))
        },
    ))
}

#[get("/swap")]
async fn swap() -> Markup {
    page(&html! {
        button #next hx-get="/fragment/2" hx-target="#slot" { "Next" }
        button #clear hx-get="/empty" hx-target="#slot" { "Clear" }
        div #slot { (echo("i1", "1")) }
        div #other {}
    })
}

#[get("/fragment/{n}")]
async fn fragment(Path(n): Path<u32>) -> Markup {
    html! { (echo(&format!("i{n}"), &n.to_string())) }
}

#[get("/lazy")]
async fn lazy() -> Markup {
    page(&html! {
        (echo("idle", "idle").mount_when(MountWhen::Idle))
        div style="height: 5000px" {}
        (echo("visible", "visible").mount_when(MountWhen::Visible))
        (echo("gone", "gone").mount_when(MountWhen::Visible))
        (echo("idle-gone", "idle-gone").mount_when(MountWhen::Idle))
    })
}

#[get("/errors")]
async fn errors() -> Markup {
    page(&html! {
        (Island::new("Broken").id("broken").fallback(html! { i { "broken fallback" } }))
        (Island::new("Partial").id("partial").fallback(html! { i { "partial fallback" } }))
        div #badprops data-leptos-island="Echo" data-leptos-props="{" { i { "bad fallback" } }
        div #array data-leptos-island="Echo" data-leptos-props="[1]" { i { "array fallback" } }
        (Island::new("Nobody").id("unknown").fallback(html! { i { "unknown fallback" } }))
        (echo("ok", "ok"))
    })
}

#[get("/panic")]
async fn panic_page() -> Markup {
    page_with(
        &html! { (leptos_script()) (leptos_bundle(&FAKE)) (leptos_bundle(&OTHER)) },
        &html! {
            (echo("a", "a"))
            (echo("b", "b"))
            (Island::new("Other").id("o").fallback(html! { i { "other fallback" } }))
            (Island::new("Panic").id("p").mount_when(MountWhen::Idle)
                .fallback(html! { i { "panic fallback" } }))
        },
    )
}

#[get("/order")]
async fn order() -> Markup {
    // Bundle first, loader second, loader again. Named elements clobber
    // globals.
    page_with(
        &html! { (leptos_bundle(&FAKE)) (leptos_script()) (leptos_script()) },
        &html! {
            div #autumnLeptos {}
            form name="documentElement" {}
            (echo("e", "order"))
        },
    )
}

#[get("/ignore")]
async fn ignore() -> Markup {
    page(&html! {
        div #zone data-leptos-ignore { (echo("ignored", "ignored")) }
        div #outer data-leptos-island="Echo" data-leptos-props="{\"k\":\"outer\"}" {
            (echo("inner", "inner"))
        }
        (echo("free", "free"))
    })
}

#[get("/failures")]
async fn failures() -> Markup {
    let fake = FAKE.get("fake.js").expect("glue");
    page_with(
        &html! {
            (leptos_script())
            (leptos_bundle(&BADABI))
            // The SRI hash does not match the wasm bytes.
            link rel="modulepreload" href=(fake.url()) integrity=(fake.integrity())
                crossorigin="anonymous" data-leptos-wasm=(FAKE.url("fake_bg.wasm"))
                data-leptos-wasm-integrity="sha384-AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
            link rel="modulepreload" href="/missing/glue.js" data-leptos-wasm="/missing/glue_bg.wasm";
            (leptos_bundle(&OTHER))
        },
        &html! {
            (Island::new("Bad").id("bad").fallback(html! { i { "bad fallback" } }))
            (Island::new("Echo").id("echo").fallback(html! { i { "echo fallback" } }))
            (Island::new("Other").id("o").fallback(html! { i { "other fallback" } }))
        },
    )
}

#[get("/late")]
async fn late() -> Markup {
    page(&html! {
        (Island::new("Late").id("late").fallback(html! { i { "late fallback" } }))
        (echo("e", "first"))
    })
}

#[get("/dup-link")]
async fn dup_link() -> Markup {
    leptos_bundle(&DUP)
}

#[get("/props")]
async fn props_page() -> Markup {
    page(&html! {
        button #bump hx-get="/bump" hx-swap="none" { "Bump" }
        (echo("basket", "0"))
    })
}

#[get("/bump")]
async fn bump() -> AutumnResult<(PropsUpdate, &'static str)> {
    let update = PropsUpdate::new().set("#basket", &serde_json::json!({ "k": "Zoë 🍁" }))?;
    Ok((update, ""))
}

#[get("/history")]
async fn history() -> Markup {
    // An htmx history restore brings back old island output.
    page(&html! {
        div #h data-leptos-island="Echo" data-leptos-props="{\"k\":\"h\"}"
            data-leptos-state="mounted" { span.echo { "stale" } }
    })
}

fn app(csp: Option<String>) -> TestApp {
    let mut config = AutumnConfig::default();
    if let Some(csp) = csp {
        config.security.headers.content_security_policy = csp;
    }
    TestApp::new()
        .config(config)
        .plugin(
            LeptosPlugin::new()
                .bundle(&FAKE)
                .bundle(&OTHER)
                .bundle(&DUP)
                .bundle(&BADABI)
                .bundle(&REAL),
        )
        .routes(routes![
            probe_js, empty, fake, real, swap, fragment, lazy, errors, panic_page, order, ignore,
            failures, late, dup_link, props_page, bump, history
        ])
}

/// Serves the app on a free port and opens Chromium on it.
async fn start_with(csp: Option<String>) -> SystemTestRunner {
    let router = app(csp).build().into_router();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        axum::serve(listener, router).await.expect("serve");
    });
    SystemTest::attach(format!("http://{addr}"))
        .await
        .expect("Chromium")
}

/// The app with the wasm CSP.
async fn start() -> SystemTestRunner {
    start_with(Some(wasm_csp())).await
}

async fn run(page: &Page, js: &str) {
    page.evaluate(js).await.expect("evaluate");
}

async fn eval<T: DeserializeOwned>(page: &Page, js: &str) -> T {
    page.evaluate(js)
        .await
        .expect("evaluate")
        .into_value()
        .expect("decode")
}

/// Polls `js` until it is `true`. Fails after 10 seconds.
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

/// The `data-leptos-state` of `#id`, or `""` when it has none.
async fn state(page: &Page, id: &str) -> String {
    eval(
        page,
        &format!("document.getElementById('{id}').getAttribute('data-leptos-state') || ''"),
    )
    .await
}

async fn wait_state(page: &Page, id: &str, value: &str) {
    wait_for(
        page,
        &format!(
            "document.getElementById('{id}')?.getAttribute('data-leptos-state') === '{value}'"
        ),
    )
    .await;
}

async fn text(page: &Page, id: &str) -> String {
    eval(
        page,
        &format!("document.getElementById('{id}').textContent"),
    )
    .await
}

async fn log(page: &Page) -> Vec<String> {
    eval(page, "window.__leptosLog || []").await
}

fn count(log: &[String], prefix: &str) -> usize {
    log.iter().filter(|line| line.starts_with(prefix)).count()
}

// AC6: a real Leptos component, from real wasm, with its props.
#[tokio::test]
#[ignore = "requires Chromium"]
async fn real_leptos_component_mounts_with_props_and_reacts() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/real").await.expect("visit");
    wait_state(&page, "counter", "mounted").await;
    page.expect_text("Hits: 3").await.expect("props");
    assert!(!text(&page, "counter").await.contains("(server)"));
    page.click("#counter button").await.expect("click");
    page.expect_text("Hits: 4").await.expect("reactive");
    let bundles: Vec<serde_json::Value> = eval(&page, "autumnLeptos.bundles()").await;
    assert!(bundles.iter().all(|b| b["state"] == "ready"), "{bundles:?}");
    let names: Vec<String> = eval(&page, "autumnLeptos.names()").await;
    for name in ["Boom", "Clock", "Counter", "Echo"] {
        assert!(names.iter().any(|n| n == name), "{name} in {names:?}");
    }
    page.expect_no_console_errors()
        .await
        .expect("clean console");
    let response = app(Some(wasm_csp())).build().get("/real").send().await;
    assert_eq!(
        response.header("content-security-policy"),
        Some(wasm_csp().as_str())
    );
}

// AC8 with real Leptos: new props reach the same instance.
#[tokio::test]
#[ignore = "requires Chromium"]
async fn real_props_update_keeps_the_component_state() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/real").await.expect("visit");
    wait_state(&page, "counter", "mounted").await;
    page.click("#counter button").await.expect("click");
    page.expect_text("Hits: 4").await.expect("clicked");
    run(
        &page,
        r#"document.getElementById('counter').setAttribute('data-leptos-props', '{"start":0,"label":"Score"}')"#,
    )
    .await;
    page.expect_text("Score: 4")
        .await
        .expect("new label, same count");
    // The update event fires after Leptos changes the DOM.
    wait_for(
        &page,
        "__leptosLog.some((l) => l.startsWith('update:Counter:') && l.includes('Score: 4'))",
    )
    .await;
    page.expect_no_console_errors()
        .await
        .expect("clean console");
}

// AC7 with real Leptos: htmx removes an island, and Leptos cleanup runs.
#[tokio::test]
#[ignore = "requires Chromium"]
async fn htmx_swap_unmounts_a_real_component_and_runs_cleanup() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/real").await.expect("visit");
    wait_state(&page, "clock", "mounted").await;
    run(
        &page,
        "window.__logs = []; const l = console.log; \
         console.log = (...a) => { __logs.push(a.join(' ')); l(...a); }",
    )
    .await;
    page.click("#clear-clock").await.expect("click");
    page.expect_text("cleared").await.expect("swapped");
    wait_for(
        &page,
        "__leptosLog.some((l) => l.startsWith('unmount:Clock'))",
    )
    .await;
    wait_for(&page, "__logs.some((l) => l.includes('clock stopped'))").await;
    page.expect_no_console_errors()
        .await
        .expect("clean console");
}

// AC11 with real Leptos: a panic puts the fallbacks of its bundle back.
#[tokio::test]
#[ignore = "requires Chromium"]
async fn a_real_panic_crashes_only_its_own_bundle() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/real").await.expect("visit");
    wait_state(&page, "boom", "mounted").await;
    wait_state(&page, "e1", "mounted").await;
    page.click("#boom button").await.expect("click");
    wait_state(&page, "counter", "error").await;
    wait_state(&page, "boom", "error").await;
    assert_eq!(text(&page, "counter").await, "Hits: 3 (server)");
    assert_eq!(text(&page, "boom").await, "boom (server)");
    // The fake bundle keeps working.
    assert_eq!(state(&page, "e1").await, "mounted");
    let bundles: Vec<serde_json::Value> = eval(&page, "autumnLeptos.bundles()").await;
    let crashed: Vec<_> = bundles.iter().filter(|b| b["state"] == "crashed").collect();
    assert_eq!(crashed.len(), 1, "{bundles:?}");
    assert!(
        crashed[0]["url"]
            .as_str()
            .is_some_and(|u| u.contains("leptos_demo_islands")),
        "{bundles:?}"
    );
    let errors = page.console_errors();
    assert!(
        errors.iter().any(|e| e.contains("boom")),
        "the panic message is in the console: {errors:?}"
    );
}

// AC10: without `'wasm-unsafe-eval'` the fallback stays.
#[tokio::test]
#[ignore = "requires Chromium"]
async fn the_default_csp_blocks_wasm_and_keeps_the_fallback() {
    let runner = start_with(None).await;
    let page = runner.page().await.expect("page");
    page.visit("/real").await.expect("visit");
    wait_for(
        &page,
        "autumnLeptos.bundles().some((b) => b.state === 'failed')",
    )
    .await;
    assert_eq!(text(&page, "counter").await, "Hits: 3 (server)");
    assert_eq!(state(&page, "counter").await, "pending");
    wait_for(&page, "__leptosLog.includes('error:bundle:')").await;
    // The fake bundle has no real wasm, so the CSP does not stop it.
    wait_state(&page, "e1", "mounted").await;
}

// AC6/AC10: the fake ABI bundle mounts with its props; the wasm fetch
// passes SRI.
#[tokio::test]
#[ignore = "requires Chromium"]
async fn fake_bundle_mounts_after_init_and_start() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/fake").await.expect("visit");
    wait_state(&page, "e1", "mounted").await;
    assert_eq!(text(&page, "e1").await, r#"{"k":"1"}"#);
    let log = log(&page).await;
    let init = log.iter().position(|l| l == "init").expect("init");
    let start = log.iter().position(|l| l == "start").expect("start");
    let mount = log
        .iter()
        .position(|l| l.starts_with("mount:"))
        .expect("mount");
    assert!(init < start && start < mount, "{log:?}");
    assert_eq!(count(&log, "mount:Echo:"), 1, "{log:?}");
    page.expect_no_console_errors()
        .await
        .expect("clean console");
}

// AC7: htmx swaps islands in and out. A moved island keeps its instance.
#[tokio::test]
#[ignore = "requires Chromium"]
async fn htmx_swaps_mount_and_unmount_and_moves_keep_the_instance() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/swap").await.expect("visit");
    wait_state(&page, "i1", "mounted").await;
    let first: String = eval(
        &page,
        "document.querySelector('#i1 .echo').dataset.instance",
    )
    .await;
    // Move: remove and add in one task.
    run(
        &page,
        "document.getElementById('other').appendChild(document.getElementById('i1'))",
    )
    .await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    let moved: String = eval(
        &page,
        "document.querySelector('#i1 .echo').dataset.instance",
    )
    .await;
    assert_eq!(moved, first, "same instance after a move");
    assert_eq!(count(&log(&page).await, "destroy:"), 0);
    // Swap in a new island.
    page.click("#next").await.expect("click");
    wait_state(&page, "i2", "mounted").await;
    assert_eq!(text(&page, "i2").await, r#"{"k":"2"}"#);
    // Swap out the moved island's sibling slot, then the moved island.
    page.click("#clear").await.expect("click");
    page.expect_text("cleared").await.expect("cleared");
    wait_for(&page, "__leptosLog.includes('destroy:{\"k\":\"2\"}')").await;
    run(&page, "document.getElementById('i1').remove()").await;
    wait_for(&page, "__leptosLog.includes('destroy:{\"k\":\"1\"}')").await;
    let log = log(&page).await;
    assert_eq!(count(&log, "unmount:Echo"), 2, "{log:?}");
    page.expect_no_console_errors()
        .await
        .expect("clean console");
}

// AC8, AC10: a props change updates the same instance. Bad props keep the
// last good render.
#[tokio::test]
#[ignore = "requires Chromium"]
async fn props_changes_update_the_same_instance() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/fake").await.expect("visit");
    wait_state(&page, "e1", "mounted").await;
    run(
        &page,
        r#"document.getElementById('e1').setAttribute('data-leptos-props', '{"k":"2"}')"#,
    )
    .await;
    wait_for(&page, "__leptosLog.includes('set:{\"k\":\"2\"}')").await;
    wait_for(&page, "__leptosLog.includes('update:Echo:{\"k\":\"2\"}')").await;
    let instance: String = eval(
        &page,
        "document.querySelector('#e1 .echo').dataset.instance",
    )
    .await;
    assert_eq!(instance, "1");
    // Bad props: error state, the last good render stays.
    run(
        &page,
        "document.getElementById('e1').setAttribute('data-leptos-props', '[1]')",
    )
    .await;
    wait_state(&page, "e1", "error").await;
    assert_eq!(text(&page, "e1").await, r#"{"k":"2"}"#);
    // Good props again: the same instance renders them.
    run(
        &page,
        r#"document.getElementById('e1').setAttribute('data-leptos-props', '{"k":"3"}')"#,
    )
    .await;
    wait_state(&page, "e1", "mounted").await;
    assert_eq!(text(&page, "e1").await, r#"{"k":"3"}"#);
    assert_eq!(count(&log(&page).await, "mount:"), 1);
}

// AC9: idle and visible wait for their trigger. Teardown cancels it.
#[tokio::test]
#[ignore = "requires Chromium"]
async fn idle_and_visible_wait_for_their_trigger() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/lazy").await.expect("visit");
    wait_for(
        &page,
        "autumnLeptos.bundles().every((b) => b.state === 'ready')",
    )
    .await;
    for id in ["idle", "visible", "gone", "idle-gone"] {
        assert_eq!(state(&page, id).await, "waiting", "{id}");
    }
    // Remove one of each before the trigger.
    run(
        &page,
        "document.getElementById('gone').remove(); document.getElementById('idle-gone').remove()",
    )
    .await;
    wait_for(
        &page,
        "window.__idleCancels === 1 && window.__unobserves >= 1",
    )
    .await;
    run(&page, "__runIdle()").await;
    wait_state(&page, "idle", "mounted").await;
    assert_eq!(state(&page, "visible").await, "waiting");
    run(&page, "document.getElementById('visible').scrollIntoView()").await;
    wait_state(&page, "visible", "mounted").await;
    let log = log(&page).await;
    assert_eq!(count(&log, "mount:"), 2, "{log:?}");
}

// AC10: errors stay in one island.
#[tokio::test]
#[ignore = "requires Chromium"]
async fn errors_affect_one_island_only() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/errors").await.expect("visit");
    wait_state(&page, "ok", "mounted").await;
    for (id, fallback) in [
        ("broken", "broken fallback"),
        ("partial", "partial fallback"),
        ("badprops", "bad fallback"),
        ("array", "array fallback"),
    ] {
        wait_state(&page, id, "error").await;
        assert_eq!(text(&page, id).await, fallback, "{id}");
    }
    assert_eq!(state(&page, "unknown").await, "pending");
    assert_eq!(text(&page, "unknown").await, "unknown fallback");
    let log = log(&page).await;
    for name in ["Broken", "Partial"] {
        assert_eq!(count(&log, &format!("error:{name}:")), 1, "{log:?}");
    }
    assert_eq!(count(&log, "error:Echo:"), 2, "{log:?}");
    let errors = page.console_errors();
    assert!(
        errors.iter().any(|e| e.contains("broken mount")),
        "{errors:?}"
    );
}

// AC11: a panic puts back the fallbacks of its bundle only.
#[tokio::test]
#[ignore = "requires Chromium"]
async fn a_panic_crashes_only_its_bundle() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/panic").await.expect("visit");
    wait_state(&page, "a", "mounted").await;
    wait_state(&page, "b", "mounted").await;
    wait_state(&page, "o", "mounted").await;
    run(&page, "__fakePanic()").await;
    for (id, fallback) in [("a", "fallback a"), ("b", "fallback b")] {
        wait_state(&page, id, "error").await;
        assert_eq!(text(&page, id).await, fallback);
    }
    assert_eq!(state(&page, "o").await, "mounted");
    // The loader makes no call into a crashed bundle.
    assert_eq!(count(&log(&page).await, "destroy:"), 0);
    // A later island of the crashed bundle does not mount.
    run(&page, "__runIdle()").await;
    wait_state(&page, "p", "error").await;
    assert_eq!(text(&page, "p").await, "panic fallback");
    let log = log(&page).await;
    assert_eq!(count(&log, "error:Echo:"), 2, "{log:?}");
}

// AC11: a panic during the mount.
#[tokio::test]
#[ignore = "requires Chromium"]
async fn a_panic_at_mount_crashes_the_bundle() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/panic").await.expect("visit");
    wait_state(&page, "a", "mounted").await;
    run(&page, "__runIdle()").await;
    wait_state(&page, "p", "error").await;
    wait_state(&page, "a", "error").await;
    assert_eq!(text(&page, "p").await, "panic fallback");
    assert_eq!(text(&page, "a").await, "fallback a");
    assert_eq!(state(&page, "o").await, "mounted");
    let log = log(&page).await;
    assert_eq!(count(&log, "error:Panic:"), 1, "{log:?}");
}

// AC12: script order, a second loader and clobbered globals.
#[tokio::test]
#[ignore = "requires Chromium"]
async fn order_second_loader_and_clobbering_are_harmless() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/order").await.expect("visit");
    wait_state(&page, "e", "mounted").await;
    tokio::time::sleep(Duration::from_millis(200)).await;
    let log = log(&page).await;
    assert_eq!(count(&log, "mount:"), 1, "{log:?}");
    assert_eq!(count(&log, "init"), 1, "{log:?}");
    let loader: bool = eval(&page, "window.autumnLeptos.loader === true").await;
    assert!(loader);
    page.expect_no_console_errors()
        .await
        .expect("clean console");
}

// AC12: ignored and nested islands do not mount.
#[tokio::test]
#[ignore = "requires Chromium"]
async fn ignored_and_nested_islands_do_not_mount() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/ignore").await.expect("visit");
    wait_state(&page, "free", "mounted").await;
    wait_state(&page, "outer", "mounted").await;
    assert_eq!(state(&page, "ignored").await, "");
    // The inner island is part of the outer fallback. It never mounts.
    let inner: bool = eval(&page, "document.getElementById('inner') === null").await;
    assert!(inner, "the outer island moved its fallback out");
    let log = log(&page).await;
    assert!(!log.iter().any(|l| l.contains("inner")), "{log:?}");
    // Remove the ignore marker: the island mounts.
    run(
        &page,
        "document.getElementById('zone').removeAttribute('data-leptos-ignore')",
    )
    .await;
    wait_state(&page, "ignored", "mounted").await;
    // Set it again: the island goes back to its fallback.
    run(
        &page,
        "document.getElementById('zone').setAttribute('data-leptos-ignore', '')",
    )
    .await;
    wait_for(
        &page,
        "!document.getElementById('ignored').hasAttribute('data-leptos-state')",
    )
    .await;
    assert_eq!(text(&page, "ignored").await, "fallback ignored");
}

// AC10: bundles that cannot load. Other bundles keep working.
#[tokio::test]
#[ignore = "requires Chromium"]
async fn bad_bundles_fail_alone() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/failures").await.expect("visit");
    wait_state(&page, "o", "mounted").await;
    wait_for(
        &page,
        "autumnLeptos.bundles().filter((b) => b.state === 'failed').length === 3",
    )
    .await;
    for (id, fallback) in [("bad", "bad fallback"), ("echo", "echo fallback")] {
        assert_eq!(state(&page, id).await, "pending", "{id}");
        assert_eq!(text(&page, id).await, fallback);
    }
    let log = log(&page).await;
    assert_eq!(count(&log, "error:bundle:"), 3, "{log:?}");
    assert_eq!(count(&log, "init"), 0, "SRI stops the wasm: {log:?}");
    let errors = page.console_errors();
    assert!(errors.iter().any(|e| e.contains("ABI")), "{errors:?}");
}

// AC10: a bundle link that comes later mounts pending islands. The first
// registration of a name stays.
#[tokio::test]
#[ignore = "requires Chromium"]
async fn a_late_bundle_mounts_pending_islands() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/late").await.expect("visit");
    wait_state(&page, "e", "mounted").await;
    assert_eq!(state(&page, "late").await, "pending");
    run(
        &page,
        "fetch('/dup-link').then((r) => r.text()).then((t) => { \
           const tpl = document.createElement('template'); \
           tpl.setHTMLUnsafe ? tpl.setHTMLUnsafe(t) : (tpl.innerHTML = t); \
           document.head.append(...tpl.content.childNodes); })",
    )
    .await;
    wait_state(&page, "late", "mounted").await;
    assert_eq!(text(&page, "late").await, "from dup: Late");
    // `Echo` stays with the first bundle.
    assert_eq!(text(&page, "e").await, r#"{"k":"first"}"#);
    let errors = page.console_errors();
    assert!(
        errors.iter().any(|e| e.contains("already registered")),
        "{errors:?}"
    );
}

// AC15: `PropsUpdate` through htmx.
#[tokio::test]
#[ignore = "requires Chromium"]
async fn props_update_header_reaches_the_island() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/props").await.expect("visit");
    wait_state(&page, "basket", "mounted").await;
    page.click("#bump").await.expect("click");
    wait_for(
        &page,
        "document.getElementById('basket').textContent === '{\"k\":\"Zoë 🍁\"}'",
    )
    .await;
    assert_eq!(count(&log(&page).await, "mount:"), 1);
    // App code can send the event too.
    run(
        &page,
        "document.getElementById('basket').dispatchEvent(new CustomEvent( \
           'autumn:leptos:props', { bubbles: true, detail: { props: { k: 'js' } } }))",
    )
    .await;
    wait_for(
        &page,
        "document.getElementById('basket').textContent === '{\"k\":\"js\"}'",
    )
    .await;
    page.expect_no_console_errors()
        .await
        .expect("clean console");
}

// History restore: old output goes away and the island mounts again.
#[tokio::test]
#[ignore = "requires Chromium"]
async fn restored_output_is_replaced() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/history").await.expect("visit");
    wait_for(
        &page,
        "document.getElementById('h').textContent === '{\"k\":\"h\"}'",
    )
    .await;
    let spans: u32 = eval(&page, "document.querySelectorAll('#h span').length").await;
    assert_eq!(spans, 1);
}

// Events: mount, update, unmount and error carry the name and element.
#[tokio::test]
#[ignore = "requires Chromium"]
async fn events_carry_name_and_element() {
    let runner = start().await;
    let page = runner.page().await.expect("page");
    page.visit("/fake").await.expect("visit");
    wait_state(&page, "e1", "mounted").await;
    run(
        &page,
        r#"window.__seen = []; ['mount','update','unmount','error'].forEach((t) =>
             document.addEventListener('autumn:leptos:' + t, (e) =>
               __seen.push(t + ':' + e.detail.name + ':' + (e.detail.element && e.detail.element.id))));
           const el = document.getElementById('e1');
           el.setAttribute('data-leptos-props', '{"k":"x"}');"#,
    )
    .await;
    wait_for(&page, "__seen.includes('update:Echo:e1')").await;
    run(&page, "document.getElementById('e1').remove()").await;
    wait_for(&page, "__seen.includes('unmount:Echo:e1')").await;
}
