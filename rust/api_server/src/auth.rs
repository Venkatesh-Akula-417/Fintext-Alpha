//! ═══════════════════════════════════════════════════════════════════════════════
//! FinText-Alpha-Vectorizer — Production JWT Authentication & Token Management
//! ═══════════════════════════════════════════════════════════════════════════════

use crate::state::AppState;
use axum::extract::{Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use chrono::Utc;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};
use utoipa::ToSchema;

/// Default token expiration in seconds (1 hour).
pub const DEFAULT_JWT_EXPIRY_SECS: u64 = 3600;

/// Standard JWT Claims payload for authenticated clients.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct Claims {
    /// Subject (User ID / Client identifier)
    #[schema(example = "quant_fund_01")]
    pub sub: String,
    /// Expiration timestamp in seconds since Unix epoch
    #[schema(example = 1787943989)]
    pub exp: usize,
    /// Issued-at timestamp in seconds since Unix epoch
    #[schema(example = 1787940389)]
    pub iat: usize,
    /// Client role (e.g. "institutional", "admin", "trader")
    #[serde(default = "default_role")]
    #[schema(example = "institutional")]
    pub role: String,
    /// Optional active organization identifier UUID
    #[serde(default)]
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub org_id: Option<String>,
    /// Optional flag indicating whether token is scoped exclusively for Sandbox simulation mode
    #[serde(default)]
    #[schema(example = false)]
    pub sandbox: Option<bool>,
}

fn default_role() -> String {
    "institutional".to_string()
}

/// Request payload for token issuance (`POST /auth/token`).
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct IssueTokenRequest {
    /// Target User or Institutional Fund ID
    #[schema(example = "quant_fund_alpha")]
    pub user_id: String,
    /// Optional expiration duration in seconds (default: 3600s = 1 hour)
    #[serde(default)]
    #[schema(example = 3600)]
    pub expires_in_seconds: Option<u64>,
    /// Optional institutional role assignment (default: "institutional")
    #[serde(default)]
    #[schema(example = "institutional")]
    pub role: Option<String>,
    /// Optional active organization identifier
    #[serde(default)]
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub org_id: Option<String>,
    /// Optional flag requesting an explicit Sandbox simulation token
    #[serde(default)]
    #[schema(example = false)]
    pub sandbox: Option<bool>,
}

/// Successful response for token issuance.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct IssueTokenResponse {
    /// Signed HMAC-SHA256 JWT Token string
    #[schema(example = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9...")]
    pub token: String,
    /// Token Authorization scheme
    #[schema(example = "Bearer")]
    pub token_type: String,
    /// Token lifetime duration in seconds
    #[schema(example = 3600)]
    pub expires_in: u64,
    /// Target authenticated user ID
    #[schema(example = "quant_fund_alpha")]
    pub user_id: String,
    /// Assigned role
    #[schema(example = "institutional")]
    pub role: String,
    /// Active organization context (if selected)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "550e8400-e29b-41d4-a716-446655440000")]
    pub org_id: Option<String>,
    /// Whether this token is scoped for sandbox mode
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = false)]
    pub sandbox: Option<bool>,
}

/// Standardized JSON error response for authentication errors.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AuthErrorResponse {
    /// Error category name
    #[schema(example = "Unauthorized")]
    pub error: String,
    /// Detailed diagnostic message
    #[schema(example = "Missing or invalid Bearer token")]
    pub message: String,
}

/// Generates a signed JWT with optional organization and sandbox context.
pub fn generate_jwt_with_sandbox(
    user_id: &str,
    expires_in_secs: u64,
    role: Option<&str>,
    org_id: Option<&str>,
    sandbox: Option<bool>,
    secret: &[u8],
) -> Result<String, jsonwebtoken::errors::Error> {
    let now = Utc::now().timestamp() as usize;
    let exp = now + (expires_in_secs as usize);

    let claims = Claims {
        sub: user_id.trim().to_string(),
        exp,
        iat: now,
        role: role.unwrap_or("institutional").to_string(),
        org_id: org_id.map(|s| s.trim().to_string()),
        sandbox,
    };

    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret),
    )
}

/// Generates a signed JWT with optional organization context.
pub fn generate_jwt_with_org(
    user_id: &str,
    expires_in_secs: u64,
    role: Option<&str>,
    org_id: Option<&str>,
    secret: &[u8],
) -> Result<String, jsonwebtoken::errors::Error> {
    generate_jwt_with_sandbox(user_id, expires_in_secs, role, org_id, None, secret)
}

/// Generates a signed JWT for a given user ID and expiration duration.
pub fn generate_jwt(
    user_id: &str,
    expires_in_secs: u64,
    role: Option<&str>,
    secret: &[u8],
) -> Result<String, jsonwebtoken::errors::Error> {
    generate_jwt_with_org(user_id, expires_in_secs, role, None, secret)
}

/// Helper function to determine if request is in sandbox mode from Axum extensions.
pub fn is_sandbox_request(extensions: &axum::http::Extensions) -> bool {
    extensions
        .get::<crate::sandbox::SandboxContext>()
        .map(|ctx| ctx.is_sandbox)
        .unwrap_or(false)
}

/// Validates and decodes a signed JWT, verifying signature and expiration.
pub fn validate_jwt(token: &str, secret: &[u8]) -> Result<Claims, jsonwebtoken::errors::Error> {
    let mut validation = Validation::default();
    validation.validate_exp = true;
    validation.leeway = 5; // 5-second clock skew tolerance

    let token_data = decode::<Claims>(token, &DecodingKey::from_secret(secret), &validation)?;

    Ok(token_data.claims)
}

/// Issue Temporary Institutional JWT Token.
///
/// Generates a signed HMAC-SHA256 JWT bearer token for institutional API access.
/// Protected by the `X-Admin-Token` header in staging and testing environments.
#[utoipa::path(
    post,
    path = "/auth/token",
    tag = "Authentication",
    params(
        ("x-admin-token" = Option<String>, Header, description = "Administrative secret token authorizing JWT issuance")
    ),
    request_body = IssueTokenRequest,
    responses(
        (status = 200, description = "JWT Token successfully issued", body = IssueTokenResponse),
        (status = 400, description = "Bad request (missing user_id)", body = AuthErrorResponse),
        (status = 401, description = "Unauthorized (invalid or missing X-Admin-Token)", body = AuthErrorResponse)
    )
)]
pub async fn issue_token_handler(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<IssueTokenRequest>,
) -> Result<Json<IssueTokenResponse>, (StatusCode, Json<AuthErrorResponse>)> {
    let user_id = payload.user_id.trim();
    if user_id.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(AuthErrorResponse {
                error: "Bad Request".to_string(),
                message: "Field 'user_id' cannot be empty".to_string(),
            }),
        ));
    }

    // Admin token validation
    if !state.admin_token.trim().is_empty() {
        let admin_header = headers
            .get("x-admin-token")
            .and_then(|h| h.to_str().ok())
            .unwrap_or("")
            .trim();

        if admin_header != state.admin_token.trim() {
            warn!(
                "[Auth] Unauthorized token issuance attempt for user '{}'",
                user_id
            );
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(AuthErrorResponse {
                    error: "Unauthorized".to_string(),
                    message: "Invalid or missing X-Admin-Token header".to_string(),
                }),
            ));
        }
    }

    let expiry_secs = payload
        .expires_in_seconds
        .unwrap_or(DEFAULT_JWT_EXPIRY_SECS)
        .max(1);

    let role = payload.role.as_deref().unwrap_or("institutional");
    let org_id = payload.org_id.as_deref();
    let sandbox = payload.sandbox;

    match generate_jwt_with_sandbox(
        user_id,
        expiry_secs,
        Some(role),
        org_id,
        sandbox,
        state.jwt_secret.as_bytes(),
    ) {
        Ok(token) => {
            debug!("[Auth] Issued JWT for user '{}' (role: '{}', org: '{:?}', sandbox: '{:?}', exp: {}s)", user_id, role, org_id, sandbox, expiry_secs);
            Ok(Json(IssueTokenResponse {
                token,
                token_type: "Bearer".to_string(),
                expires_in: expiry_secs,
                user_id: user_id.to_string(),
                role: role.to_string(),
                org_id: org_id.map(|s| s.to_string()),
                sandbox,
            }))
        }
        Err(err) => {
            warn!("[Auth] Failed to encode JWT: {}", err);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(AuthErrorResponse {
                    error: "Internal Server Error".to_string(),
                    message: "Failed to generate authentication token".to_string(),
                }),
            ))
        }
    }
}

/// Axum middleware function that validates the JWT or API Key on incoming requests.
///
/// Supports:
/// 1. `Authorization: Bearer <token>` HTTP header
/// 2. `?token=<token>` query parameter (for WebSocket connections)
/// 3. `X-API-Key: <api_key>` HTTP header
/// 4. `Authorization: ApiKey <api_key>` HTTP header
pub async fn auth_middleware(
    State(state): State<AppState>,
    mut req: Request,
    next: Next,
) -> Result<Response, Response> {
    let mut jwt_opt = None;
    let mut api_key_opt = None;

    // 1. Extract from Authorization header
    if let Some(auth_header) = req
        .headers()
        .get("authorization")
        .and_then(|h| h.to_str().ok())
    {
        let parts: Vec<&str> = auth_header.split_whitespace().collect();
        if parts.len() == 2 {
            if parts[0].eq_ignore_ascii_case("bearer") {
                jwt_opt = Some(parts[1].to_string());
            } else if parts[0].eq_ignore_ascii_case("apikey") {
                api_key_opt = Some(parts[1].to_string());
            }
        }
    }

    // 2. Extract from X-API-Key header
    if api_key_opt.is_none() {
        if let Some(key_header) = req.headers().get("x-api-key").and_then(|h| h.to_str().ok()) {
            if !key_header.trim().is_empty() {
                api_key_opt = Some(key_header.trim().to_string());
            }
        }
    }

    // 3. Extract from query parameter `?token=...` (WebSocket fallback)
    if jwt_opt.is_none() && api_key_opt.is_none() {
        if let Some(query) = req.uri().query() {
            for pair in query.split('&') {
                let mut kv = pair.splitn(2, '=');
                if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
                    if k == "token" && !v.trim().is_empty() {
                        jwt_opt = Some(v.trim().to_string());
                        break;
                    }
                }
            }
        }
    }

    // Path A: Validate JWT
    if let Some(token) = jwt_opt {
        match validate_jwt(&token, state.jwt_secret.as_bytes()) {
            Ok(claims) => {
                let is_sandbox = claims.sandbox.unwrap_or(false)
                    || state.sandbox_registry.is_active(&claims.sub);
                let sandbox_ctx = crate::sandbox::SandboxContext {
                    is_sandbox,
                    mock_version: crate::models::sandbox::DEFAULT_SANDBOX_MOCK_VERSION.to_string(),
                };
                req.extensions_mut().insert(sandbox_ctx);
                req.extensions_mut().insert(claims);
                let mut response = next.run(req).await;
                if is_sandbox {
                    if let Ok(val) = HeaderValue::from_str("true") {
                        response.headers_mut().insert("X-FinText-Sandbox", val);
                    }
                    if let Ok(val) =
                        HeaderValue::from_str(crate::models::sandbox::DEFAULT_SANDBOX_MOCK_VERSION)
                    {
                        response.headers_mut().insert("X-FinText-Mock-Version", val);
                    }
                }
                return Ok(response);
            }
            Err(err) => {
                let message = match err.kind() {
                    jsonwebtoken::errors::ErrorKind::ExpiredSignature => {
                        "JWT token has expired".to_string()
                    }
                    jsonwebtoken::errors::ErrorKind::InvalidSignature => {
                        "Invalid JWT signature".to_string()
                    }
                    _ => format!("JWT validation error: {}", err),
                };
                let err_body = Json(AuthErrorResponse {
                    error: "Unauthorized".to_string(),
                    message,
                });
                return Err((StatusCode::UNAUTHORIZED, err_body).into_response());
            }
        }
    }

    // Path B: Validate API Key
    if let Some(raw_key) = api_key_opt {
        let key_hash = crate::users::hash_api_key(&raw_key);
        let mut key_entry = state.api_key_registry.get_by_hash(&key_hash);

        // Fallback to PostgreSQL lookup if not in cache
        if key_entry.is_none() {
            if let Some(pool) = &state.db_pool {
                let row = sqlx::query_as::<_, (
                    uuid::Uuid,
                    uuid::Uuid,
                    String,
                    String,
                    String,
                    chrono::DateTime<Utc>,
                    Option<chrono::DateTime<Utc>>,
                    Option<chrono::DateTime<Utc>>,
                    Option<uuid::Uuid>,
                    String,
                )>(
                    "SELECT id, user_id, name, key_hash, prefix, created_at, revoked_at, expires_at, rotated_from, rotation_status FROM api_keys WHERE key_hash = $1 AND revoked_at IS NULL",
                )
                .bind(&key_hash)
                .fetch_optional(pool)
                .await;

                if let Ok(Some((
                    id,
                    user_id,
                    name,
                    key_hash,
                    prefix,
                    created_at,
                    revoked_at,
                    expires_at,
                    rotated_from,
                    rotation_status,
                ))) = row
                {
                    let k = crate::users::StoredApiKey {
                        id,
                        user_id,
                        name,
                        key_hash,
                        prefix,
                        created_at,
                        revoked_at,
                        expires_at,
                        rotated_from,
                        rotation_status,
                    };
                    state.api_key_registry.insert(k.clone());
                    key_entry = Some(k);
                }
            }
        }

        if let Some(key) = key_entry {
            let is_not_revoked = key.revoked_at.is_none();
            let is_not_expired = key.expires_at.map_or(true, |exp| exp > Utc::now());

            if is_not_revoked && is_not_expired {
                let user_id_str = key.user_id.to_string();
                let is_sandbox = state.sandbox_registry.is_active(&user_id_str);
                let claims = Claims {
                    sub: user_id_str,
                    exp: usize::MAX,
                    iat: Utc::now().timestamp() as usize,
                    role: "api_key".to_string(),
                    org_id: None,
                    sandbox: if is_sandbox { Some(true) } else { None },
                };
                let sandbox_ctx = crate::sandbox::SandboxContext {
                    is_sandbox,
                    mock_version: crate::models::sandbox::DEFAULT_SANDBOX_MOCK_VERSION.to_string(),
                };
                req.extensions_mut().insert(sandbox_ctx);
                req.extensions_mut().insert(claims);
                let mut response = next.run(req).await;
                if is_sandbox {
                    if let Ok(val) = HeaderValue::from_str("true") {
                        response.headers_mut().insert("X-FinText-Sandbox", val);
                    }
                    if let Ok(val) =
                        HeaderValue::from_str(crate::models::sandbox::DEFAULT_SANDBOX_MOCK_VERSION)
                    {
                        response.headers_mut().insert("X-FinText-Mock-Version", val);
                    }
                }
                return Ok(response);
            }
        }

        let err_body = Json(AuthErrorResponse {
            error: "Unauthorized".to_string(),
            message: "Invalid or revoked API key".to_string(),
        });
        return Err((StatusCode::UNAUTHORIZED, err_body).into_response());
    }

    let err_body = Json(AuthErrorResponse {
        error: "Unauthorized".to_string(),
        message: "Missing or invalid Bearer token or API key. Please supply Authorization: Bearer <token>, X-API-Key: <key>, or ?token=<token>".to_string(),
    });
    Err((StatusCode::UNAUTHORIZED, err_body).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SECRET: &[u8] = b"super-secret-institutional-test-key-2026";

    #[test]
    fn test_jwt_encode_and_decode_roundtrip() {
        let token = generate_jwt("trader_007", 3600, Some("quant"), TEST_SECRET)
            .expect("Should generate valid JWT");

        let claims = validate_jwt(&token, TEST_SECRET).expect("Should validate valid JWT");
        assert_eq!(claims.sub, "trader_007");
        assert_eq!(claims.role, "quant");
        assert!(claims.exp > claims.iat);
    }

    #[test]
    fn test_jwt_invalid_signature_rejected() {
        let token =
            generate_jwt("trader_007", 3600, None, TEST_SECRET).expect("Should generate valid JWT");

        let wrong_secret = b"wrong-secret-key-that-does-not-match";
        let err = validate_jwt(&token, wrong_secret).expect_err("Should reject invalid signature");
        assert!(matches!(
            err.kind(),
            jsonwebtoken::errors::ErrorKind::InvalidSignature
        ));
    }

    #[test]
    fn test_jwt_tampered_token_rejected() {
        let mut token =
            generate_jwt("trader_007", 3600, None, TEST_SECRET).expect("Should generate valid JWT");

        // Tamper with the token string
        token.push_str("tampered");
        let err = validate_jwt(&token, TEST_SECRET).expect_err("Should reject tampered token");
        assert!(err.to_string().len() > 0);
    }

    #[test]
    fn test_jwt_expired_token_rejected() {
        // Create token that already expired 10 seconds ago
        let now = Utc::now().timestamp() as usize;
        let claims = Claims {
            sub: "expired_user".to_string(),
            exp: now - 10,
            iat: now - 3600,
            role: "institutional".to_string(),
            org_id: None,
            sandbox: None,
        };

        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(TEST_SECRET),
        )
        .expect("Should encode token");

        let err = validate_jwt(&token, TEST_SECRET).expect_err("Should reject expired token");
        assert!(matches!(
            err.kind(),
            jsonwebtoken::errors::ErrorKind::ExpiredSignature
        ));
    }
}
