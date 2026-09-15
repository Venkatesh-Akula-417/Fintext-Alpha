//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Secrets Management Abstraction Layer
//! Decoupled Secret Resolution: Local Environment vs AWS Secrets Manager
//! ═══════════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::sync::Arc;
use tracing::{debug, error, info};

/// Common trait for resolving secrets across different backends.
pub trait SecretsProvider: Send + Sync {
    /// Retrieve an optional secret value by key name.
    fn get(&self, key: &str) -> Option<String>;

    /// Retrieve a mandatory secret value or return an error if missing.
    fn required(&self, key: &str) -> Result<String, String> {
        self.get(key)
            .filter(|v| !v.trim().is_empty())
            .ok_or_else(|| format!("Required secret '{}' is missing or empty", key))
    }

    /// Retrieve all configured secrets as a key-value map.
    fn all(&self) -> HashMap<String, String>;
}

/// Redact a secret value for safe logging (e.g. `abcd...1234`).
pub fn redact_secret(val: &str) -> String {
    if val.len() <= 8 {
        "****".to_string()
    } else {
        format!("{}...{}", &val[..4], &val[val.len() - 4..])
    }
}

/// Configuration specifying the active secrets provider and credentials/ARN.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretsConfig {
    /// Provider backend: "env" (default) or "aws_secrets_manager"
    pub provider: String,
    /// AWS region for Secrets Manager (default: "us-east-1")
    pub aws_region: String,
    /// ARN or secret name in AWS Secrets Manager
    pub aws_secret_arn: String,
}

impl Default for SecretsConfig {
    fn default() -> Self {
        Self {
            provider: "env".to_string(),
            aws_region: "us-east-1".to_string(),
            aws_secret_arn: String::new(),
        }
    }
}

impl SecretsConfig {
    /// Load configuration with priority: environment variables -> config/config.yaml -> defaults.
    pub fn from_env_or_config() -> Self {
        let mut cfg = Self::default();

        let config_paths = [
            env::var("CONFIG_PATH").unwrap_or_default(),
            "config/config.yaml".to_string(),
            "config.yaml".to_string(),
            "../config/config.yaml".to_string(),
            "../../config/config.yaml".to_string(),
        ];

        for path in &config_paths {
            if path.is_empty() {
                continue;
            }
            if let Ok(contents) = fs::read_to_string(path) {
                let mut in_secrets = false;

                for line in contents.lines() {
                    let trimmed = line.trim();
                    if trimmed.starts_with("secrets:") {
                        in_secrets = true;
                        continue;
                    }
                    if in_secrets
                        && !line.starts_with(' ')
                        && !line.starts_with('\t')
                        && !trimmed.is_empty()
                    {
                        in_secrets = false;
                    }
                    if in_secrets {
                        if let Some((k, v)) = trimmed.split_once(':') {
                            let key = k.trim();
                            let val = v
                                .split('#')
                                .next()
                                .unwrap_or("")
                                .trim()
                                .trim_matches('"')
                                .trim_matches('\'');
                            match key {
                                "provider" => cfg.provider = val.to_string(),
                                "aws_region" => cfg.aws_region = val.to_string(),
                                "aws_secret_arn" => cfg.aws_secret_arn = val.to_string(),
                                _ => {}
                            }
                        }
                    }
                }
                break;
            }
        }

        // Environment variable overrides
        if let Ok(p) = env::var("SECRETS_PROVIDER") {
            if !p.trim().is_empty() {
                cfg.provider = p.trim().to_lowercase();
            }
        }
        if let Ok(r) = env::var("AWS_REGION") {
            if !r.trim().is_empty() {
                cfg.aws_region = r.trim().to_string();
            }
        }
        if let Ok(arn) = env::var("AWS_SECRET_ARN") {
            if !arn.trim().is_empty() {
                cfg.aws_secret_arn = arn.trim().to_string();
            }
        }

        cfg
    }
}

/// Environment-based secrets provider (wraps `std::env::var`).
#[derive(Debug, Default, Clone)]
pub struct EnvSecretsProvider {
    overrides: HashMap<String, String>,
}

impl EnvSecretsProvider {
    /// Create a new standard environment secrets provider.
    pub fn new() -> Self {
        Self {
            overrides: HashMap::new(),
        }
    }

    /// Create an environment provider with custom overrides (useful for testing).
    pub fn with_overrides(overrides: HashMap<String, String>) -> Self {
        Self { overrides }
    }
}

impl SecretsProvider for EnvSecretsProvider {
    fn get(&self, key: &str) -> Option<String> {
        if let Some(val) = self.overrides.get(key) {
            return Some(val.clone());
        }
        env::var(key).ok()
    }

    fn all(&self) -> HashMap<String, String> {
        let mut map: HashMap<String, String> = env::vars().collect();
        for (k, v) in &self.overrides {
            map.insert(k.clone(), v.clone());
        }
        map
    }
}

/// AWS Secrets Manager provider.
#[derive(Debug, Clone)]
pub struct AwsSecretsManagerProvider {
    secrets: HashMap<String, String>,
    region: String,
    secret_arn: String,
}

impl AwsSecretsManagerProvider {
    /// Create directly from a JSON string payload (useful for unit tests & mocking).
    pub fn from_json_str(json_str: &str, region: &str, secret_arn: &str) -> Result<Self, String> {
        let parsed: serde_json::Value = serde_json::from_str(json_str)
            .map_err(|e| format!("Failed to parse AWS Secrets Manager JSON payload: {}", e))?;

        let obj = parsed
            .as_object()
            .ok_or_else(|| "AWS Secrets Manager payload must be a JSON object".to_string())?;

        let mut secrets = HashMap::new();
        for (k, v) in obj {
            let val_str = match v {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::Bool(b) => b.to_string(),
                other => other.to_string(),
            };
            secrets.insert(k.clone(), val_str);
        }

        Ok(Self {
            secrets,
            region: region.to_string(),
            secret_arn: secret_arn.to_string(),
        })
    }

    /// Asynchronously fetch and parse secret from AWS Secrets Manager using standard AWS credential chain.
    pub async fn load(region: &str, secret_arn: &str) -> Result<Self, String> {
        if secret_arn.trim().is_empty() {
            return Err("AWS Secrets Manager secret_arn cannot be empty".to_string());
        }

        info!(
            "[AWS Secrets Manager] Fetching secret from region='{}', secret_id='{}'",
            region, secret_arn
        );

        let region_str = region.to_string();
        let arn_str = secret_arn.to_string();

        let fetch_operation = async {
            let aws_cfg = aws_config::defaults(aws_config::BehaviorVersion::latest())
                .region(aws_sdk_secretsmanager::config::Region::new(
                    region_str.clone(),
                ))
                .load()
                .await;

            let client = aws_sdk_secretsmanager::Client::new(&aws_cfg);

            let resp = client
                .get_secret_value()
                .secret_id(&arn_str)
                .send()
                .await
                .map_err(|e| {
                    format!(
                        "Failed to retrieve secret from AWS Secrets Manager ({}): {}",
                        arn_str, e
                    )
                })?;

            let secret_str = if let Some(ref s) = resp.secret_string() {
                s.to_string()
            } else if let Some(ref b) = resp.secret_binary() {
                String::from_utf8(b.as_ref().to_vec()).map_err(|e| {
                    format!("Failed to decode binary secret payload as UTF-8: {}", e)
                })?
            } else {
                return Err("AWS Secrets Manager response contained neither secret_string nor secret_binary".to_string());
            };

            Self::from_json_str(&secret_str, &region_str, &arn_str)
        };

        let provider =
            match tokio::time::timeout(std::time::Duration::from_secs(3), fetch_operation).await {
                Ok(res) => res?,
                Err(_) => {
                    return Err(format!(
                    "Failed to retrieve secret from AWS Secrets Manager ({}): connection timed out",
                    secret_arn
                ))
                }
            };

        info!(
            "[AWS Secrets Manager] Successfully loaded {} secrets from '{}'",
            provider.secrets.len(),
            secret_arn
        );

        Ok(provider)
    }

    /// Get region.
    pub fn region(&self) -> &str {
        &self.region
    }

    /// Get ARN.
    pub fn secret_arn(&self) -> &str {
        &self.secret_arn
    }
}

impl SecretsProvider for AwsSecretsManagerProvider {
    fn get(&self, key: &str) -> Option<String> {
        self.secrets.get(key).cloned()
    }

    fn all(&self) -> HashMap<String, String> {
        self.secrets.clone()
    }
}

/// Resolved required secrets for API server startup.
#[derive(Debug, Clone)]
pub struct RequiredSecrets {
    pub jwt_secret: String,
    pub admin_token: String,
    pub database_url: String,
}

/// Initialize the configured SecretsProvider.
pub async fn init_secrets_provider(
    cfg: &SecretsConfig,
) -> Result<Arc<dyn SecretsProvider>, String> {
    match cfg.provider.to_lowercase().as_str() {
        "aws_secrets_manager" | "aws" => {
            if cfg.aws_secret_arn.trim().is_empty() {
                let err = "AWS Secrets Manager enabled (secrets.provider='aws_secrets_manager') but 'aws_secret_arn' is empty".to_string();
                error!("[Secrets Manager] {}", err);
                return Err(err);
            }
            let provider =
                AwsSecretsManagerProvider::load(&cfg.aws_region, &cfg.aws_secret_arn).await?;
            Ok(Arc::new(provider))
        }
        "env" | "" => {
            debug!("[Secrets Manager] Initializing Environment secrets provider");
            Ok(Arc::new(EnvSecretsProvider::new()))
        }
        other => {
            let err = format!(
                "Unknown secrets provider '{}'. Supported values are 'env' and 'aws_secrets_manager'",
                other
            );
            error!("[Secrets Manager] {}", err);
            Err(err)
        }
    }
}

/// Validate that mandatory secrets are present.
/// In production mode or when provider is AWS Secrets Manager, missing secrets trigger immediate failure.
/// In development mode with provider="env", fallback defaults are utilized for non-breaking backward compatibility.
pub fn validate_required_secrets(
    provider: &dyn SecretsProvider,
    provider_name: &str,
    prod_mode: bool,
) -> Result<RequiredSecrets, String> {
    let strict = prod_mode
        || provider_name == "aws_secrets_manager"
        || provider_name == "aws"
        || env::var("STRICT_SECRETS_CHECK").as_deref() == Ok("1");

    if strict {
        let jwt_secret = provider.required("JWT_SECRET")?;
        let admin_token = provider.required("ADMIN_TOKEN")?;
        let database_url = provider.get("DATABASE_URL").unwrap_or_else(|| {
            "postgres://fintext:fintext@localhost:5432/fintext_metadata".to_string()
        });

        info!(
            "[Secrets Manager] Strict validation passed: JWT_SECRET=[{}], ADMIN_TOKEN=[{}], DATABASE_URL=[{}]",
            redact_secret(&jwt_secret),
            redact_secret(&admin_token),
            redact_secret(&database_url)
        );

        Ok(RequiredSecrets {
            jwt_secret,
            admin_token,
            database_url,
        })
    } else {
        let jwt_secret = provider
            .get("JWT_SECRET")
            .unwrap_or_else(|| crate::DEFAULT_DEV_JWT_SECRET.to_string());
        let admin_token = provider
            .get("ADMIN_TOKEN")
            .unwrap_or_else(|| crate::DEFAULT_DEV_ADMIN_TOKEN.to_string());
        let database_url = provider.get("DATABASE_URL").unwrap_or_else(|| {
            "postgres://fintext:fintext@localhost:5432/fintext_metadata".to_string()
        });

        debug!(
            "[Secrets Manager] Development secrets resolved: JWT_SECRET=[{}], ADMIN_TOKEN=[{}], DATABASE_URL=[{}]",
            redact_secret(&jwt_secret),
            redact_secret(&admin_token),
            redact_secret(&database_url)
        );

        Ok(RequiredSecrets {
            jwt_secret,
            admin_token,
            database_url,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_secrets_redaction() {
        assert_eq!(redact_secret("short"), "****");
        assert_eq!(redact_secret("12345678"), "****");
        assert_eq!(redact_secret("123456789"), "1234...6789");
        assert_eq!(
            redact_secret("fintext-admin-dev-secret-token"),
            "fint...oken"
        );
    }

    #[test]
    fn test_env_secrets_provider_with_overrides() {
        let mut overrides = HashMap::new();
        overrides.insert("TEST_KEY_1".to_string(), "secret_val_1".to_string());
        overrides.insert("TEST_KEY_2".to_string(), "secret_val_2".to_string());

        let provider = EnvSecretsProvider::with_overrides(overrides);
        assert_eq!(provider.get("TEST_KEY_1"), Some("secret_val_1".to_string()));
        assert_eq!(provider.get("TEST_KEY_2"), Some("secret_val_2".to_string()));
        assert_eq!(provider.get("NONEXISTENT_KEY_XYZ"), None);

        assert_eq!(provider.required("TEST_KEY_1").unwrap(), "secret_val_1");
        assert!(provider.required("NONEXISTENT_KEY_XYZ").is_err());
    }

    #[test]
    fn test_aws_secrets_manager_provider_from_json() {
        let json_payload = r#"{
            "JWT_SECRET": "my_super_jwt_secret_2026",
            "ADMIN_TOKEN": "my_admin_token_xyz",
            "DATABASE_URL": "postgres://user:pass@db:5432/fintext",
            "FINNHUB_API_KEY": "finnhub_key_123",
            "POLYGON_API_KEY": "poly_key_456"
        }"#;

        let provider = AwsSecretsManagerProvider::from_json_str(
            json_payload,
            "us-east-1",
            "arn:aws:secretsmanager:us-east-1:123456789012:secret:fintext-prod",
        )
        .expect("Valid JSON should parse cleanly");

        assert_eq!(provider.region(), "us-east-1");
        assert_eq!(
            provider.secret_arn(),
            "arn:aws:secretsmanager:us-east-1:123456789012:secret:fintext-prod"
        );
        assert_eq!(
            provider.get("JWT_SECRET"),
            Some("my_super_jwt_secret_2026".to_string())
        );
        assert_eq!(
            provider.get("ADMIN_TOKEN"),
            Some("my_admin_token_xyz".to_string())
        );
        assert_eq!(
            provider.get("DATABASE_URL"),
            Some("postgres://user:pass@db:5432/fintext".to_string())
        );
        assert_eq!(
            provider.get("FINNHUB_API_KEY"),
            Some("finnhub_key_123".to_string())
        );
        assert_eq!(
            provider.get("POLYGON_API_KEY"),
            Some("poly_key_456".to_string())
        );
        assert_eq!(provider.get("UNKNOWN_KEY"), None);

        let all = provider.all();
        assert_eq!(all.len(), 5);
    }

    #[test]
    fn test_aws_secrets_manager_invalid_json() {
        let bad_json = r#"{"JWT_SECRET": "unclosed_string"#;
        let res = AwsSecretsManagerProvider::from_json_str(bad_json, "us-east-1", "my-arn");
        assert!(res.is_err());
    }

    #[test]
    fn test_secrets_config_defaults() {
        let cfg = SecretsConfig::default();
        assert_eq!(cfg.provider, "env");
        assert_eq!(cfg.aws_region, "us-east-1");
        assert_eq!(cfg.aws_secret_arn, "");
    }

    #[test]
    fn test_strict_validation_missing_key() {
        let orig_admin = std::env::var("ADMIN_TOKEN").ok();
        let orig_jwt = std::env::var("JWT_SECRET").ok();
        let orig_db = std::env::var("DATABASE_URL").ok();
        let orig_strict = std::env::var("STRICT_SECRETS_CHECK").ok();

        std::env::remove_var("ADMIN_TOKEN");
        std::env::remove_var("JWT_SECRET");
        std::env::remove_var("DATABASE_URL");
        std::env::remove_var("STRICT_SECRETS_CHECK");

        let mut map = HashMap::new();
        map.insert("JWT_SECRET".to_string(), "jwt_ok".to_string());
        // Missing ADMIN_TOKEN and DATABASE_URL
        let provider = EnvSecretsProvider::with_overrides(map);

        let res = validate_required_secrets(&provider, "aws_secrets_manager", false);

        if let Some(v) = orig_admin {
            std::env::set_var("ADMIN_TOKEN", v);
        }
        if let Some(v) = orig_jwt {
            std::env::set_var("JWT_SECRET", v);
        }
        if let Some(v) = orig_db {
            std::env::set_var("DATABASE_URL", v);
        }
        if let Some(v) = orig_strict {
            std::env::set_var("STRICT_SECRETS_CHECK", v);
        }

        assert!(res.is_err());
        assert!(res.unwrap_err().contains("ADMIN_TOKEN"));
    }

    #[test]
    fn test_dev_validation_fallback_defaults() {
        let orig_admin = std::env::var("ADMIN_TOKEN").ok();
        let orig_jwt = std::env::var("JWT_SECRET").ok();
        let orig_db = std::env::var("DATABASE_URL").ok();

        std::env::remove_var("ADMIN_TOKEN");
        std::env::remove_var("JWT_SECRET");
        std::env::remove_var("DATABASE_URL");

        let map = HashMap::new();
        let provider = EnvSecretsProvider::with_overrides(map);

        let res = validate_required_secrets(&provider, "env", false);

        if let Some(v) = orig_admin {
            std::env::set_var("ADMIN_TOKEN", v);
        }
        if let Some(v) = orig_jwt {
            std::env::set_var("JWT_SECRET", v);
        }
        if let Some(v) = orig_db {
            std::env::set_var("DATABASE_URL", v);
        }

        let res = res.unwrap();
        assert_eq!(res.jwt_secret, crate::DEFAULT_DEV_JWT_SECRET);
        assert_eq!(res.admin_token, crate::DEFAULT_DEV_ADMIN_TOKEN);
        assert!(res.database_url.contains("postgres://"));
    }
}
