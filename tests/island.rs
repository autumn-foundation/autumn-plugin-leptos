//! `Island` renders the mount element. All values are escaped.

#![allow(clippy::expect_used, clippy::panic)]

use std::collections::BTreeMap;

use autumn_plugin_leptos::{AttrError, Island, JsonKind, MountWhen, PropsError};
use autumn_web::{AutumnError, Markup, html};
use maud::Render as _;
use serde::Serialize;

fn render(island: &Island) -> String {
    island.render().into_string()
}

#[test]
fn minimal_island_has_only_the_name() {
    assert_eq!(
        render(&Island::new("Counter")),
        r#"<div data-leptos-island="Counter"></div>"#
    );
}

#[test]
fn full_island_renders_every_attribute_in_a_fixed_order() {
    let island = Island::new("Counter")
        .props(&serde_json::json!({ "start": 3 }))
        .expect("props")
        .mount_when(MountWhen::Visible)
        .id("c1")
        .class("card wide")
        .fallback(html! { p { "Count: 3" } });
    assert_eq!(
        render(&island),
        r#"<div data-leptos-island="Counter" data-leptos-props="{&quot;start&quot;:3}" data-leptos-mount="visible" id="c1" class="card wide"><p>Count: 3</p></div>"#
    );
}

#[test]
fn mount_strategies_render_their_attribute() {
    let attr = |when| render(&Island::new("X").mount_when(when));
    assert!(!attr(MountWhen::Load).contains("data-leptos-mount"));
    assert!(attr(MountWhen::Idle).contains(r#"data-leptos-mount="idle""#));
    assert!(attr(MountWhen::Visible).contains(r#"data-leptos-mount="visible""#));
    assert_eq!(MountWhen::default(), MountWhen::Load);
}

#[test]
fn props_keep_struct_field_order() {
    #[derive(Serialize)]
    struct Props<'a> {
        b: u8,
        a: &'a str,
    }
    let island = Island::new("X")
        .props(&Props { b: 1, a: "z" })
        .expect("props");
    assert!(
        render(&island)
            .contains(r#"data-leptos-props="{&quot;b&quot;:1,&quot;a&quot;:&quot;z&quot;}""#),
        "{}",
        render(&island)
    );
}

#[test]
fn hostile_values_are_escaped() {
    let island = Island::new(r#"X" onload="alert(1)"#)
        .props(&serde_json::json!({ "s": "</div><script>alert(1)</script>'\"&" }))
        .expect("props")
        .id(r#"i" onclick="x"#)
        .class("<c>");
    let html = render(&island);
    assert!(!html.contains("<script"), "{html}");
    assert!(!html.contains(r#"" onload"#), "{html}");
    assert!(!html.contains(r#"" onclick"#), "{html}");
    assert!(html.contains("&lt;c&gt;"), "{html}");
    assert!(html.contains("&amp;"), "{html}");
}

#[test]
fn fallback_markup_is_kept_as_is() {
    let fallback: Markup = html! { span.x { "a & b" } };
    let html = render(&Island::new("X").fallback(fallback));
    assert!(
        html.ends_with(r#"><span class="x">a &amp; b</span></div>"#),
        "{html}"
    );
}

#[test]
fn island_is_usable_inside_html_macro() {
    let island = Island::new("X").id("in");
    let page = html! { main { (island) } }.into_string();
    assert_eq!(
        page,
        r#"<main><div data-leptos-island="X" id="in"></div></main>"#
    );
}

#[test]
fn non_object_props_are_refused_with_their_kind() {
    let cases: [(serde_json::Value, JsonKind, &str); 5] = [
        (serde_json::json!(null), JsonKind::Null, "null"),
        (serde_json::json!(true), JsonKind::Boolean, "a boolean"),
        (serde_json::json!(5), JsonKind::Number, "a number"),
        (serde_json::json!("s"), JsonKind::String, "a string"),
        (serde_json::json!([1]), JsonKind::Array, "an array"),
    ];
    for (value, kind, text) in cases {
        let error = Island::new("X").props(&value).expect_err("not an object");
        assert!(
            matches!(error, PropsError::NotAnObject(k) if k == kind),
            "{error:?}"
        );
        assert_eq!(
            error.to_string(),
            format!("island props must be a JSON object, not {text}")
        );
    }
    assert!(matches!(
        Island::new("X").props(&-1.5_f64),
        Err(PropsError::NotAnObject(JsonKind::Number))
    ));
}

#[test]
fn props_that_do_not_serialize_are_refused() {
    let mut map = BTreeMap::new();
    map.insert(vec![1_u8], 1_u8);
    let error = Island::new("X").props(&map).expect_err("bad key");
    assert!(matches!(error, PropsError::Serialize(_)), "{error:?}");
    assert!(
        error
            .to_string()
            .starts_with("island props do not serialize to JSON: ")
    );
    assert!(std::error::Error::source(&error).is_some());
}

#[test]
fn props_error_converts_to_autumn_error() {
    fn handler() -> Result<Island, AutumnError> {
        Ok(Island::new("X").props(&5)?)
    }
    let error = handler().expect_err("refused");
    assert_eq!(error.status().as_u16(), 500);
}

#[test]
fn later_calls_replace_earlier_values() {
    let island = Island::new("X")
        .id("a")
        .id("b")
        .class("a")
        .class("b")
        .props(&serde_json::json!({ "n": 1 }))
        .expect("props")
        .props(&serde_json::json!({ "n": 2 }))
        .expect("props")
        .mount_when(MountWhen::Idle)
        .mount_when(MountWhen::Load);
    assert_eq!(
        render(&island),
        r#"<div data-leptos-island="X" data-leptos-props="{&quot;n&quot;:2}" id="b" class="b"></div>"#
    );
}

#[test]
fn island_is_clone_and_debug() {
    let island = Island::new("X").id("d");
    let copy = island.clone();
    assert_eq!(render(&island), render(&copy));
    assert!(format!("{island:?}").contains("Island"));
    assert_eq!(island.name(), "X");
}

#[test]
fn a_proto_key_is_plain_data() {
    // Rust `serde` reads the props, not `JSON.parse`. The key is data.
    assert!(
        Island::new("X")
            .props(&serde_json::json!({ "__proto__": { "isAdmin": true } }))
            .is_ok()
    );
}

#[test]
fn extra_attributes_render_after_class_and_are_escaped() {
    let island = Island::new("X")
        .class("c")
        .attr("aria-label", r#"A "b" <c>"#)
        .expect("attr")
        .attr("role", "region")
        .expect("attr")
        .attr("hx-preserve", "")
        .expect("attr");
    assert_eq!(
        render(&island),
        r#"<div data-leptos-island="X" class="c" aria-label="A &quot;b&quot; &lt;c&gt;" role="region" hx-preserve=""></div>"#
    );
}

#[test]
fn a_repeated_attribute_keeps_the_last_value() {
    let island = Island::new("X")
        .attr("role", "a")
        .expect("attr")
        .attr("role", "b")
        .expect("attr");
    assert_eq!(
        render(&island),
        r#"<div data-leptos-island="X" role="b"></div>"#
    );
}

#[test]
fn bad_attribute_names_are_refused() {
    for name in ["", "a b", "a\"b", "a>b", "1a", "-a", "é"] {
        let error = Island::new("X").attr(name, "v").expect_err(name);
        assert!(
            matches!(error, AttrError::InvalidName(_)),
            "{name}: {error:?}"
        );
    }
    for name in [
        "id",
        "class",
        "ID",
        "data-leptos-island",
        "data-leptos-props",
        "DATA-LEPTOS-STATE",
        "onclick",
        "onLoad",
    ] {
        let error = Island::new("X").attr(name, "v").expect_err(name);
        assert!(matches!(error, AttrError::Reserved(_)), "{name}: {error:?}");
    }
    assert_eq!(
        AttrError::Reserved("id".into()).to_string(),
        "the island attribute `id` is reserved; use the island builder method or the loader"
    );
    assert_eq!(
        AttrError::InvalidName("a b".into()).to_string(),
        "`a b` is not a valid attribute name"
    );
}

#[test]
fn attr_error_converts_to_autumn_error() {
    fn handler() -> Result<Island, AutumnError> {
        Ok(Island::new("X").attr("onclick", "x")?)
    }
    assert_eq!(handler().expect_err("refused").status().as_u16(), 500);
}

#[test]
fn an_inline_island_is_a_span() {
    let island = Island::new("X").inline().fallback(html! { "f" });
    assert_eq!(render(&island), r#"<span data-leptos-island="X">f</span>"#);
}
