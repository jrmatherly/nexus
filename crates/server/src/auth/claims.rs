use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

/// Custom JWT claims that include OAuth 2.0 scopes and standard JWT claims
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomClaims {
    /// Issuer claim - identifies the principal that issued the JWT
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub iss: Option<String>,

    /// Audience claim - identifies the recipients that the JWT is intended for
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub aud: Option<Value>,
    
    /// Subject claim - identifies the principal that is the subject of the JWT
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sub: Option<String>,
    
    /// Additional claims for flexible access to custom fields
    #[serde(flatten)]
    pub additional: HashMap<String, Value>,
}

impl CustomClaims {
    /// Get the issuer claim
    pub fn get_issuer(&self) -> Option<&str> {
        self.iss.as_deref()
    }

    /// Get the audience claim as a list of strings
    pub fn get_audiences(&self) -> Vec<String> {
        match &self.aud {
            Some(Value::String(s)) => vec![s.clone()],
            Some(Value::Array(arr)) => arr.iter().filter_map(|v| v.as_str().map(String::from)).collect(),
            _ => Vec::new(),
        }
    }

    /// Check if the given audience is present in the audience claim
    pub fn has_audience(&self, expected_audience: &str) -> bool {
        self.get_audiences().iter().any(|aud| aud == expected_audience)
    }
    
    /// Extract a claim value by path, supporting nested claims.
    ///
    /// Paths can be simple (e.g., "sub") or nested (e.g., "user.plan").
    pub fn get_claim(&self, path: &str) -> Option<String> {
        // Handle standard claims
        match path {
            "iss" => return self.iss.clone(),
            "sub" => return self.sub.clone(),
            "aud" => return self.get_audiences().first().cloned(),
            _ => {}
        }
        
        // Handle nested paths in additional claims
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = self.additional.get(parts[0])?;
        
        for part in &parts[1..] {
            current = current.as_object()?.get(*part)?;
        }
        
        // Convert the final value to string
        match current {
            Value::String(s) => Some(s.clone()),
            Value::Number(n) => Some(n.to_string()),
            Value::Bool(b) => Some(b.to_string()),
            _ => None,
        }
    }
}
