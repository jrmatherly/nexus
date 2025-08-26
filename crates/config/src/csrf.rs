//! CSRF (Cross-Site Request Forgery) protection configuration.

use serde::Deserialize;

/// CSRF (Cross-Site Request Forgery) protection configuration.
#[derive(Clone, Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CsrfConfig {
    /// Whether CSRF protection is enabled.
    pub enabled: bool,
    /// The name of the header to use for CSRF tokens.
    pub header_name: String,
}

impl Default for CsrfConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            header_name: "X-Nexus-CSRF-Protection".into(),
        }
    }
}
