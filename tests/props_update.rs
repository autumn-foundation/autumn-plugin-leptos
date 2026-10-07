//! `PropsUpdate` sends new island props in an `HX-Trigger` header.

#![allow(clippy::expect_used, clippy::panic)]

use autumn_plugin_leptos::{JsonKind, PROPS_EVENT, PropsError, PropsUpdate};
use autumn_web::htmx::HxResponseExt as _;
use autumn_web::prelude::*;
use autumn_web::reexports::axum::response::Response;
use autumn_web::reexports::http::{HeaderMap, HeaderValue};
use autumn_web::test::TestApp;

#[test]
fn header_value_is_an_event_with_a_list_of_updates() {
    let update = PropsUpdate::new()
        .set("#cart", &serde_json::json!({ "count": 2 }))
        .expect("props")
        .set("#badge", &serde_json::json!({ "n": 1 }))
        .expect("props");
    assert_eq!(PROPS_EVENT, "autumn:leptos:props");
    assert_eq!(
        update.to_header_value(),
        r##"{"autumn:leptos:props":[{"target":"#cart","props":{"count":2}},{"target":"#badge","props":{"n":1}}]}"##
    );
    assert_eq!(update.len(), 2);
    assert!(!update.is_empty());
}

#[test]
fn header_value_is_visible_ascii() {
    let update = PropsUpdate::new()
        .set(
            "#é",
            &serde_json::json!({ "name": "Zoë 🍁", "nl": "a\nb", "del": "\u{7f}" }),
        )
        .expect("props");
    let value = update.to_header_value();
    assert!(value.bytes().all(|b| (0x20..0x7f).contains(&b)), "{value}");
    // The JSON still decodes to the same text.
    let back: serde_json::Value = serde_json::from_str(&value).expect("json");
    let first = &back["autumn:leptos:props"][0];
    assert_eq!(first["target"], "#é");
    assert_eq!(first["props"]["name"], "Zoë 🍁");
    assert_eq!(first["props"]["nl"], "a\nb");
    assert_eq!(first["props"]["del"], "\u{7f}");
}

#[test]
fn set_refuses_props_that_are_not_an_object() {
    let error = PropsUpdate::new().set("#x", &[1, 2]).expect_err("array");
    assert!(matches!(error, PropsError::NotAnObject(JsonKind::Array)));
}

#[test]
fn empty_update_is_empty() {
    let update = PropsUpdate::default();
    assert!(update.is_empty());
    assert_eq!(update.len(), 0);
    assert_eq!(update.to_header_value(), r#"{"autumn:leptos:props":[]}"#);
}

#[get("/bump")]
async fn bump() -> AutumnResult<(PropsUpdate, &'static str)> {
    let update =
        PropsUpdate::new().set("#cart", &serde_json::json!({ "count": 3, "who": "Zoë" }))?;
    Ok((update, "ok"))
}

#[get("/empty")]
async fn empty() -> (PropsUpdate, &'static str) {
    (PropsUpdate::new(), "ok")
}

#[tokio::test]
async fn a_handler_returns_the_update_as_a_response_part() {
    let client = TestApp::new().routes(routes![bump, empty]).build();
    let response = client.get("/bump").send().await;
    response.assert_ok();
    let header = response.header("hx-trigger").expect("HX-Trigger header");
    assert_eq!(
        header,
        r##"{"autumn:leptos:props":[{"target":"#cart","props":{"count":3,"who":"Zo\u00eb"}}]}"##
    );
    assert_eq!(response.text(), "ok");
    // An empty update sends no header.
    let response = client.get("/empty").send().await;
    response.assert_ok();
    assert!(response.header("hx-trigger").is_none());
}

#[test]
fn raw_json_with_whitespace_still_gives_valid_ascii_json() {
    let raw = serde_json::value::RawValue::from_string(
        "{\n  \"a\": \"é\",\n  \"b\" : [1, 2]\n}".to_owned(),
    )
    .expect("raw");
    let value = PropsUpdate::new()
        .set("#x", &raw)
        .expect("props")
        .to_header_value();
    assert!(value.bytes().all(|b| (0x20..0x7f).contains(&b)), "{value}");
    assert!(
        !value.contains(' '),
        "no whitespace outside strings: {value}"
    );
    let back: serde_json::Value = serde_json::from_str(&value).expect("valid JSON");
    assert_eq!(back["autumn:leptos:props"][0]["props"]["a"], "é");
    assert_eq!(back["autumn:leptos:props"][0]["props"]["b"][1], 2);
}

#[test]
fn spaces_inside_strings_are_kept() {
    let value = PropsUpdate::new()
        .set("#a b", &serde_json::json!({ "t": "x  y\t\\\"" }))
        .expect("props")
        .to_header_value();
    let back: serde_json::Value = serde_json::from_str(&value).expect("valid JSON");
    assert_eq!(back["autumn:leptos:props"][0]["target"], "#a b");
    assert_eq!(back["autumn:leptos:props"][0]["props"]["t"], "x  y\t\\\"");
}

fn update() -> PropsUpdate {
    PropsUpdate::new()
        .set("#cart", &serde_json::json!({ "n": 1 }))
        .expect("props")
}

fn trigger(headers: &HeaderMap, name: &str) -> serde_json::Value {
    let text = headers.get(name).expect(name).to_str().expect("ascii");
    serde_json::from_str(text).expect("json")
}

#[test]
fn apply_to_merges_with_an_event_list() {
    let mut headers = HeaderMap::new();
    headers.insert("hx-trigger", HeaderValue::from_static("saved, closed"));
    update().apply_to(&mut headers);
    let json = trigger(&headers, "hx-trigger");
    assert_eq!(json["saved"], serde_json::Value::Null);
    assert_eq!(json["closed"], serde_json::Value::Null);
    assert_eq!(json[PROPS_EVENT][0]["target"], "#cart");
}

#[test]
fn apply_to_merges_with_a_json_object_and_an_earlier_update() {
    let mut headers = HeaderMap::new();
    headers.insert(
        "hx-trigger",
        HeaderValue::from_static(
            r##"{"toast":{"text":"ok"},"autumn:leptos:props":[{"target":"#a","props":{}}]}"##,
        ),
    );
    update().apply_to(&mut headers);
    let json = trigger(&headers, "hx-trigger");
    assert_eq!(json["toast"]["text"], "ok");
    let list = json[PROPS_EVENT].as_array().expect("list");
    assert_eq!(list.len(), 2);
    assert_eq!(list[0]["target"], "#a");
    assert_eq!(list[1]["target"], "#cart");
}

#[test]
fn apply_to_replaces_a_header_that_is_not_json() {
    let mut headers = HeaderMap::new();
    headers.insert("hx-trigger", HeaderValue::from_static("{broken"));
    update().apply_to(&mut headers);
    let json = trigger(&headers, "hx-trigger");
    assert_eq!(json[PROPS_EVENT][0]["target"], "#cart");
}

#[test]
fn after_settle_uses_its_own_header() {
    let mut headers = HeaderMap::new();
    let update = update().after_settle();
    assert_eq!(update.header_name(), "hx-trigger-after-settle");
    update.apply_to(&mut headers);
    assert!(headers.get("hx-trigger").is_none());
    let json = trigger(&headers, "hx-trigger-after-settle");
    assert_eq!(json[PROPS_EVENT][0]["target"], "#cart");
    assert_eq!(PropsUpdate::new().header_name(), "hx-trigger");
}

#[get("/merged")]
async fn merged() -> AutumnResult<(PropsUpdate, Response)> {
    Ok((update(), "ok".hx_trigger("saved")))
}

#[tokio::test]
async fn a_response_part_merges_with_the_body_trigger() {
    let client = TestApp::new().routes(routes![merged]).build();
    let response = client.get("/merged").send().await;
    response.assert_ok();
    let json: serde_json::Value =
        serde_json::from_str(response.header("hx-trigger").expect("header")).expect("json");
    assert_eq!(json["saved"], serde_json::Value::Null);
    assert_eq!(json[PROPS_EVENT][0]["target"], "#cart");
}
