//! CSP helpers. A browser compiles wasm only when `script-src` allows
//! `'wasm-unsafe-eval'`.
//!
//! No plugin hook can change the Autumn CSP, so the app sets the policy:
//!
//! ```toml
//! # autumn.toml
//! [security.headers]
//! content_security_policy = "default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; script-src 'self' 'wasm-unsafe-eval'; connect-src 'self'; form-action 'self'; frame-ancestors 'none'; base-uri 'self'"
//! ```
//!
//! That string is [`wasm_csp()`]. `'wasm-unsafe-eval'` allows wasm
//! compilation only. It does not allow `eval()`.

use autumn_web::config::{AutumnConfig, ConfigError, ConfigLoader, TomlEnvConfigLoader};
use autumn_web::security::default_content_security_policy;

/// The CSP source that allows wasm compilation.
pub const WASM_UNSAFE_EVAL: &str = "'wasm-unsafe-eval'";

/// The Autumn default CSP plus [`WASM_UNSAFE_EVAL`] in `script-src`.
///
/// ```rust
/// let csp = autumn_plugin_leptos::wasm_csp();
/// assert!(csp.contains("script-src 'self' 'wasm-unsafe-eval';"), "{csp}");
/// ```
#[must_use]
pub fn wasm_csp() -> String {
    add_wasm_unsafe_eval(&default_content_security_policy())
}

/// Adds [`WASM_UNSAFE_EVAL`] to the `script-src` directive of `policy`.
///
/// - It changes the first `script-src` only. A browser ignores the others.
/// - With no `script-src`, it adds one with the `default-src` sources.
/// - With neither, it changes nothing: wasm is already allowed.
/// - It adds nothing when `'unsafe-eval'` or the token is there.
/// - It removes `'none'`, because `'none'` cannot stand next to a source.
///
/// It always writes `"; "` between directives and drops empty directives.
/// It keeps the text of the other directives. A second call does not
/// change the result.
///
/// ```rust
/// use autumn_plugin_leptos::add_wasm_unsafe_eval;
///
/// assert_eq!(
///     add_wasm_unsafe_eval("default-src 'self'; script-src 'self'"),
///     "default-src 'self'; script-src 'self' 'wasm-unsafe-eval'"
/// );
/// ```
#[must_use]
pub fn add_wasm_unsafe_eval(policy: &str) -> String {
    let mut directives: Vec<String> = policy
        .split(';')
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .map(str::to_owned)
        .collect();
    if let Some(script) = directives.iter_mut().find(|d| is(d, "script-src")) {
        *script = with_token(script);
    } else if let Some(default) = directives.iter().find(|d| is(d, "default-src")) {
        let sources: Vec<&str> = default.split_ascii_whitespace().skip(1).collect();
        if !allows_wasm(&sources) {
            let derived = with_token(&format!("script-src {}", sources.join(" ")));
            directives.push(derived);
        }
    }
    directives.join("; ")
}

/// `true` when the name of `directive` is `name` (ASCII case-insensitive).
fn is(directive: &str, name: &str) -> bool {
    directive
        .split_ascii_whitespace()
        .next()
        .is_some_and(|n| n.eq_ignore_ascii_case(name))
}

fn allows_wasm(sources: &[&str]) -> bool {
    sources.iter().any(|s| {
        s.eq_ignore_ascii_case(WASM_UNSAFE_EVAL) || s.eq_ignore_ascii_case("'unsafe-eval'")
    })
}

/// Adds the token to one directive.
fn with_token(directive: &str) -> String {
    let directive = directive.trim();
    let mut words = directive.split_ascii_whitespace();
    let name = words.next().unwrap_or_default();
    let sources: Vec<&str> = words.collect();
    if allows_wasm(&sources) {
        return directive.to_owned();
    }
    if sources.iter().any(|s| s.eq_ignore_ascii_case("'none'")) || sources.is_empty() {
        let kept = sources.iter().filter(|s| !s.eq_ignore_ascii_case("'none'"));
        let mut out = name.to_owned();
        for source in kept.chain([&WASM_UNSAFE_EVAL]) {
            out.push(' ');
            out.push_str(source);
        }
        return out;
    }
    format!("{directive} {WASM_UNSAFE_EVAL}")
}

/// A [`ConfigLoader`] that adds [`WASM_UNSAFE_EVAL`] to the loaded CSP.
///
/// It runs the inner loader first. Then it calls [`add_wasm_unsafe_eval`]
/// on `security.headers.content_security_policy`. Because of this,
/// `autumn.toml` and `AUTUMN_*` variables still set the policy.
///
/// ```rust,no_run
/// use autumn_plugin_leptos::WasmCspLoader;
///
/// # async fn run() {
/// autumn_web::app()
///     .with_config_loader(WasmCspLoader::default())
///     .run()
///     .await;
/// # }
/// ```
///
/// The default inner loader is Autumn's [`TomlEnvConfigLoader`]. An app
/// has one config loader. Wrap your own loader with
/// [`WasmCspLoader::new`].
///
/// # Errors
///
/// `load` returns [`ConfigError::Validation`] when
/// `security.headers.csp_nonce.enabled` is `true`. Autumn adds nonces only
/// to its default policy. A changed policy has no nonces, so the nonces of
/// the app stop working, and `style-src` goes back to `'unsafe-inline'`.
#[derive(Debug, Clone)]
pub struct WasmCspLoader<L = TomlEnvConfigLoader> {
    inner: L,
}

impl Default for WasmCspLoader {
    fn default() -> Self {
        Self::new(TomlEnvConfigLoader::default())
    }
}

impl<L> WasmCspLoader<L> {
    /// Wraps `inner`.
    pub const fn new(inner: L) -> Self {
        Self { inner }
    }
}

impl<L: ConfigLoader> ConfigLoader for WasmCspLoader<L> {
    async fn load(&self) -> Result<AutumnConfig, ConfigError> {
        let mut config = self.inner.load().await?;
        let headers = &mut config.security.headers;
        if headers.csp_nonce.enabled {
            return Err(ConfigError::Validation(
                "WasmCspLoader cannot change the CSP when security.headers.csp_nonce.enabled \
                 is true: Autumn then sends the policy as it is, with no nonce. Turn off \
                 csp_nonce, or set a policy with 'wasm-unsafe-eval' and your own nonce handling"
                    .to_owned(),
            ));
        }
        let policy = &mut headers.content_security_policy;
        *policy = add_wasm_unsafe_eval(policy);
        Ok(config)
    }
}
