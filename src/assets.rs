//! The loader bundle: `assets/leptos-islands.js`, embedded at compile time.
//!
//! [`LeptosPlugin`](crate::LeptosPlugin) installs [`LEPTOS_ASSETS`]
//! through `AppBuilder::plugin_assets`. Autumn serves the file under
//! `/static/_plugins/leptos/`:
//!
//! - at a hashed URL (`leptos-islands.<sha256-prefix>.js`), `immutable`;
//! - at its plain URL (`leptos-islands.js`), `must-revalidate`;
//! - with `ETag`/`304`, `Range` and a computed `sha384` SRI hash.

use autumn_web::assets::PluginAssets;

/// URL namespace of the loader bundle: `/static/_plugins/leptos/`.
pub const ASSETS_NAMESPACE: &str = "leptos";

/// Logical path of the loader in [`LEPTOS_ASSETS`].
pub const LOADER_JS: &str = "leptos-islands.js";

/// The loader bundle. It holds one file, [`LOADER_JS`].
///
/// [`LeptosPlugin`](crate::LeptosPlugin) installs it. Use it directly only
/// to make URLs or tags yourself:
///
/// ```rust
/// use autumn_plugin_leptos::{LEPTOS_ASSETS, LOADER_JS};
///
/// let url = LEPTOS_ASSETS.url(LOADER_JS);
/// assert!(url.starts_with("/static/_plugins/leptos/leptos-islands."), "{url}");
/// assert!(LEPTOS_ASSETS.integrity(LOADER_JS).is_some_and(|s| s.starts_with("sha384-")));
/// ```
pub static LEPTOS_ASSETS: PluginAssets = PluginAssets::from_files(
    ASSETS_NAMESPACE,
    &[(LOADER_JS, include_bytes!("../assets/leptos-islands.js"))],
);
