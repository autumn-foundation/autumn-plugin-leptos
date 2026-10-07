//! [`LeptosPlugin`]: installs the loader and the app bundles.

use std::borrow::Cow;

use autumn_web::app::AppBuilder;
use autumn_web::assets::{PluginAsset, PluginAssets};
use autumn_web::plugin::Plugin;
use autumn_web::plugin_contract::PluginContract;

use crate::assets::LEPTOS_ASSETS;

/// The crate name. [`LeptosPlugin`] with no app bundles uses it as its
/// plugin name. Each [`LeptosPlugin`] declares it in its contract.
pub const PLUGIN_NAME: &str = env!("CARGO_PKG_NAME");

/// Installs Leptos islands in an Autumn app.
///
/// It serves the loader ([`LEPTOS_ASSETS`]) and each app bundle under
/// `/static/_plugins/<namespace>/`. It reads no configuration.
///
/// An app bundle is the output of `wasm-bindgen --target web`: a glue
/// module `x.js` and its module `x_bg.wasm`.
///
/// ```rust,no_run
/// use autumn_plugin_leptos::LeptosPlugin;
/// use autumn_web::assets::PluginAssets;
///
/// static ISLANDS: PluginAssets = PluginAssets::from_files(
///     "app-islands",
///     &[("app.js", b"/* glue */"), ("app_bg.wasm", b"\0asm\x01\0\0\0")],
/// );
///
/// # async fn run() {
/// autumn_web::app()
///     .plugin(LeptosPlugin::new().bundle(&ISLANDS))
///     .run()
///     .await;
/// # }
/// ```
///
/// The plugin name includes each bundle namespace and a fingerprint of its
/// files, for example `autumn-plugin-leptos[app-islands@1a2b3c4d]`.
/// Because of this, a library crate and the app can each install a `LeptosPlugin` with their
/// own bundles. Autumn skips a second plugin with the same bundles. Two
/// different bundles with one namespace stop the app at start-up.
#[derive(Debug, Default)]
#[must_use]
pub struct LeptosPlugin {
    bundles: Vec<&'static PluginAssets>,
}

impl LeptosPlugin {
    /// Makes the plugin with no app bundles.
    pub const fn new() -> Self {
        Self {
            bundles: Vec::new(),
        }
    }

    /// Adds an app bundle: wasm-bindgen output with Leptos components.
    ///
    /// The plugin uses a bundle one time, also when you add it two times.
    ///
    /// # Panics
    ///
    /// - The bundle namespace is `leptos`. The loader bundle uses it.
    /// - The bundle has no wasm-bindgen pair (`x.js` and `x_bg.wasm`).
    ///
    /// Autumn also stops at start-up when two different bundles use one
    /// namespace.
    pub fn bundle(mut self, bundle: &'static PluginAssets) -> Self {
        assert!(
            bundle.namespace() != LEPTOS_ASSETS.namespace(),
            "the namespace `leptos` belongs to {PLUGIN_NAME}; give the app bundle another namespace"
        );
        assert!(
            wasm_pairs(bundle).next().is_some(),
            "the bundle `{}` has no wasm-bindgen pair: add `x.js` and `x_bg.wasm` from `wasm-bindgen --target web`",
            bundle.namespace()
        );
        if !self.bundles.iter().any(|b| std::ptr::eq(*b, bundle)) {
            self.bundles.push(bundle);
        }
        self
    }
}

/// Each glue module (`x.js`) of `bundle` with its wasm module
/// (`x_bg.wasm`), in logical-path order.
pub(crate) fn wasm_pairs(
    bundle: &PluginAssets,
) -> impl Iterator<Item = (&PluginAsset, &PluginAsset)> {
    bundle.iter().filter_map(|glue| {
        let stem = glue.logical_path().strip_suffix(".js")?;
        bundle
            .get(&format!("{stem}_bg.wasm"))
            .map(|wasm| (glue, wasm))
    })
}

/// `namespace@fingerprint`. The fingerprint is FNV-1a (32 bit) over the
/// hashed URLs of the files, so it changes when any file changes.
fn bundle_id(bundle: &PluginAssets) -> String {
    let mut hash: u32 = 0x811c_9dc5;
    for asset in bundle.iter() {
        for byte in asset.url().bytes().chain([0]) {
            hash ^= u32::from(byte);
            hash = hash.wrapping_mul(0x0100_0193);
        }
    }
    format!("{}@{hash:08x}", bundle.namespace())
}

impl Plugin for LeptosPlugin {
    fn name(&self) -> Cow<'static, str> {
        if self.bundles.is_empty() {
            return Cow::Borrowed(PLUGIN_NAME);
        }
        let mut ids: Vec<String> = self.bundles.iter().map(|b| bundle_id(b)).collect();
        ids.sort_unstable();
        ids.dedup();
        Cow::Owned(format!("{PLUGIN_NAME}[{}]", ids.join(",")))
    }

    fn contract(&self) -> Option<PluginContract> {
        Some(
            PluginContract::new(PLUGIN_NAME)
                .plugin_version(env!("CARGO_PKG_VERSION"))
                .autumn_web("0.8"),
        )
    }

    fn build(self, app: AppBuilder) -> AppBuilder {
        self.bundles
            .into_iter()
            .fold(app.plugin_assets(&LEPTOS_ASSETS), AppBuilder::plugin_assets)
    }
}
