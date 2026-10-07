//! [`PropsUpdate`]: new island props from the server, without a remount.

use std::convert::Infallible;

use autumn_web::reexports::axum::response::{IntoResponseParts, ResponseParts};
use autumn_web::reexports::http::{HeaderMap, HeaderName, HeaderValue};
use serde::Serialize;

use crate::island::{PropsError, props_json};

/// The DOM event that the loader reads new props from.
pub const PROPS_EVENT: &str = "autumn:leptos:props";

const HX_TRIGGER: &str = "hx-trigger";
const HX_TRIGGER_AFTER_SETTLE: &str = "hx-trigger-after-settle";

/// New props for mounted islands, sent in an `HX-Trigger` header.
///
/// htmx sends the [`PROPS_EVENT`] event. The loader sets
/// `data-leptos-props` on the first element that matches each selector.
/// The same instance gets the new props signal value, so the component
/// keeps its state.
///
/// ```rust
/// use autumn_plugin_leptos::PropsUpdate;
/// use autumn_web::prelude::*;
///
/// #[post("/cart")]
/// async fn add_to_cart() -> AutumnResult<(PropsUpdate, Markup)> {
///     let update = PropsUpdate::new().set("#cart", &serde_json::json!({ "count": 3 }))?;
///     Ok((update, html! { p { "Added." } }))
/// }
/// ```
///
/// - htmx handles `HX-Trigger` before the swap. To update an island that
///   the same response swaps in, use [`after_settle`](Self::after_settle).
/// - The update merges with a trigger header that the response already
///   has. If your code sets the header later with `insert`, that header
///   replaces the update. In this case, call
///   [`apply_to`](Self::apply_to) after your code sets the header.
/// - Props travel in a response header. Keep them small: proxies limit
///   header size (nginx: 4 KB to 8 KB by default).
/// - Build selectors from trusted values. A selector from user input can
///   select a different island.
///
/// An empty update sends no header.
#[derive(Debug, Clone, Default)]
#[must_use]
pub struct PropsUpdate {
    /// `(selector, props JSON)` pairs, in call order.
    entries: Vec<(String, String)>,
    after_settle: bool,
}

impl PropsUpdate {
    /// Makes an empty update.
    pub const fn new() -> Self {
        Self {
            entries: Vec::new(),
            after_settle: false,
        }
    }

    /// Adds new props for the island at the CSS `selector`.
    ///
    /// # Errors
    ///
    /// The same errors as [`Island::props`](crate::Island::props).
    pub fn set<T: Serialize + ?Sized>(
        mut self,
        selector: impl Into<String>,
        props: &T,
    ) -> Result<Self, PropsError> {
        self.entries.push((selector.into(), props_json(props)?));
        Ok(self)
    }

    /// Sends the update in `HX-Trigger-After-Settle`. htmx then sends the
    /// event after the swap, so the update can reach a new island.
    pub const fn after_settle(mut self) -> Self {
        self.after_settle = true;
        self
    }

    /// The header that carries the update: `hx-trigger`, or
    /// `hx-trigger-after-settle`.
    #[must_use]
    pub const fn header_name(&self) -> &'static str {
        if self.after_settle {
            HX_TRIGGER_AFTER_SETTLE
        } else {
            HX_TRIGGER
        }
    }

    /// The number of updates.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.entries.len()
    }

    /// `true` when the update has no entries.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The header value:
    /// `{"autumn:leptos:props":[{"target":"#cart","props":{…}}]}`.
    ///
    /// The text is visible ASCII. Other characters become `\uXXXX`
    /// escapes, because a header value cannot hold them.
    #[must_use]
    pub fn to_header_value(&self) -> String {
        self.merged_value(&serde_json::Map::new(), &[])
    }

    /// Writes the update into `headers`. It keeps the events that the
    /// header already has: a JSON object, or a comma list of event names.
    /// It replaces a value that is neither.
    pub fn apply_to(&self, headers: &mut HeaderMap) {
        if self.is_empty() {
            return;
        }
        let name = HeaderName::from_static(self.header_name());
        let (events, earlier) = headers
            .get(&name)
            .and_then(|value| value.to_str().ok())
            .map(existing_events)
            .unwrap_or_default();
        // `ascii_json` makes the value valid, so `from_str` cannot fail.
        if let Ok(value) = HeaderValue::from_str(&self.merged_value(&events, &earlier)) {
            headers.insert(name, value);
        }
    }

    /// The header value with other `events` and `earlier` updates first.
    fn merged_value(
        &self,
        events: &serde_json::Map<String, serde_json::Value>,
        earlier: &[serde_json::Value],
    ) -> String {
        let mut json = String::from("{");
        for (event, detail) in events {
            json.push_str(&serde_json::Value::String(event.clone()).to_string());
            json.push(':');
            json.push_str(&detail.to_string());
            json.push(',');
        }
        json.push('"');
        json.push_str(PROPS_EVENT);
        json.push_str("\":[");
        let mut first = true;
        for value in earlier {
            if !first {
                json.push(',');
            }
            first = false;
            json.push_str(&value.to_string());
        }
        for (selector, props) in &self.entries {
            if !first {
                json.push(',');
            }
            first = false;
            json.push_str("{\"target\":");
            json.push_str(&serde_json::Value::String(selector.clone()).to_string());
            json.push_str(",\"props\":");
            json.push_str(props);
            json.push('}');
        }
        json.push_str("]}");
        ascii_json(&json)
    }
}

/// Splits an existing trigger header into other events and earlier props
/// updates. A value that is neither JSON nor an event list gives nothing.
fn existing_events(
    value: &str,
) -> (
    serde_json::Map<String, serde_json::Value>,
    Vec<serde_json::Value>,
) {
    let value = value.trim();
    let mut events = if value.starts_with('{') {
        match serde_json::from_str(value) {
            Ok(serde_json::Value::Object(map)) => map,
            _ => serde_json::Map::new(),
        }
    } else {
        value
            .split(',')
            .map(str::trim)
            .filter(|event| !event.is_empty())
            .map(|event| (event.to_owned(), serde_json::Value::Null))
            .collect()
    };
    let earlier = match events.remove(PROPS_EVENT) {
        Some(serde_json::Value::Array(list)) => list,
        _ => Vec::new(),
    };
    (events, earlier)
}

const HEX: [char; 16] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
];

fn push_escape(out: &mut String, c: char) {
    for unit in c.encode_utf16(&mut [0; 2]) {
        out.push_str("\\u");
        for shift in [12, 8, 4, 0] {
            out.push(HEX[usize::from((*unit >> shift) & 0xf)]);
        }
    }
}

/// Makes compact, visible-ASCII JSON from valid JSON text.
///
/// Outside strings it drops whitespace (raw JSON can keep newlines). Inside
/// strings it escapes each character outside visible ASCII as `\uXXXX`.
/// The JSON value does not change.
fn ascii_json(json: &str) -> String {
    let mut out = String::with_capacity(json.len());
    let mut in_string = false;
    let mut escaped = false;
    for c in json.chars() {
        if in_string {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == '"' {
                in_string = false;
            }
            if (' '..='~').contains(&c) {
                out.push(c);
            } else {
                push_escape(&mut out, c);
            }
        } else if c == '"' {
            in_string = true;
            out.push(c);
        } else if !c.is_ascii_whitespace() {
            out.push(c);
        }
    }
    out
}

impl IntoResponseParts for PropsUpdate {
    type Error = Infallible;

    fn into_response_parts(self, mut res: ResponseParts) -> Result<ResponseParts, Self::Error> {
        self.apply_to(res.headers_mut());
        Ok(res)
    }
}
