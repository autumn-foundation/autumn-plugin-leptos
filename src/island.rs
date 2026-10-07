//! [`Island`]: the server side of one Leptos island.

use std::fmt::Write as _;

use maud::{Escaper, Markup, PreEscaped, Render};
use serde::Serialize;

/// When the loader mounts an island.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MountWhen {
    /// Mount when the loader starts. This is the default.
    #[default]
    Load,
    /// Mount when the browser is idle (`requestIdleCallback`).
    Idle,
    /// Mount when the island comes into the viewport.
    Visible,
}

impl MountWhen {
    /// The `data-leptos-mount` value. `None` for [`MountWhen::Load`].
    const fn attr(self) -> Option<&'static str> {
        match self {
            Self::Load => None,
            Self::Idle => Some("idle"),
            Self::Visible => Some("visible"),
        }
    }
}

/// The kind of a JSON value that is not an object.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum JsonKind {
    /// `null`.
    Null,
    /// `true` or `false`.
    Boolean,
    /// A number.
    Number,
    /// A string.
    String,
    /// An array.
    Array,
}

impl std::fmt::Display for JsonKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Null => "null",
            Self::Boolean => "a boolean",
            Self::Number => "a number",
            Self::String => "a string",
            Self::Array => "an array",
        })
    }
}

/// The error from [`Island::props`].
///
/// It converts to `AutumnError` (status 500), so `?` works in a handler.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PropsError {
    /// The value does not serialize to JSON.
    #[error("island props do not serialize to JSON: {0}")]
    Serialize(#[from] serde_json::Error),
    /// The value is JSON, but not a JSON object.
    #[error("island props must be a JSON object, not {0}")]
    NotAnObject(JsonKind),
}

/// The error from [`Island::attr`].
///
/// It converts to `AutumnError` (status 500), so `?` works in a handler.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum AttrError {
    /// The name is empty, does not start with an ASCII letter, or has a
    /// character other than `A-Z a-z 0-9 - _ : .`.
    #[error("`{0}` is not a valid attribute name")]
    InvalidName(String),
    /// The name is `id`, `class`, `data-leptos-*` or an `on*` event handler.
    #[error("the island attribute `{0}` is reserved; use the island builder method or the loader")]
    Reserved(String),
}

/// Checks an extra attribute name for [`Island::attr`].
fn check_attr_name(name: &str) -> Result<(), AttrError> {
    let mut chars = name.chars();
    let valid = chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':' | '.'));
    if !valid {
        return Err(AttrError::InvalidName(name.to_owned()));
    }
    let lower = name.to_ascii_lowercase();
    if lower == "id"
        || lower == "class"
        || lower.starts_with("data-leptos-")
        || lower.starts_with("on")
    {
        return Err(AttrError::Reserved(name.to_owned()));
    }
    Ok(())
}

/// One Leptos island: an element that the loader mounts a component into.
///
/// It renders as:
///
/// ```html
/// <div data-leptos-island="Counter" data-leptos-props="{...}"
///      data-leptos-mount="visible" id="..." class="...">fallback</div>
/// ```
///
/// The loader replaces the fallback with the component. Without JavaScript,
/// or after an error, the fallback stays.
///
/// ```rust
/// use autumn_plugin_leptos::{Island, MountWhen};
/// use maud::{Render, html};
///
/// let island = Island::new("Counter")
///     .props(&serde_json::json!({ "start": 3 }))?
///     .mount_when(MountWhen::Visible)
///     .fallback(html! { p { "Count: 3" } });
/// let html = island.render().into_string();
/// assert!(html.starts_with(r#"<div data-leptos-island="Counter""#));
/// # Ok::<(), autumn_plugin_leptos::PropsError>(())
/// ```
///
/// Props go in an HTML attribute. Keep them small. The component name, the
/// `id` and the `class` are trusted input from your code.
#[derive(Debug, Clone)]
#[must_use]
pub struct Island {
    name: String,
    props: Option<String>,
    when: MountWhen,
    fallback: Option<Markup>,
    id: Option<String>,
    class: Option<String>,
    attrs: Vec<(String, String)>,
    inline: bool,
}

impl Island {
    /// Makes an island for the component that an app bundle registers as
    /// `name`.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            props: None,
            when: MountWhen::Load,
            fallback: None,
            id: None,
            class: None,
            attrs: Vec::new(),
            inline: false,
        }
    }

    /// The component name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Sets the component props. The value must serialize to a JSON object.
    ///
    /// # Errors
    ///
    /// - [`PropsError::Serialize`] when `serde_json` cannot serialize the
    ///   value (for example, a map with tuple keys).
    /// - [`PropsError::NotAnObject`] when the JSON is not an object.
    ///
    /// `serde_json` writes a non-finite float (`NaN`, infinity) as `null`.
    pub fn props<T: Serialize + ?Sized>(mut self, props: &T) -> Result<Self, PropsError> {
        self.props = Some(props_json(props)?);
        Ok(self)
    }

    /// Sets when the loader mounts the island. The default is
    /// [`MountWhen::Load`].
    pub const fn mount_when(mut self, when: MountWhen) -> Self {
        self.when = when;
        self
    }

    /// Sets the content to show before the mount, without JavaScript, and
    /// after an error.
    pub fn fallback(mut self, fallback: Markup) -> Self {
        self.fallback = Some(fallback);
        self
    }

    /// Renders a Leptos view on the server and sets it as the fallback.
    ///
    /// Use the same component as the client. The loader replaces this HTML
    /// with the live component at mount. There is no hydration.
    ///
    /// ```rust
    /// use autumn_plugin_leptos::Island;
    /// use leptos::prelude::*;
    /// use maud::Render as _;
    ///
    /// let html = Island::new("Hello")
    ///     .fallback_view(|| view! { <p>"Hello"</p> })
    ///     .render()
    ///     .into_string();
    /// assert_eq!(html, r#"<div data-leptos-island="Hello"><p>Hello</p></div>"#);
    /// ```
    ///
    /// The view runs in its own reactive owner. Effects do not run on the
    /// server.
    #[cfg(feature = "ssr")]
    pub fn fallback_view<F, V>(self, view: F) -> Self
    where
        F: FnOnce() -> V,
        V: leptos::IntoView,
    {
        let html = leptos::prelude::Owner::new().with(|| view().to_html());
        self.fallback(PreEscaped(html))
    }

    /// Sets the `id` attribute.
    pub fn id(mut self, id: impl Into<String>) -> Self {
        self.id = Some(id.into());
        self
    }

    /// Sets the `class` attribute.
    pub fn class(mut self, class: impl Into<String>) -> Self {
        self.class = Some(class.into());
        self
    }

    /// Adds an attribute, for example `role`, `aria-label` or
    /// `hx-preserve`. A second call with the same name replaces the value.
    ///
    /// # Errors
    ///
    /// - [`AttrError::InvalidName`] for a name that is not a plain
    ///   attribute name.
    /// - [`AttrError::Reserved`] for `id`, `class` (use [`Island::id`] and
    ///   [`Island::class`]), `data-leptos-*` (the loader owns them) and `on*`
    ///   event handlers (the default CSP blocks them).
    pub fn attr(
        mut self,
        name: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, AttrError> {
        let name = name.into();
        check_attr_name(&name)?;
        let value = value.into();
        match self.attrs.iter_mut().find(|(n, _)| *n == name) {
            Some(slot) => slot.1 = value,
            None => self.attrs.push((name, value)),
        }
        Ok(self)
    }

    /// Renders a `<span>`, not a `<div>`. Use it for an island inside a
    /// `<p>`, a `<button>` or another element that holds only inline
    /// content.
    pub const fn inline(mut self) -> Self {
        self.inline = true;
        self
    }
}

/// Serializes props to JSON text. The text must be a JSON object.
pub(crate) fn props_json<T: Serialize + ?Sized>(props: &T) -> Result<String, PropsError> {
    // Serialize one time and keep the field order. The first byte of JSON
    // text gives its kind.
    let json = serde_json::to_string(props)?;
    let kind = match json.as_bytes().first() {
        Some(b'{') => return Ok(json),
        Some(b'[') => JsonKind::Array,
        Some(b'"') => JsonKind::String,
        Some(b'n') => JsonKind::Null,
        Some(b't' | b'f') => JsonKind::Boolean,
        _ => JsonKind::Number,
    };
    Err(PropsError::NotAnObject(kind))
}

/// Writes ` name="value"` with the value escaped.
fn write_attr(html: &mut String, name: &str, value: &str) {
    html.push(' ');
    html.push_str(name);
    html.push_str("=\"");
    // Writing to a `String` cannot fail.
    let _ = Escaper::new(html).write_str(value);
    html.push('"');
}

impl Render for Island {
    fn render(&self) -> Markup {
        let tag = if self.inline { "span" } else { "div" };
        let mut html = format!("<{tag}");
        write_attr(&mut html, "data-leptos-island", &self.name);
        let known = [
            ("data-leptos-props", self.props.as_deref()),
            ("data-leptos-mount", self.when.attr()),
            ("id", self.id.as_deref()),
            ("class", self.class.as_deref()),
        ];
        for (name, value) in known {
            if let Some(value) = value {
                write_attr(&mut html, name, value);
            }
        }
        for (name, value) in &self.attrs {
            write_attr(&mut html, name, value);
        }
        html.push('>');
        if let Some(fallback) = &self.fallback {
            html.push_str(&fallback.0);
        }
        html.push_str("</");
        html.push_str(tag);
        html.push('>');
        PreEscaped(html)
    }
}
