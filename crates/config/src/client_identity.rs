//! Runtime client identity type.
//!
//! This type represents the extracted client identity at runtime,
//! as opposed to the configuration types that specify how to extract it.

/// Represents the identified client and their group membership.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientIdentity {
    /// The client identifier (e.g., user ID, API key ID)
    pub client_id: String,
    /// The group the client belongs to (e.g., "free", "pro", "enterprise")
    pub group: Option<String>,
}
