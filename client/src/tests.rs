//! Native unit tests. The browser tests cover the DOM side with a real
//! wasm build.

use leptos::prelude::*;
use serde::Deserialize;

use super::{
    ABI_VERSION, IslandError, IslandHandle, PANICKED, autumn_leptos_abi, names, parse_props,
    prepare, register, start,
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
    assert_eq!(names(), Vec::<String>::new());
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
    assert_eq!(names(), Vec::<String>::new());
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
fn update_sets_the_signal_and_keeps_the_old_value_on_error() {
    assert!(register("Counter", counter));
    let prepared = prepare("Counter", r#"{"start":1}"#).expect("prepare");
    let value = || {
        prepared
            .signal
            .downcast_ref::<ArcRwSignal<Props>>()
            .expect("signal type")
            .get_untracked()
            .start
    };
    assert_eq!(value(), 1);
    assert!((prepared.update)(r#"{"start":2}"#).is_ok());
    assert_eq!(value(), 2);
    let error = (prepared.update)("{").expect_err("bad");
    assert!(matches!(error, IslandError::BadProps { .. }), "{error:?}");
    assert_eq!(value(), 2);
}

#[test]
fn start_gives_the_names() {
    assert_eq!(autumn_leptos_abi(), ABI_VERSION);
    assert!(register("Counter", counter));
    assert_eq!(start(None), Ok(vec!["Counter".to_owned()]));
}

#[test]
fn unit_props_read_an_empty_object() {
    // Without `data-leptos-props`, the loader sends `{}`.
    assert!(register("Plain", |_: Signal<()>| view! { <i /> }));
    assert!(prepare("Plain", "{}").is_ok());
    assert!(prepare("Plain", r#"{"x":1}"#).is_err());
}

#[test]
fn a_panic_stops_start_mount_and_update() {
    assert!(register("Counter", counter));
    let prepared = prepare("Counter", r#"{"start":1}"#).expect("prepare");
    let handle = IslandHandle {
        handle: Some(Box::new(())),
        update: prepared.update,
    };
    // `register` installed the hook. A panic in this thread sets the flag.
    let result = std::panic::catch_unwind(|| panic!("boom"));
    assert!(result.is_err());
    assert!(
        PANICKED
            .with(|p| p.borrow().clone())
            .is_some_and(|m| m.contains("boom"))
    );
    assert!(matches!(start(None), Err(IslandError::Panicked(m)) if m.contains("boom")));
    assert!(matches!(
        prepare("Counter", "{}"),
        Err(IslandError::Panicked(_))
    ));
    assert!(matches!(
        (handle.update)("{\"start\":3}"),
        Err(IslandError::Panicked(_))
    ));
    // A drop after a panic leaks the Leptos handle. It makes no call into
    // Leptos.
    drop(handle);
}

#[test]
fn a_handle_updates_and_unmounts() {
    assert!(register("Counter", counter));
    let prepared = prepare("Counter", r#"{"start":1}"#).expect("prepare");
    let handle = IslandHandle {
        handle: Some(Box::new(())),
        update: prepared.update,
    };
    assert!((handle.update)(r#"{"start":5}"#).is_ok());
    handle.unmount();
}
