//! HTTP server configuration settings.

use std::net::SocketAddr;

use serde::Deserialize;

use crate::{
    ClientIdentificationConfig, CorsConfig, CsrfConfig, HealthConfig, OauthConfig, RateLimitConfig, TlsServerConfig,
};

/// HTTP server configuration settings.
#[derive(Default, Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerConfig {
    /// The socket address the server should listen on.
    pub listen_address: Option<SocketAddr>,
    /// TLS configuration for secure connections.
    pub tls: Option<TlsServerConfig>,
    /// Health endpoint configuration.
    #[serde(default)]
    pub health: HealthConfig,
    /// CORS configuration
    pub cors: Option<CorsConfig>,
    /// CSRF configuration
    #[serde(default)]
    pub csrf: CsrfConfig,
    /// OAuth2 configuration
    pub oauth: Option<OauthConfig>,
    /// Rate limiting configuration
    #[serde(default)]
    pub rate_limits: RateLimitConfig,
    /// Client identification configuration for token-based rate limiting
    #[serde(default)]
    pub client_identification: Option<ClientIdentificationConfig>,
}

impl ServerConfig {
    /// Returns whether OAuth2 authentication is configured for this server.
    pub fn uses_oauth(&self) -> bool {
        self.oauth.is_some()
    }
}
