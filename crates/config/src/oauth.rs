//! OAuth2 authentication configuration.

use std::time::Duration;

use duration_str::deserialize_option_duration;
use serde::Deserialize;
use url::Url;

/// OAuth2 configuration for authentication.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OauthConfig {
    /// The JWKs URL of the OAuth2 provider.
    pub url: Url,
    /// Polling interval for JWKs updates.
    #[serde(default, deserialize_with = "deserialize_option_duration")]
    pub poll_interval: Option<Duration>,
    /// Expected issuer (iss claim) for token validation.
    pub expected_issuer: Option<String>,
    /// Expected audience (aud claim) for token validation.
    pub expected_audience: Option<String>,
    /// Protected resource configuration.
    pub protected_resource: ProtectedResourceConfig,
}

/// Configuration for OAuth2 protected resources.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProtectedResourceConfig {
    /// The URL of the protected resource.
    pub resource: Url,
    /// List of authorization server URLs.
    pub authorization_servers: Vec<Url>,
}

impl ProtectedResourceConfig {
    /// Returns the resource documentation URL.
    pub fn resource_documentation(&self) -> Url {
        self.resource.join("/.well-known/oauth-protected-resource").unwrap()
    }
}

#[cfg(test)]
mod tests {
    use indoc::indoc;
    use insta::assert_debug_snapshot;

    use crate::Config;

    #[test]
    fn oauth_basic_config() {
        let config = indoc! {r#"
            [server.oauth]
            url = "https://auth.example.com/.well-known/jwks.json"
            poll_interval = "5m"

            [server.oauth.protected_resource]
            resource = "https://api.example.com"
            authorization_servers = ["https://auth.example.com"]
        "#};

        let config: Config = toml::from_str(config).unwrap();

        assert_debug_snapshot!(&config.server.oauth, @r#"
        Some(
            OauthConfig {
                url: Url {
                    scheme: "https",
                    cannot_be_a_base: false,
                    username: "",
                    password: None,
                    host: Some(
                        Domain(
                            "auth.example.com",
                        ),
                    ),
                    port: None,
                    path: "/.well-known/jwks.json",
                    query: None,
                    fragment: None,
                },
                poll_interval: Some(
                    300s,
                ),
                expected_issuer: None,
                expected_audience: None,
                protected_resource: ProtectedResourceConfig {
                    resource: Url {
                        scheme: "https",
                        cannot_be_a_base: false,
                        username: "",
                        password: None,
                        host: Some(
                            Domain(
                                "api.example.com",
                            ),
                        ),
                        port: None,
                        path: "/",
                        query: None,
                        fragment: None,
                    },
                    authorization_servers: [
                        Url {
                            scheme: "https",
                            cannot_be_a_base: false,
                            username: "",
                            password: None,
                            host: Some(
                                Domain(
                                    "auth.example.com",
                                ),
                            ),
                            port: None,
                            path: "/",
                            query: None,
                            fragment: None,
                        },
                    ],
                },
            },
        )
        "#);
    }

    #[test]
    fn oauth_minimal_config() {
        let config = indoc! {r#"
            [server.oauth]
            url = "https://auth.example.com/.well-known/jwks.json"

            [server.oauth.protected_resource]
            resource = "https://api.example.com"
            authorization_servers = ["https://auth.example.com"]
        "#};

        let config: Config = toml::from_str(config).unwrap();

        assert_debug_snapshot!(&config.server.oauth, @r#"
        Some(
            OauthConfig {
                url: Url {
                    scheme: "https",
                    cannot_be_a_base: false,
                    username: "",
                    password: None,
                    host: Some(
                        Domain(
                            "auth.example.com",
                        ),
                    ),
                    port: None,
                    path: "/.well-known/jwks.json",
                    query: None,
                    fragment: None,
                },
                poll_interval: None,
                expected_issuer: None,
                expected_audience: None,
                protected_resource: ProtectedResourceConfig {
                    resource: Url {
                        scheme: "https",
                        cannot_be_a_base: false,
                        username: "",
                        password: None,
                        host: Some(
                            Domain(
                                "api.example.com",
                            ),
                        ),
                        port: None,
                        path: "/",
                        query: None,
                        fragment: None,
                    },
                    authorization_servers: [
                        Url {
                            scheme: "https",
                            cannot_be_a_base: false,
                            username: "",
                            password: None,
                            host: Some(
                                Domain(
                                    "auth.example.com",
                                ),
                            ),
                            port: None,
                            path: "/",
                            query: None,
                            fragment: None,
                        },
                    ],
                },
            },
        )
        "#);
    }

    #[test]
    fn oauth_config_with_issuer_audience() {
        let config = indoc! {r#"
            [server.oauth]
            url = "https://auth.example.com/.well-known/jwks.json"
            poll_interval = "5m"
            expected_issuer = "https://auth.example.com"
            expected_audience = "my-app-client-id"

            [server.oauth.protected_resource]
            resource = "https://api.example.com"
            authorization_servers = ["https://auth.example.com"]
        "#};

        let config: Config = toml::from_str(config).unwrap();

        assert_debug_snapshot!(&config.server.oauth, @r#"
        Some(
            OauthConfig {
                url: Url {
                    scheme: "https",
                    cannot_be_a_base: false,
                    username: "",
                    password: None,
                    host: Some(
                        Domain(
                            "auth.example.com",
                        ),
                    ),
                    port: None,
                    path: "/.well-known/jwks.json",
                    query: None,
                    fragment: None,
                },
                poll_interval: Some(
                    300s,
                ),
                expected_issuer: Some(
                    "https://auth.example.com",
                ),
                expected_audience: Some(
                    "my-app-client-id",
                ),
                protected_resource: ProtectedResourceConfig {
                    resource: Url {
                        scheme: "https",
                        cannot_be_a_base: false,
                        username: "",
                        password: None,
                        host: Some(
                            Domain(
                                "api.example.com",
                            ),
                        ),
                        port: None,
                        path: "/",
                        query: None,
                        fragment: None,
                    },
                    authorization_servers: [
                        Url {
                            scheme: "https",
                            cannot_be_a_base: false,
                            username: "",
                            password: None,
                            host: Some(
                                Domain(
                                    "auth.example.com",
                                ),
                            ),
                            port: None,
                            path: "/",
                            query: None,
                            fragment: None,
                        },
                    ],
                },
            },
        )
        "#);
    }

    #[test]
    fn oauth_multiple_authorization_servers() {
        let config = indoc! {r#"
            [server.oauth]
            url = "https://auth.example.com/.well-known/jwks.json"

            [server.oauth.protected_resource]
            resource = "https://api.example.com"
            authorization_servers = [
                "https://auth1.example.com",
                "https://auth2.example.com",
                "https://auth3.example.com"
            ]
        "#};

        let config: Config = toml::from_str(config).unwrap();

        assert_debug_snapshot!(&config.server.oauth, @r#"
        Some(
            OauthConfig {
                url: Url {
                    scheme: "https",
                    cannot_be_a_base: false,
                    username: "",
                    password: None,
                    host: Some(
                        Domain(
                            "auth.example.com",
                        ),
                    ),
                    port: None,
                    path: "/.well-known/jwks.json",
                    query: None,
                    fragment: None,
                },
                poll_interval: None,
                expected_issuer: None,
                expected_audience: None,
                protected_resource: ProtectedResourceConfig {
                    resource: Url {
                        scheme: "https",
                        cannot_be_a_base: false,
                        username: "",
                        password: None,
                        host: Some(
                            Domain(
                                "api.example.com",
                            ),
                        ),
                        port: None,
                        path: "/",
                        query: None,
                        fragment: None,
                    },
                    authorization_servers: [
                        Url {
                            scheme: "https",
                            cannot_be_a_base: false,
                            username: "",
                            password: None,
                            host: Some(
                                Domain(
                                    "auth1.example.com",
                                ),
                            ),
                            port: None,
                            path: "/",
                            query: None,
                            fragment: None,
                        },
                        Url {
                            scheme: "https",
                            cannot_be_a_base: false,
                            username: "",
                            password: None,
                            host: Some(
                                Domain(
                                    "auth2.example.com",
                                ),
                            ),
                            port: None,
                            path: "/",
                            query: None,
                            fragment: None,
                        },
                        Url {
                            scheme: "https",
                            cannot_be_a_base: false,
                            username: "",
                            password: None,
                            host: Some(
                                Domain(
                                    "auth3.example.com",
                                ),
                            ),
                            port: None,
                            path: "/",
                            query: None,
                            fragment: None,
                        },
                    ],
                },
            },
        )
        "#);
    }

    #[test]
    fn oauth_resource_documentation() {
        let config = indoc! {r#"
            [server.oauth]
            url = "https://auth.example.com/.well-known/jwks.json"

            [server.oauth.protected_resource]
            resource = "https://api.example.com"
            authorization_servers = ["https://auth.example.com"]
        "#};

        let config: Config = toml::from_str(config).unwrap();
        let oauth_config = config.server.oauth.as_ref().unwrap();

        let resource_doc_url = oauth_config.protected_resource.resource_documentation();
        assert_debug_snapshot!(&resource_doc_url.as_str(), @r#""https://api.example.com/.well-known/oauth-protected-resource""#);
    }

    #[test]
    fn oauth_with_various_poll_intervals() {
        let config = indoc! {r#"
            [server.oauth]
            url = "https://auth.example.com/.well-known/jwks.json"
            poll_interval = "30s"

            [server.oauth.protected_resource]
            resource = "https://api.example.com"
            authorization_servers = ["https://auth.example.com"]
        "#};

        let config: Config = toml::from_str(config).unwrap();

        assert_debug_snapshot!(&config.server.oauth, @r#"
        Some(
            OauthConfig {
                url: Url {
                    scheme: "https",
                    cannot_be_a_base: false,
                    username: "",
                    password: None,
                    host: Some(
                        Domain(
                            "auth.example.com",
                        ),
                    ),
                    port: None,
                    path: "/.well-known/jwks.json",
                    query: None,
                    fragment: None,
                },
                poll_interval: Some(
                    30s,
                ),
                expected_issuer: None,
                expected_audience: None,
                protected_resource: ProtectedResourceConfig {
                    resource: Url {
                        scheme: "https",
                        cannot_be_a_base: false,
                        username: "",
                        password: None,
                        host: Some(
                            Domain(
                                "api.example.com",
                            ),
                        ),
                        port: None,
                        path: "/",
                        query: None,
                        fragment: None,
                    },
                    authorization_servers: [
                        Url {
                            scheme: "https",
                            cannot_be_a_base: false,
                            username: "",
                            password: None,
                            host: Some(
                                Domain(
                                    "auth.example.com",
                                ),
                            ),
                            port: None,
                            path: "/",
                            query: None,
                            fragment: None,
                        },
                    ],
                },
            },
        )
        "#);
    }

    #[test]
    fn oauth_invalid_url_should_fail() {
        let config = indoc! {r#"
            [server.oauth]
            url = "not-a-valid-url"

            [server.oauth.protected_resource]
            resource = "https://api.example.com"
            authorization_servers = ["https://auth.example.com"]
        "#};

        let result: Result<Config, _> = toml::from_str(config);
        assert!(result.is_err());
    }

    #[test]
    fn oauth_invalid_authorization_server_url_should_fail() {
        let config = indoc! {r#"
            [server.oauth]
            url = "https://auth.example.com/.well-known/jwks.json"

            [server.oauth.protected_resource]
            resource = "https://api.example.com"
            authorization_servers = ["not-a-valid-url"]
        "#};

        let result: Result<Config, _> = toml::from_str(config);
        assert!(result.is_err());
    }
}
