use std::{path::Path, str::FromStr};

use anyhow::bail;
use indoc::formatdoc;
use serde::Deserialize;
use serde_dynamic_string::DynamicString;
use std::fmt::Write;
use toml::Value;

use crate::{Config, LlmProviderConfig};

pub fn load<P: AsRef<Path>>(path: P) -> anyhow::Result<Config> {
    let path = path.as_ref().to_path_buf();
    let content = std::fs::read_to_string(&path)?;
    let mut raw_config: Value = toml::from_str(&content)?;

    expand_dynamic_strings(&mut Vec::new(), &mut raw_config)?;

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
        let message = formatdoc! {r#"
            No downstream servers configured. Nexus requires at least one MCP server or LLM provider to function.

            Example configuration:

            For MCP servers:

              [mcp.servers.example]
              cmd = ["path/to/mcp-server"]

            For LLM providers:

              [llm.providers.openai]
              type = "openai"
              api_key = "{{{{ env.OPENAI_API_KEY }}}}"

            See https://nexusrouter.com/docs for more configuration examples.
        "#};

        bail!(message);
    }

    Ok(())
}

fn expand_dynamic_strings<'a>(path: &mut Vec<Result<&'a str, usize>>, value: &'a mut Value) -> anyhow::Result<()> {
    match value {
        Value::String(s) => match DynamicString::<String>::from_str(s) {
            Ok(out) => *s = out.into_inner(),
            Err(err) => {
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
    let has_llm_rate_limits = config.llm.providers.values().any(|provider| {
        !provider.rate_limits.groups.is_empty() 
            || provider.rate_limits.default.is_some()
            || provider.models.values().any(|model| 
                !model.rate_limits.groups.is_empty() || model.rate_limits.default.is_some()
            )
    });
    
    // If LLM rate limits are defined, client identification MUST be enabled
    if has_llm_rate_limits && !config.server.client_identification.enabled {
        anyhow::bail!("LLM rate limits are configured but client identification is not enabled. Enable client identification in [server.client_identification]");
    }

    // Skip further validation if client identification is not enabled (and no rate limits)
    if !config.server.client_identification.enabled {
        return Ok(warnings);
    }
    
    // If group_id is configured, allowed_groups MUST be defined
    if config.server.client_identification.group_id.is_some() 
        && config.server.client_identification.allowed_groups.is_empty() {
        anyhow::bail!("group_id is configured for client identification but allowed_groups is empty. Define allowed_groups in [server.client_identification]");
    }
    
    // Check if any provider has group-based rate limits
    let has_group_rate_limits = config.llm.providers.values().any(|provider| {
        !provider.rate_limits.groups.is_empty() 
            || provider.models.values().any(|model| !model.rate_limits.groups.is_empty())
    });
    
    // If group rate limits are defined, group identification MUST be configured
    if has_group_rate_limits && config.server.client_identification.group_id.is_none() {
        anyhow::bail!("Group-based rate limits are configured but group_id is not set in client identification. Configure group_id in [server.client_identification]");
    }

    // Validate all group names in rate limits exist in allowed_groups
    for (provider_name, provider) in &config.llm.providers {
        validate_provider_groups(config, provider_name, provider)?;

        // Generate warnings for fallback scenarios
        if config.server.client_identification.allowed_groups.is_empty() {
            continue;
        }

        for group in &config.server.client_identification.allowed_groups {
            check_group_fallbacks(group, provider_name, provider, &mut warnings);
        }
    }

    Ok(warnings)
}

fn validate_provider_groups(
    config: &Config,
    provider_name: &str,
    provider: &LlmProviderConfig,
) -> anyhow::Result<()> {
    // Check provider-level group rate limits
    for group_name in provider.rate_limits.groups.keys() {
        if !config.server.client_identification.allowed_groups.contains(group_name) {
            anyhow::bail!(
                "Group '{group_name}' in provider '{provider_name}' rate limits not found in allowed_groups",
            );
        }
    }

    // Check model-level group rate limits
    for (model_name, model) in &provider.models {
        for group_name in model.rate_limits.groups.keys() {
            if !config.server.client_identification.allowed_groups.contains(group_name) {
                anyhow::bail!(
                    "Group '{group_name}' in model '{provider_name}/{model_name}' rate limits not found in allowed_groups",
                );
            }
        }
    }

    Ok(())
}

fn check_group_fallbacks(
    group: &str,
    provider_name: &str,
    provider: &LlmProviderConfig,
    warnings: &mut Vec<String>,
) {
    // Check each model's fallback situation
    for (model_name, model) in &provider.models {
        let has_model_group = model.rate_limits.groups.contains_key(group);
        if has_model_group {
            continue;
        }

        let has_model_default = model.rate_limits.default.is_some();
        let has_provider_group = provider.rate_limits.groups.contains_key(group);
        let has_provider_default = provider.rate_limits.default.is_some();

        let warning = match (has_model_default, has_provider_group, has_provider_default) {
            (true, _, _) => format!(
                "Group '{group}' for model '{provider_name}/{model_name}' will use model default rate limit",
            ),
            (false, true, _) => format!(
                "Group '{group}' for model '{provider_name}/{model_name}' will use provider group rate limit",
            ),
            (false, false, true) => format!(
                "Group '{group}' for model '{provider_name}/{model_name}' will fall back to provider default rate limit",
            ),
            (false, false, false) => {
                format!("Group '{group}' for model '{provider_name}/{model_name}' has no rate limit configured",)
            }
        };

        warnings.push(warning);
    }

    // Check if group has no specific limits at all for this provider
    let has_provider_limit = provider.rate_limits.groups.contains_key(group);
    let has_any_model_limit = provider
        .models
        .values()
        .any(|m| m.rate_limits.groups.contains_key(group));

    if !has_provider_limit && !has_any_model_limit && provider.rate_limits.default.is_none() {
        let warning = format!("Group '{group}' has no rate limits configured for provider '{provider_name}'",);

        warnings.push(warning);
    }
}