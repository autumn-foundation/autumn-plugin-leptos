//! Property tests: the escape and props invariants hold for any input.

use autumn_plugin_leptos::Island;
use maud::Render as _;
use proptest::prelude::*;

/// Reverses Maud attribute escaping (`&`, `<`, `>`, `"`).
fn unescape(s: &str) -> String {
    s.replace("&quot;", "\"")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

/// Returns the raw value of `name="…"` in `html`.
fn attr<'a>(html: &'a str, name: &str) -> Option<&'a str> {
    let start = html.find(&format!(" {name}=\""))? + name.len() + 3;
    let len = html[start..].find('"')?;
    Some(&html[start..start + len])
}

proptest! {
    /// Any string map round-trips through the attribute without a change.
    #[test]
    fn props_round_trip(map in proptest::collection::btree_map(".*", ".*", 0..6)) {
        let html = Island::new("X").props(&map).unwrap().render().into_string();
        let raw = attr(&html, "data-leptos-props").unwrap();
        let back: std::collections::BTreeMap<String, String> =
            serde_json::from_str(&unescape(raw)).unwrap();
        prop_assert_eq!(back, map);
    }

    /// No input can open a tag or close an attribute early.
    #[test]
    fn values_cannot_break_out(name in ".*", id in ".*", class in ".*", value in ".*") {
        let html = Island::new(name.clone())
            .id(id.clone())
            .class(class.clone())
            .props(&serde_json::json!({ "v": value }))
            .unwrap()
            .render()
            .into_string();
        // One element: one `<` for the open tag, one for the close tag.
        prop_assert_eq!(html.matches('<').count(), 2, "{}", html);
        prop_assert_eq!(unescape(attr(&html, "data-leptos-island").unwrap()), name);
        prop_assert_eq!(unescape(attr(&html, "id").unwrap()), id);
        prop_assert_eq!(unescape(attr(&html, "class").unwrap()), class);
    }

    /// An extra attribute value cannot break out either.
    #[test]
    fn extra_attribute_values_cannot_break_out(value in ".*") {
        let html = Island::new("X")
            .attr("aria-label", value.clone())
            .unwrap()
            .render()
            .into_string();
        prop_assert_eq!(html.matches('<').count(), 2, "{}", html);
        prop_assert_eq!(unescape(attr(&html, "aria-label").unwrap()), value);
    }
}
