//! Native unit tests. The browser tests cover the DOM side with a real
//! wasm build.

use leptos::prelude::*;
use serde::Deserialize;

use super::{
    ABI_VERSION, BUNDLE, IslandError, IslandHandle, autumn_leptos_abi, autumn_leptos_start, names,
    parse_props, prepare, register,
};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
struct Props {
    start: i32,
    #[serde(default)]
    label: String,
}

fn counter(props: Signal<Props>) -> impl IntoView {
    view! { <p>{move || props.get().label} {move || props.get().start}</p> }
}

#[test]
fn abi_version_is_one() {
    assert_eq!(ABI_VERSION, 1);
}

#[test]
fn register_adds_names_in_sorted_order() {
    assert!(names().is_empty());
    assert!(register("Counter", counter));
    assert!(register("Alpha", |_: Signal<Props>| view! { <i /> }));
    assert_eq!(names(), ["Alpha", "Counter"]);
}

#[test]
fn the_first_registration_of_a_name_stays() {
    assert!(register("Twice", counter));
    assert!(!register("Twice", |_: Signal<Props>| view! { <b /> }));
    assert_eq!(names(), ["Twice"]);
}

#[test]
fn an_empty_name_is_refused() {
    assert!(!register("", counter));
    assert!(names().is_empty());
}

#[test]
fn parse_reads_a_json_object_into_the_props_type() {
    let props: Props = parse_props("C", r#"{"start":3,"label":"Hits"}"#).expect("props");
    assert_eq!(
        props,
        Props {
            start: 3,
            label: "Hits".into()
        }
    );
    let props: Props = parse_props("C", " \n{\"start\":-1}").expect("props");
    assert_eq!(props.start, -1);
}

#[test]
fn parse_refuses_bad_json_and_wrong_types() {
    for text in ["", "{", r#"{"start":"x"}"#, "{}", "nope"] {
        let error = parse_props::<Props>("C", text).expect_err(text);
        assert!(
            matches!(&error, IslandError::BadProps { name, .. } if name == "C"),
            "{text}: {error:?}"
        );
        assert!(
            error.to_string().starts_with("island `C` has bad props: "),
            "{error}"
        );
    }
}

#[test]
fn parse_refuses_props_that_are_not_an_object() {
    // `serde` reads a struct from an array too. Props are always objects.
    for text in ["[3]", "3", "null", "\"s\"", "true"] {
        let error = parse_props::<Props>("C", text).expect_err(text);
        assert_eq!(
            error,
            IslandError::BadProps {
                name: "C".into(),
                message: "props must be a JSON object".into()
            },
            "{text}"
        );
    }
}

#[test]
fn prepare_refuses_an_unknown_name() {
    let error = prepare("Nobody", "{}").err().expect("unknown");
    assert_eq!(error, IslandError::UnknownName("Nobody".into()));
    assert_eq!(
        error.to_string(),
        "no island component is registered as `Nobody`"
    );
}

#[test]
fn prepare_checks_props_before_the_dom() {
    assert!(register("Counter", counter));
    let error = prepare("Counter", "[1]").err().expect("bad props");
    assert!(matches!(error, IslandError::BadProps { .. }), "{error:?}");
    assert!(prepare("Counter", r#"{"start":1}"#).is_ok());
}

#[test]
fn update_checks_props() {
    assert!(register("Counter", counter));
    let prepared = prepare("Counter", r#"{"start":1}"#).expect("prepare");
    assert!((prepared.update)(r#"{"start":2}"#).is_ok());
    let error = (prepared.update)("{").expect_err("bad");
    assert!(matches!(error, IslandError::BadProps { .. }), "{error:?}");
}

#[test]
fn abi_exports_give_the_version_and_the_names() {
    assert_eq!(autumn_leptos_abi(), ABI_VERSION);
    assert!(register("Counter", counter));
    assert_eq!(autumn_leptos_start("/glue.js".into()), ["Counter"]);
    assert_eq!(BUNDLE.with(|b| b.borrow().clone()), "/glue.js");
    // A second start installs no second hook.
    assert_eq!(autumn_leptos_start("/glue.js".into()), ["Counter"]);
}

#[test]
fn a_handle_updates_and_unmounts() {
    assert!(register("Counter", counter));
    let prepared = prepare("Counter", r#"{"start":1}"#).expect("prepare");
    let handle = IslandHandle {
        _handle: Box::new(()),
        update: prepared.update,
    };
    assert!(handle.update(r#"{"start":5}"#).is_ok());
    handle.unmount();
}
