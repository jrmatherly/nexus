mod middleware;

use config::{ClientIdentificationConfig, IdentificationSource};
use http::Request;
use jwt_compact::Token;
pub use middleware::ClientIdentificationLayer;

use crate::auth::claims::CustomClaims;

/// Represents the identified client and their group membership.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientIdentity {
    /// The client identifier (e.g., user ID, API key ID)
    pub client_id: String,
    /// The group the client belongs to (e.g., "free", "pro", "enterprise")
    pub group: Option<String>,
}

/// Errors that can occur during client identification extraction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientIdentificationError {
    /// The client is in a group that's not in the allowed list
    UnauthorizedGroup { group: String, allowed_groups: Vec<String> },
    /// Client identification is required but not found
    MissingIdentification,
}

impl std::fmt::Display for ClientIdentificationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnauthorizedGroup { group, allowed_groups } => {
                write!(
                    f,
                    "Client is in group '{}' which is not in allowed groups: {:?}",
                    group, allowed_groups
                )
            }
            Self::MissingIdentification => {
                write!(f, "Client identification is required but not provided")
            }
        }
    }
}

impl std::error::Error for ClientIdentificationError {}

/// Extract client identity from a request based on configuration.
///
/// This function attempts to extract client identification from either:
/// 1. JWT claims (if a valid token is present)
/// 2. HTTP headers
///
/// The extraction method and field names are determined by the configuration.
///
/// Returns:
/// - `Ok(Some(identity))` if identity was successfully extracted
/// - `Ok(None)` if client identification is not enabled
/// - `Err(...)` if identification is required but invalid or missing
pub fn extract_client_identity<B>(
    req: &Request<B>,
    config: &ClientIdentificationConfig,
) -> Result<Option<ClientIdentity>, ClientIdentificationError> {
    if !config.enabled {
        return Ok(None);
    }

    // Extract client ID if configured
    let client_id = if let Some(source) = &config.client_id {
        let id = extract_from_source(req, source).ok_or(ClientIdentificationError::MissingIdentification)?;

        Some(id)
    } else {
        None
    };

    // Extract group if configured
    let group = config
        .group_id
        .as_ref()
        .and_then(|source| extract_from_source(req, source));

    // Validate group against allowed list
    validate_group(&group, config)?;

    // If nothing was extracted but identification is enabled, return None
    // (this handles the case where identification is enabled but nothing is configured)
    let Some(client_id) = client_id else {
        return match group {
            Some(g) => Ok(Some(ClientIdentity {
                client_id: "unknown".to_string(),
                group: Some(g),
            })),
            None => Ok(None),
        };
    };

    Ok(Some(ClientIdentity { client_id, group }))
}

/// Validate that a group is in the allowed list if restrictions are configured.
fn validate_group(
    group: &Option<String>,
    config: &ClientIdentificationConfig,
) -> Result<(), ClientIdentificationError> {
    // No validation needed if no allowed_groups are configured
    if config.allowed_groups.is_empty() {
        return Ok(());
    }

    // If group_id is configured but no group was extracted, that's an error
    if config.group_id.is_some() && group.is_none() {
        return Err(ClientIdentificationError::MissingIdentification);
    }

    // If we have a group, it must be in the allowed list
    if let Some(g) = group
        && !config.allowed_groups.contains(g)
    {
        return Err(ClientIdentificationError::UnauthorizedGroup {
            group: g.clone(),
            allowed_groups: config.allowed_groups.iter().cloned().collect(),
        });
    }

    Ok(())
}

/// Extract a value from either JWT claims or HTTP headers based on the source configuration.
fn extract_from_source<B>(req: &Request<B>, source: &IdentificationSource) -> Option<String> {
    match source {
        IdentificationSource::JwtClaim { jwt_claim } => extract_from_jwt_claims(req, jwt_claim),
        IdentificationSource::HttpHeader { http_header } => extract_from_http_header(req, http_header),
    }
}

/// Extract a value from JWT claims.
///
/// This requires the request to have a validated JWT token in its extensions,
/// which is added by the authentication middleware.
fn extract_from_jwt_claims<B>(req: &Request<B>, claim_path: &str) -> Option<String> {
    // Get the validated token from request extensions
    let token = req.extensions().get::<Token<CustomClaims>>()?;
    let claims = token.claims();

    // Use the get_claim method to extract the value
    claims.custom.get_claim(claim_path)
}

/// Extract a value from HTTP headers.
fn extract_from_http_header<B>(req: &Request<B>, header_name: &str) -> Option<String> {
    req.headers()
        .get(header_name)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use config::IdentificationSource;
    use std::collections::BTreeSet;

    fn create_config(enabled: bool) -> ClientIdentificationConfig {
        ClientIdentificationConfig {
            enabled,
            allowed_groups: BTreeSet::new(),
            client_id: None,
            group_id: None,
        }
    }

    #[test]
    fn disabled_config_returns_none() {
        let config = create_config(false);
        let req = Request::builder().body(()).unwrap();

        assert_eq!(extract_client_identity(&req, &config), Ok(None));
    }

    #[test]
    fn no_client_id_configured_returns_none() {
        let config = create_config(true);
        let req = Request::builder().body(()).unwrap();

        // When identification is enabled but no sources are configured, returns None
        assert_eq!(extract_client_identity(&req, &config), Ok(None));
    }

    #[test]
    fn extract_from_http_headers() {
        let mut config = create_config(true);
        config.client_id = Some(IdentificationSource::HttpHeader {
            http_header: "X-Client-Id".to_string(),
        });
        config.group_id = Some(IdentificationSource::HttpHeader {
            http_header: "X-Group".to_string(),
        });

        let req = Request::builder()
            .header("X-Client-Id", "user123")
            .header("X-Group", "pro")
            .body(())
            .unwrap();

        let identity = extract_client_identity(&req, &config).unwrap().unwrap();
        assert_eq!(identity.client_id, "user123");
        assert_eq!(identity.group, Some("pro".to_string()));
    }

    #[test]
    fn validates_group_against_allowed_list() {
        let mut config = create_config(true);
        config.client_id = Some(IdentificationSource::HttpHeader {
            http_header: "X-Client-Id".to_string(),
        });
        config.group_id = Some(IdentificationSource::HttpHeader {
            http_header: "X-Group".to_string(),
        });
        config.allowed_groups = ["free", "pro"].iter().map(|s| s.to_string()).collect();

        // Valid group
        let req = Request::builder()
            .header("X-Client-Id", "user123")
            .header("X-Group", "pro")
            .body(())
            .unwrap();

        let identity = extract_client_identity(&req, &config).unwrap().unwrap();
        assert_eq!(identity.group, Some("pro".to_string()));

        // Invalid group - should return error
        let req = Request::builder()
            .header("X-Client-Id", "user123")
            .header("X-Group", "enterprise")
            .body(())
            .unwrap();

        let result = extract_client_identity(&req, &config);
        assert_eq!(
            result,
            Err(ClientIdentificationError::UnauthorizedGroup {
                group: "enterprise".to_string(),
                allowed_groups: vec!["free".to_string(), "pro".to_string()],
            })
        );
    }

    #[test]
    fn missing_header_returns_error() {
        let mut config = create_config(true);
        config.client_id = Some(IdentificationSource::HttpHeader {
            http_header: "X-Client-Id".to_string(),
        });

        let req = Request::builder().body(()).unwrap();

        assert_eq!(
            extract_client_identity(&req, &config),
            Err(ClientIdentificationError::MissingIdentification)
        );
    }
}
