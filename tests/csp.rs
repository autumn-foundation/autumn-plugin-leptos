//! The CSP helpers add `'wasm-unsafe-eval'` and change nothing else.

#![allow(clippy::expect_used, clippy::panic)]

use autumn_plugin_leptos::{WASM_UNSAFE_EVAL, add_wasm_unsafe_eval, wasm_csp};
use autumn_web::config::AutumnConfig;
use autumn_web::prelude::*;
use autumn_web::security::default_content_security_policy;
use autumn_web::test::TestApp;
use proptest::prelude::*;

#[test]
fn token_is_the_csp_keyword() {
    assert_eq!(WASM_UNSAFE_EVAL, "'wasm-unsafe-eval'");
}

#[test]
fn wasm_csp_is_the_autumn_default_plus_the_token() {
    let expected = default_content_security_policy().replace(
        "script-src 'self';",
        "script-src 'self' 'wasm-unsafe-eval';",
    );
    assert_ne!(expected, default_content_security_policy());
    assert_eq!(wasm_csp(), expected);
}

#[test]
fn adds_the_token_to_script_src() {
    assert_eq!(
        add_wasm_unsafe_eval("default-src 'self'; script-src 'self' https://cdn.example"),
        "default-src 'self'; script-src 'self' https://cdn.example 'wasm-unsafe-eval'"
    );
}

#[test]
fn replaces_none_in_script_src() {
    // `'none'` cannot stand next to another source.
    assert_eq!(
        add_wasm_unsafe_eval("script-src 'none'; img-src 'self'"),
        "script-src 'wasm-unsafe-eval'; img-src 'self'"
    );
    assert_eq!(
        add_wasm_unsafe_eval("script-src"),
        "script-src 'wasm-unsafe-eval'"
    );
}

#[test]
fn keeps_a_policy_that_already_allows_wasm() {
    for policy in [
        "script-src 'self' 'wasm-unsafe-eval'",
        "script-src 'self' 'WASM-UNSAFE-EVAL'",
        "script-src 'self' 'unsafe-eval'",
    ] {
        assert_eq!(add_wasm_unsafe_eval(policy), policy);
    }
}

#[test]
fn derives_script_src_from_default_src() {
    assert_eq!(
        add_wasm_unsafe_eval("default-src 'self' data:; img-src *"),
        "default-src 'self' data:; img-src *; script-src 'self' data: 'wasm-unsafe-eval'"
    );
    // `'none'` cannot stand next to another source.
    assert_eq!(
        add_wasm_unsafe_eval("default-src 'none'"),
        "default-src 'none'; script-src 'wasm-unsafe-eval'"
    );
}

#[test]
fn keeps_a_policy_with_no_script_rule() {
    // No `script-src` and no `default-src`: wasm is already allowed.
    assert_eq!(add_wasm_unsafe_eval("img-src 'self'"), "img-src 'self'");
    assert_eq!(add_wasm_unsafe_eval(""), "");
    assert_eq!(add_wasm_unsafe_eval(" ; ;"), "");
}

#[test]
fn directive_names_are_case_insensitive_and_the_first_one_wins() {
    assert_eq!(
        add_wasm_unsafe_eval("SCRIPT-SRC 'self'"),
        "SCRIPT-SRC 'self' 'wasm-unsafe-eval'"
    );
    // A browser uses the first `script-src` and ignores the second.
    assert_eq!(
        add_wasm_unsafe_eval("script-src 'self'; script-src 'none'"),
        "script-src 'self' 'wasm-unsafe-eval'; script-src 'none'"
    );
}

#[test]
fn normalizes_separators_only() {
    assert_eq!(
        add_wasm_unsafe_eval("  img-src  a  ;;script-src\t'self'  "),
        "img-src  a; script-src\t'self' 'wasm-unsafe-eval'"
    );
}

/// The sources of the first `name` directive in `policy`, or `None`.
fn sources(policy: &str, name: &str) -> Option<Vec<String>> {
    policy.split(';').find_map(|d| {
        let mut parts = d.split_ascii_whitespace();
        parts
            .next()
            .filter(|n| n.eq_ignore_ascii_case(name))
            .map(|_| parts.map(str::to_owned).collect())
    })
}

fn script_src(policy: &str) -> Option<Vec<String>> {
    sources(policy, "script-src")
}

/// The sources that control wasm: `script-src`, else `default-src`.
fn effective(policy: &str) -> Option<Vec<String>> {
    script_src(policy).or_else(|| sources(policy, "default-src"))
}

fn others(policy: &str) -> Vec<String> {
    policy
        .split(';')
        .map(str::trim)
        .filter(|d| !d.is_empty() && script_src(d).is_none())
        .map(str::to_owned)
        .collect()
}

fn directive() -> impl Strategy<Value = String> {
    let name = prop::sample::select(vec![
        "default-src",
        "script-src",
        "img-src",
        "style-src",
        "connect-src",
    ]);
    let source = prop::sample::select(vec![
        "'self'",
        "'none'",
        "data:",
        "https://a.example",
        "'unsafe-inline'",
        "'unsafe-eval'",
        "'wasm-unsafe-eval'",
    ]);
    (name, prop::collection::vec(source, 0..4))
        .prop_map(|(name, sources)| format!("{name} {}", sources.join(" ")))
}

proptest! {
    /// A second call changes nothing.
    #[test]
    fn is_idempotent(directives in prop::collection::vec(directive(), 0..5)) {
        let once = add_wasm_unsafe_eval(&directives.join("; "));
        prop_assert_eq!(add_wasm_unsafe_eval(&once), once);
    }

    /// Only `script-src` changes, and it then allows wasm.
    #[test]
    fn changes_only_script_src(directives in prop::collection::vec(directive(), 0..5)) {
        let policy = directives.join("; ");
        let out = add_wasm_unsafe_eval(&policy);
        if let Some(list) = effective(&out) {
            prop_assert!(
                list.iter().any(|s| s == "'wasm-unsafe-eval'" || s == "'unsafe-eval'"),
                "{}", out
            );
        }
        prop_assert_eq!(effective(&policy).is_some(), effective(&out).is_some());
        // A derived `script-src` is new. The other directives stay.
        let mut before = others(&policy);
        let mut after = others(&out);
        before.sort();
        after.sort();
        prop_assert_eq!(before, after);
    }
}

#[get("/")]
async fn index() -> &'static str {
    "ok"
}

#[tokio::test]
async fn autumn_sends_the_wasm_policy_from_config() {
    let mut config = AutumnConfig::default();
    config.security.headers.content_security_policy = wasm_csp();
    let client = TestApp::new().config(config).routes(routes![index]).build();
    let response = client.get("/").send().await;
    response.assert_ok();
    assert_eq!(
        response.header("content-security-policy"),
        Some(wasm_csp().as_str())
    );
}

/// A loader that returns one fixed config.
struct Fixed(String);

impl autumn_web::config::ConfigLoader for Fixed {
    fn load(
        &self,
    ) -> impl Future<Output = Result<AutumnConfig, autumn_web::config::ConfigError>> + Send {
        let mut config = AutumnConfig::default();
        config
            .security
            .headers
            .content_security_policy
            .clone_from(&self.0);
        std::future::ready(Ok(config))
    }
}

#[tokio::test]
async fn the_loader_adds_the_token_to_the_loaded_policy() {
    use autumn_plugin_leptos::WasmCspLoader;
    use autumn_web::config::ConfigLoader as _;

    let loader = WasmCspLoader::new(Fixed("default-src 'self'; img-src *".into()));
    let config = loader.load().await.expect("config");
    assert_eq!(
        config.security.headers.content_security_policy,
        "default-src 'self'; img-src *; script-src 'self' 'wasm-unsafe-eval'"
    );
    let config = WasmCspLoader::new(Fixed(default_content_security_policy()))
        .load()
        .await
        .expect("config");
    assert_eq!(config.security.headers.content_security_policy, wasm_csp());
}

#[test]
fn the_default_loader_wraps_toml_and_env() {
    use autumn_plugin_leptos::WasmCspLoader;
    // It plugs into the app builder.
    let _app = autumn_web::app().with_config_loader(WasmCspLoader::default());
    let text = format!("{:?}", WasmCspLoader::default());
    assert!(text.contains("TomlEnvConfigLoader"), "{text}");
}

/// A loader that returns a config with nonce mode on.
struct NonceMode;

impl autumn_web::config::ConfigLoader for NonceMode {
    fn load(
        &self,
    ) -> impl Future<Output = Result<AutumnConfig, autumn_web::config::ConfigError>> + Send {
        let mut config = AutumnConfig::default();
        config.security.headers.csp_nonce.enabled = true;
        std::future::ready(Ok(config))
    }
}

#[tokio::test]
async fn the_loader_refuses_nonce_mode() {
    use autumn_plugin_leptos::WasmCspLoader;
    use autumn_web::config::{ConfigError, ConfigLoader as _};

    // An explicit policy turns off Autumn nonce injection. Refuse, so that
    // the app does not lose its nonces without a sign.
    let error = WasmCspLoader::new(NonceMode)
        .load()
        .await
        .expect_err("nonce mode");
    assert!(
        matches!(&error, ConfigError::Validation(m) if m.contains("csp_nonce")),
        "{error}"
    );
}
