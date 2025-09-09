use std::{path::Path, str::FromStr};

use anyhow::bail;
use indoc::indoc;
use serde::Deserialize;
use serde_dynamic_string::DynamicString;
use std::fmt::Write;
use toml::Value;

use crate::{ClientIdentificationConfig, Config, LlmProviderConfig};

/// Check if a configuration path represents an optional environment variable field
fn is_optional_env_field(path: &str) -> bool {
    // Known optional environment variable fields
    const OPTIONAL_FIELDS: &[&str] = &[
        "llm.providers.openai.base_url",
        "llm.providers.anthropic.base_url", 
        "llm.providers.google.base_url",
        "llm.providers.bedrock.base_url",
        "server.client_identification.jwt_public_key_url",
        "telemetry.service_name",
    ];
    
    OPTIONAL_FIELDS.iter().any(|&field| path.ends_with(field))
}

/// Check if the error is specifically about a missing environment variable
fn is_missing_env_var_error<E: std::fmt::Display>(err: &E) -> bool {
    let err_str = err.to_string().to_lowercase();
    err_str.contains("environment variable not found") || 
    err_str.contains("env var") ||
    (err_str.contains("variable") && err_str.contains("not found"))
}

/// Extract the path from an error message like "Failed to expand dynamic string at path 'path': error"
fn extract_path_from_error(error_message: &str) -> Option<String> {
    if let Some(start) = error_message.find("path '") {
        let start_pos = start + 6; // Skip "path '"
        if let Some(end) = error_message[start_pos..].find("':") {
            let path = &error_message[start_pos..start_pos + end];
            return Some(path.to_string());
        }
    }
    None
}

/// Remove a field from the TOML configuration by path
fn remove_field_from_config(config: &mut Value, path: &str) {
    let parts: Vec<&str> = path.split('.').collect();
    
    if parts.is_empty() {
        return;
    }
    
    let mut current = config;
    
    // Navigate to the parent of the field we want to remove
    for &part in &parts[..parts.len() - 1] {
        if let Some(table) = current.as_table_mut() {
            if let Some(value) = table.get_mut(part) {
                current = value;
            } else {
                return; // Path doesn't exist
            }
        } else {
            return; // Not a table, can't navigate further
        }
    }
    
    // Remove the final field
    if let Some(table) = current.as_table_mut() {
        let final_key = parts[parts.len() - 1];
        table.remove(final_key);
        log::debug!("Removed optional field '{path}' due to missing environment variable");
    }
}

pub fn load<P: AsRef<Path>>(path: P) -> anyhow::Result<Config> {
    let path = path.as_ref().to_path_buf();
    let content = std::fs::read_to_string(&path)?;
    let mut raw_config: Value = toml::from_str(&content)?;

    // Try expanding dynamic strings, but handle optional fields gracefully
    if let Err(err) = expand_dynamic_strings(&mut Vec::new(), &mut raw_config) {
        // Check if this error is from an optional field with missing env var
        let err_str = err.to_string();
        if err_str.contains("Failed to expand dynamic string at path") {
            if let Some(path_part) = extract_path_from_error(&err_str) {
                if is_optional_env_field(&path_part) && is_missing_env_var_error(&err) {
                    // Remove the optional field and try again
                    remove_field_from_config(&mut raw_config, &path_part);
                    expand_dynamic_strings(&mut Vec::new(), &mut raw_config)?;
                } else {
                    return Err(err);
                }
            } else {
                return Err(err);
            }
        } else {
            return Err(err);
        }
    }

    let config = Config::deserialize(raw_config)?;
    validate_has_downstreams(&config)?;

    // Validate LLM rate limit configuration and log warnings
    let warnings = validate_rate_limits(&config)?;

    for warning in warnings {
        log::warn!("{warning}");
    }

    Ok(config)
}

pub(crate) fn validate_has_downstreams(config: &Config) -> anyhow::Result<()> {
    // Check if any downstreams are actually configured (not just enabled)
    let has_mcp_servers = config.mcp.enabled() && config.mcp.has_servers();
    let has_llm_providers = config.llm.enabled() && config.llm.has_providers();

    if !has_mcp_servers && !has_llm_providers {
        bail!(indoc! {r#"
            No downstream servers configured. Nexus requires at least one MCP server or LLM provider to function.

            Example configuration:

            For MCP servers:

              [mcp.servers.example]
              cmd = ["path/to/mcp-server"]

            For LLM providers:

              [llm.providers.openai]
              type = "openai"
              api_key = "{{ env.OPENAI_API_KEY }}"

            See https://nexusrouter.com/docs for more configuration examples.
        "#});
    }

    Ok(())
}

fn expand_dynamic_strings<'a>(path: &mut Vec<Result<&'a str, usize>>, value: &'a mut Value) -> anyhow::Result<()> {
    match value {
        Value::String(s) => match DynamicString::<String>::from_str(s) {
            Ok(out) => *s = out.into_inner(),
            Err(err) => {
                // Build the path string for error reporting and optional field detection
                let mut p = String::new();
                for segment in path {
                    match segment {
                        Ok(s) => {
                            p.push_str(s);
                            p.push('.');
                        }
                        Err(i) => write!(p, "[{i}]").unwrap(),
                    }
                }
                if p.ends_with('.') {
                    p.pop();
                }

                bail!("Failed to expand dynamic string at path '{p}': {err}");
            }
        },
        Value::Array(values) => {
            for (i, value) in values.iter_mut().enumerate() {
                path.push(Err(i));
                expand_dynamic_strings(path, value)?;
                path.pop();
            }
        }
        Value::Table(map) => {
            for (key, value) in map {
                path.push(Ok(key.as_str()));
                expand_dynamic_strings(path, value)?;
                path.pop();
            }
        }
        Value::Integer(_) | Value::Float(_) | Value::Boolean(_) | Value::Datetime(_) => (),
    }

    Ok(())
}

/// Validates the rate limit configuration and returns warnings.
pub(crate) fn validate_rate_limits(config: &Config) -> anyhow::Result<Vec<String>> {
    let mut warnings = Vec::new();

    // Check if any LLM provider has rate limits defined
    let has_llm_rate_limits = check_if_any_llm_rate_limits_exist(config);

    // If we do not have LLM rate limits, skip further validation
    if !has_llm_rate_limits {
        return Ok(Vec::new());
    }

    // If we have LLM rate limits, client identification MUST be enabled
    let Some(client_identification) = config.server.client_identification.as_ref() else {
        anyhow::bail!(
            "LLM rate limits are configured but client identification is not enabled. Enable client identification in [server.client_identification]"
        );
    };

    // If LLM rate limits are defined, client identification MUST be enabled
    if !client_identification.enabled {
        anyhow::bail!(
            "LLM rate limits are configured but client identification is not enabled. Enable client identification in [server.client_identification]"
        );
    }

    // If group_id is configured, group_values MUST be defined
    if client_identification.group_id.is_some() && client_identification.validation.group_values.is_empty() {
        anyhow::bail!(
            "group_id is configured for client identification but validation.group_values is empty. Define group_values in [server.client_identification.validation]"
        );
    }

    // Check if any provider has group-based rate limits
    let has_group_rate_limits = check_if_any_group_rate_limits_exist(config);

    if !has_group_rate_limits {
        return Ok(warnings);
    }

    // If group rate limits are defined, group identification MUST be configured
    if client_identification.group_id.is_none() {
        anyhow::bail!(indoc! {r#"
            Group-based rate limits are configured but group_id is not set in client identification.
            To fix this, add a group_id configuration to your [server.client_identification] section, for example:

            [server.client_identification]
            enabled = true
            client_id.http_header = "X-Client-ID"      # or client_id.jwt_claim = "sub"
            group_id.http_header = "X-Group-ID"        # or group_id.jwt_claim = "groups"
            
            [server.client_identification.validation]
            group_values = ["basic", "premium", "enterprise"]
        "#});
    }

    // Validate all group names in rate limits exist in group_values
    for (provider_name, provider) in &config.llm.providers {
        validate_provider_groups(client_identification, provider_name, provider)?;

        // Generate warnings for fallback scenarios
        if client_identification.validation.group_values.is_empty() {
            continue;
        }

        for group in &client_identification.validation.group_values {
            check_group_fallbacks(group, provider_name, provider, &mut warnings);
        }
    }

    Ok(warnings)
}

fn validate_provider_groups(
    config: &ClientIdentificationConfig,
    provider_name: &str,
    provider: &LlmProviderConfig,
) -> anyhow::Result<()> {
    // Check provider-level group rate limits
    if let Some(rate_limits) = &provider.rate_limits()
        && let Some(per_user) = &rate_limits.per_user
    {
        for group_name in per_user.groups.keys() {
            if config.validation.group_values.contains(group_name) {
                continue;
            }

            anyhow::bail!("Group '{group_name}' in provider '{provider_name}' rate limits not found in group_values",);
        }
    }

    // Check model-level group rate limits
    for (model_name, model) in provider.models() {
        let Some(rate_limits) = model.rate_limits() else {
            continue;
        };

        if let Some(per_user) = &rate_limits.per_user {
            for group_name in per_user.groups.keys() {
                if config.validation.group_values.contains(group_name) {
                    continue;
                }

                anyhow::bail!(
                    "Group '{group_name}' in model '{provider_name}/{model_name}' rate limits not found in group_values",
                );
            }
        }
    }

    Ok(())
}

/// Check if any LLM provider or model has rate limits configured.
fn check_if_any_llm_rate_limits_exist(config: &Config) -> bool {
    for provider in config.llm.providers.values() {
        // Check if provider has any rate limits
        if provider.rate_limits().is_some() {
            return true;
        }

        // Check if provider has group-specific rate limits
        if let Some(limits) = provider.rate_limits()
            && let Some(per_user) = &limits.per_user
            && !per_user.groups.is_empty()
        {
            return true;
        }

        // Check if any model has rate limits
        for model in provider.models().values() {
            if model.rate_limits().is_some() {
                return true;
            }

            // Check if model has group-specific rate limits
            if let Some(limits) = model.rate_limits()
                && let Some(per_user) = &limits.per_user
                && !per_user.groups.is_empty()
            {
                return true;
            }
        }
    }

    false
}

/// Check if any provider or model has group-based rate limits.
fn check_if_any_group_rate_limits_exist(config: &Config) -> bool {
    for provider in config.llm.providers.values() {
        // Check provider-level group rate limits
        if let Some(limits) = provider.rate_limits()
            && let Some(per_user) = &limits.per_user
            && !per_user.groups.is_empty()
        {
            return true;
        }

        // Check model-level group rate limits
        for model in provider.models().values() {
            if let Some(limits) = model.rate_limits()
                && let Some(per_user) = &limits.per_user
                && !per_user.groups.is_empty()
            {
                return true;
            }
        }
    }

    false
}

fn check_group_fallbacks(group: &str, provider_name: &str, provider: &LlmProviderConfig, warnings: &mut Vec<String>) {
    // Check each model's fallback situation
    for (model_name, model) in provider.models() {
        // Check if this model has a specific rate limit for this group
        let has_model_group = model_has_group_limit(&model, group);

        if has_model_group {
            continue;
        }

        // Model doesn't have a group-specific limit, check fallbacks
        let has_model_default = model.rate_limits().is_some();
        let has_provider_group = provider_has_group_limit(provider, group);
        let has_provider_default = provider.rate_limits().is_some();

        let warning = match (has_model_default, has_provider_group, has_provider_default) {
            (true, _, _) => {
                format!("Group '{group}' for model '{provider_name}/{model_name}' will use model default rate limit")
            }
            (false, true, _) => {
                format!("Group '{group}' for model '{provider_name}/{model_name}' will use provider group rate limit")
            }
            (false, false, true) => {
                format!(
                    "Group '{group}' for model '{provider_name}/{model_name}' will fall back to provider default rate limit"
                )
            }
            (false, false, false) => {
                format!("Group '{group}' for model '{provider_name}/{model_name}' has no rate limit configured")
            }
        };

        warnings.push(warning);
    }

    // Check if group has no specific limits at all for this provider
    let has_provider_limit = provider_has_group_limit(provider, group);
    let has_any_model_limit = provider_has_any_model_with_group_limit(provider, group);

    if !has_provider_limit && !has_any_model_limit && provider.rate_limits().is_none() {
        let warning = format!("Group '{group}' has no rate limits configured for provider '{provider_name}'");
        warnings.push(warning);
    }
}

/// Check if a model has a rate limit for a specific group.
fn model_has_group_limit(model: &crate::ModelConfig, group: &str) -> bool {
    model
        .rate_limits()
        .and_then(|limits| limits.per_user.as_ref())
        .map(|per_user| per_user.groups.contains_key(group))
        .unwrap_or(false)
}

/// Check if a provider has a rate limit for a specific group.
fn provider_has_group_limit(provider: &LlmProviderConfig, group: &str) -> bool {
    provider
        .rate_limits()
        .as_ref()
        .and_then(|limits| limits.per_user.as_ref())
        .map(|per_user| per_user.groups.contains_key(group))
        .unwrap_or(false)
}

/// Check if any model in a provider has a rate limit for a specific group.
fn provider_has_any_model_with_group_limit(provider: &LlmProviderConfig, group: &str) -> bool {
    provider
        .models()
        .values()
        .any(|model| model_has_group_limit(model, group))
}

#[cfg(test)]
mod tests {
    use indoc::indoc;
    use insta::assert_debug_snapshot;
    use insta::assert_snapshot;

    use crate::Config;

    #[test]
    fn validation_logic_identifies_no_downstreams() {
        // Test that validation logic correctly identifies when no downstreams are configured
        let config = Config::default();
        let result = super::validate_has_downstreams(&config);
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();

        assert_snapshot!(error_msg, @r#"
        No downstream servers configured. Nexus requires at least one MCP server or LLM provider to function.

        Example configuration:

        For MCP servers:

          [mcp.servers.example]
          cmd = ["path/to/mcp-server"]

        For LLM providers:

          [llm.providers.openai]
          type = "openai"
          api_key = "{{ env.OPENAI_API_KEY }}"

        See https://nexusrouter.com/docs for more configuration examples.
        "#);
    }

    #[test]
    fn validation_fails_when_both_disabled() {
        let config_str = indoc! {r#"
            [mcp]
            enabled = false

            [llm]
            enabled = false
        "#};

        let config: Config = toml::from_str(config_str).unwrap();
        let result = super::validate_has_downstreams(&config);
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();

        assert_snapshot!(error_msg, @r#"
        No downstream servers configured. Nexus requires at least one MCP server or LLM provider to function.

        Example configuration:

        For MCP servers:

          [mcp.servers.example]
          cmd = ["path/to/mcp-server"]

        For LLM providers:

          [llm.providers.openai]
          type = "openai"
          api_key = "{{ env.OPENAI_API_KEY }}"

        See https://nexusrouter.com/docs for more configuration examples.
        "#);
    }

    #[test]
    fn validation_fails_when_mcp_enabled_but_no_servers() {
        let config_str = indoc! {r#"
            [mcp]
            enabled = true

            [llm]
            enabled = false
        "#};

        let config: Config = toml::from_str(config_str).unwrap();
        let result = super::validate_has_downstreams(&config);
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();

        assert_snapshot!(error_msg, @r#"
        No downstream servers configured. Nexus requires at least one MCP server or LLM provider to function.

        Example configuration:

        For MCP servers:

          [mcp.servers.example]
          cmd = ["path/to/mcp-server"]

        For LLM providers:

          [llm.providers.openai]
          type = "openai"
          api_key = "{{ env.OPENAI_API_KEY }}"

        See https://nexusrouter.com/docs for more configuration examples.
        "#);
    }

    #[test]
    fn validation_fails_when_llm_enabled_but_no_providers() {
        let config_str = indoc! {r#"
            [mcp]
            enabled = false

            [llm]
            enabled = true
        "#};

        let config: Config = toml::from_str(config_str).unwrap();
        let result = super::validate_has_downstreams(&config);
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();

        assert_snapshot!(error_msg, @r#"
        No downstream servers configured. Nexus requires at least one MCP server or LLM provider to function.

        Example configuration:

        For MCP servers:

          [mcp.servers.example]
          cmd = ["path/to/mcp-server"]

        For LLM providers:

          [llm.providers.openai]
          type = "openai"
          api_key = "{{ env.OPENAI_API_KEY }}"

        See https://nexusrouter.com/docs for more configuration examples.
        "#);
    }

    #[test]
    fn validation_passes_with_mcp_server() {
        let config_str = indoc! {r#"
            [mcp.servers.test]
            cmd = ["echo", "test"]
        "#};

        let config: Config = toml::from_str(config_str).unwrap();
        let result = super::validate_has_downstreams(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn validation_passes_with_llm_provider() {
        let config_str = indoc! {r#"
            [llm.providers.openai]
            type = "openai"
            api_key = "test-key"

            [llm.providers.openai.models.gpt-4]
        "#};

        let config: Config = toml::from_str(config_str).unwrap();
        let result = super::validate_has_downstreams(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn validation_passes_with_both_mcp_and_llm() {
        let config_str = indoc! {r#"
            [mcp.servers.test]
            cmd = ["echo", "test"]

            [llm.providers.openai]
            type = "openai"
            api_key = "test-key"

            [llm.providers.openai.models.gpt-4]
        "#};

        let config: Config = toml::from_str(config_str).unwrap();
        let result = super::validate_has_downstreams(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn validation_passes_when_mcp_disabled_but_llm_has_providers() {
        let config_str = indoc! {r#"
            [mcp]
            enabled = false

            [llm.providers.openai]
            type = "openai"
            api_key = "test-key"

            [llm.providers.openai.models.gpt-4]
        "#};

        let config: Config = toml::from_str(config_str).unwrap();
        let result = super::validate_has_downstreams(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn validation_passes_when_llm_disabled_but_mcp_has_servers() {
        let config_str = indoc! {r#"
            [llm]
            enabled = false

            [mcp.servers.test]
            cmd = ["echo", "test"]
        "#};

        let config: Config = toml::from_str(config_str).unwrap();
        let result = super::validate_has_downstreams(&config);
        assert!(result.is_ok());
    }

    #[test]
    fn rate_limit_validation_with_groups() {
        let config = indoc! {r#"
            [server.client_identification]
            enabled = true
            client_id.jwt_claim = "sub"
            group_id.jwt_claim = "plan"

            [server.client_identification.validation]
            group_values = ["free", "pro"]

            [llm.providers.openai]
            type = "openai"
            api_key = "test-key"

            [llm.providers.openai.rate_limits.per_user]
            input_token_limit = 50000
            interval = "60s"

            [llm.providers.openai.rate_limits.per_user.groups]
            free = { input_token_limit = 10000, interval = "60s" }
            pro = { input_token_limit = 100000, interval = "60s" }

            [llm.providers.openai.models.gpt-4]
        "#};

        let config: Config = toml::from_str(config).unwrap();
        let warnings = super::validate_rate_limits(&config).unwrap();

        // Should have warnings about model fallbacks
        assert_debug_snapshot!(warnings, @r#"
        [
            "Group 'free' for model 'openai/gpt-4' will use provider group rate limit",
            "Group 'pro' for model 'openai/gpt-4' will use provider group rate limit",
        ]
        "#);
    }

    #[test]
    fn rate_limits_without_client_identification_fails() {
        let config = indoc! {r#"
            [server.client_identification]
            enabled = false

            [llm.providers.openai]
            type = "openai"
            api_key = "test-key"

            [llm.providers.openai.rate_limits.per_user]
            input_token_limit = 10000
            interval = "60s"

            [llm.providers.openai.models.gpt-4]
        "#};

        let config: Config = toml::from_str(config).unwrap();
        let result = super::validate_rate_limits(&config);

        assert!(result.is_err());
        let error = result.unwrap_err().to_string();

        assert_snapshot!(error, @"LLM rate limits are configured but client identification is not enabled. Enable client identification in [server.client_identification]");
    }

    #[test]
    fn model_rate_limits_without_client_identification_fails() {
        let config = indoc! {r#"
            [server.client_identification]
            enabled = false

            [llm.providers.openai]
            type = "openai"
            api_key = "test-key"

            [llm.providers.openai.models.gpt-4.rate_limits.per_user]
            input_token_limit = 5000
            interval = "60s"
        "#};

        let config: Config = toml::from_str(config).unwrap();
        let result = super::validate_rate_limits(&config);

        assert!(result.is_err());
        let error = result.unwrap_err().to_string();
        assert_snapshot!(error, @"LLM rate limits are configured but client identification is not enabled. Enable client identification in [server.client_identification]");
    }

    #[test]
    fn group_id_without_allowed_groups_fails() {
        let config = indoc! {r#"
            [server.client_identification]
            enabled = true
            client_id.jwt_claim = "sub"
            group_id.jwt_claim = "plan"

            [llm.providers.openai]
            type = "openai"
            api_key = "test-key"

            [llm.providers.openai.rate_limits.per_user]
            input_token_limit = 5000
            interval = "60s"

            [llm.providers.openai.models.gpt-4]
        "#};

        let config: Config = toml::from_str(config).unwrap();
        let result = super::validate_rate_limits(&config);

        assert!(result.is_err());
        let error = result.unwrap_err().to_string();

        assert_snapshot!(error, @"group_id is configured for client identification but validation.group_values is empty. Define group_values in [server.client_identification.validation]");
    }

    #[test]
    fn group_rate_limits_without_group_id_fails() {
        let config = indoc! {r#"
            [server.client_identification]
            enabled = true
            client_id.jwt_claim = "sub"

            [llm.providers.openai]
            type = "openai"
            api_key = "test-key"

            [llm.providers.openai.rate_limits.per_user]
            input_token_limit = 5000
            interval = "60s"

            [llm.providers.openai.rate_limits.per_user.groups]
            free = { input_token_limit = 10000, interval = "60s" }

            [llm.providers.openai.models.gpt-4]
        "#};

        let config: Config = toml::from_str(config).unwrap();
        let result = super::validate_rate_limits(&config);

        assert!(result.is_err());
        let error = result.unwrap_err().to_string();

        assert_snapshot!(error, @r#"
        Group-based rate limits are configured but group_id is not set in client identification.
        To fix this, add a group_id configuration to your [server.client_identification] section, for example:

        [server.client_identification]
        enabled = true
        client_id.http_header = "X-Client-ID"      # or client_id.jwt_claim = "sub"
        group_id.http_header = "X-Group-ID"        # or group_id.jwt_claim = "groups"

        [server.client_identification.validation]
        group_values = ["basic", "premium", "enterprise"]
        "#);
    }

    #[test]
    fn rate_limit_validation_invalid_group() {
        let config = indoc! {r#"
            [server.client_identification]
            enabled = true
            client_id.jwt_claim = "sub"
            group_id.jwt_claim = "plan"

            [server.client_identification.validation]
            group_values = ["free", "pro"]

            [llm.providers.openai]
            type = "openai"
            api_key = "test-key"

            [llm.providers.openai.rate_limits.per_user]
            input_token_limit = 50000
            interval = "60s"

            [llm.providers.openai.rate_limits.per_user.groups]
            enterprise = { input_token_limit = 1000000, interval = "60s" }

            [llm.providers.openai.models.gpt-4]
        "#};

        let config: Config = toml::from_str(config).unwrap();
        let result = super::validate_rate_limits(&config);

        assert!(result.is_err());
        let error = result.unwrap_err().to_string();

        assert_snapshot!(error, @"Group 'enterprise' in provider 'openai' rate limits not found in group_values");
    }

    #[test]
    fn missing_optional_environment_variable_should_be_handled() {
        // Test that when an optional environment variable like OPENAI_BASE_URL is missing,
        // the configuration loads successfully and the optional field is omitted
        let config_str = indoc! {r#"
            [llm.providers.openai]
            type = "openai"
            api_key = "sk-1234567890abcdef"
            base_url = "{{ env.OPENAI_BASE_URL_MISSING }}"

            [llm.providers.openai.models.gpt-4]
        "#};

        // This should succeed - the missing environment variable for optional field should be handled gracefully
        let raw_config: toml::Value = toml::from_str(config_str).unwrap();
        
        // This should not fail even though OPENAI_BASE_URL_MISSING is not set
        let result = load_from_value(raw_config);
        assert!(result.is_ok(), "Configuration loading should succeed when optional env var is missing: {:?}", result);
        
        let config = result.unwrap();
        
        // Verify the configuration loaded correctly
        assert!(config.llm.enabled());
        assert!(config.llm.has_providers());
        
        let openai_provider = config.llm.providers.get("openai").unwrap();
        
        // The base_url should be None since the environment variable was missing
        assert!(openai_provider.base_url().is_none(), "base_url should be None when env var is missing");
    }

    /// Helper function for testing - loads config from a TOML value instead of file
    fn load_from_value(mut raw_config: toml::Value) -> anyhow::Result<Config> {
        use serde::Deserialize;
        // Try expanding dynamic strings, but handle optional fields gracefully
        if let Err(err) = super::expand_dynamic_strings(&mut Vec::new(), &mut raw_config) {
            // Check if this error is from an optional field with missing env var
            let err_str = err.to_string();
            if err_str.contains("Failed to expand dynamic string at path") {
                if let Some(path_part) = super::extract_path_from_error(&err_str) {
                    if super::is_optional_env_field(&path_part) && super::is_missing_env_var_error(&err) {
                        // Remove the optional field and try again
                        super::remove_field_from_config(&mut raw_config, &path_part);
                        super::expand_dynamic_strings(&mut Vec::new(), &mut raw_config)?;
                    } else {
                        return Err(err);
                    }
                } else {
                    return Err(err);
                }
            } else {
                return Err(err);
            }
        }

        let config = Config::deserialize(raw_config)?;
        super::validate_has_downstreams(&config)?;

        Ok(config)
    }
}
