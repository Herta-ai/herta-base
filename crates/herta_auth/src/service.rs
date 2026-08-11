use std::{
    collections::{HashMap, VecDeque},
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
    time::{SystemTime, UNIX_EPOCH},
};

use argon2::{
    Argon2, PasswordHash, PasswordHasher, PasswordVerifier,
    password_hash::{SaltString, rand_core::OsRng},
};
use email_address::EmailAddress;
use herta_core::{AuthConfig, HbConfig, HbError, HbResult};
use herta_db::{CollectionType, DbClient, FieldType, SchemaManager, validation::validate_record};
use jsonwebtoken::{
    Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode, errors::ErrorKind,
};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TokenClaims {
    pub sub: String,
    pub collection: String,
    pub role: String,
    pub email: String,
    pub admin: bool,
    pub token_key: String,
    pub typ: String,
    pub iat: u64,
    pub exp: u64,
    pub jti: String,
    pub family: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileTokenClaims {
    pub sub: String,
    pub account_collection: String,
    pub email: String,
    pub admin: bool,
    pub token_key: String,
    pub typ: String,
    pub iat: u64,
    pub exp: u64,
    pub jti: String,
    pub collection: String,
    pub record_id: String,
    pub field: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FileToken {
    pub token: String,
    pub expires_in: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTokenScope {
    pub collection: String,
    pub record_id: String,
    pub field: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum AuthIdentity {
    #[default]
    Anonymous,
    User {
        id: String,
        collection: String,
        email: String,
        role: String,
        token_key: String,
    },
    Admin {
        id: String,
        email: String,
        role: String,
        token_key: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Authentication {
    pub identity: AuthIdentity,
    pub expires_at: u64,
}

impl AuthIdentity {
    pub fn is_admin(&self) -> bool {
        matches!(self, Self::Admin { .. })
    }

    pub fn as_rule_value(&self) -> Value {
        match self {
            Self::Anonymous => Value::Null,
            Self::User {
                id,
                collection,
                email,
                role,
                ..
            } => json!({
                "id": id,
                "collection": collection,
                "email": email,
                "role": role,
                "admin": false
            }),
            Self::Admin {
                id, email, role, ..
            } => json!({
                "id": id,
                "collection": "_admins",
                "email": email,
                "role": role,
                "admin": true
            }),
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct Credentials {
    pub email: String,
    pub password: String,
    #[serde(flatten)]
    pub profile: Map<String, Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RefreshRequest {
    pub refresh_token: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthUser {
    pub id: String,
    pub collection: String,
    pub email: String,
    pub role: String,
    pub verified: bool,
    pub admin: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<Value>,
    #[serde(flatten)]
    pub profile: Map<String, Value>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthResponse {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: &'static str,
    pub expires_in: u64,
    pub user: AuthUser,
}

#[derive(Clone)]
pub struct AuthService {
    db: DbClient,
    config: Arc<AuthConfig>,
    file_token_ttl_seconds: u64,
    secret: Arc<Vec<u8>>,
    rate_limits: Arc<Mutex<HashMap<String, VecDeque<u64>>>>,
}

impl AuthService {
    pub async fn new(db: DbClient, config: &HbConfig) -> HbResult<Self> {
        let secret = load_secret(config).map_err(|error| HbError::Database(error.to_string()))?;
        let service = Self {
            db,
            config: Arc::new(config.auth.clone()),
            file_token_ttl_seconds: config.storage.file_token_ttl_seconds,
            secret: Arc::new(secret),
            rate_limits: Arc::new(Mutex::new(HashMap::new())),
        };
        service.bootstrap_admin().await?;
        Ok(service)
    }

    pub fn check_register_rate(&self, key: &str) -> HbResult<()> {
        self.check_rate("register", key, self.config.register_rate_limit_per_minute)
    }

    pub fn check_login_rate(&self, key: &str) -> HbResult<()> {
        self.check_rate("login", key, self.config.login_rate_limit_per_minute)
    }

    pub fn check_refresh_rate(&self, key: &str) -> HbResult<()> {
        self.check_rate("refresh", key, self.config.refresh_rate_limit_per_minute)
    }

    fn check_rate(&self, endpoint: &str, key: &str, limit: u32) -> HbResult<()> {
        let now = now();
        let mut limits = self.rate_limits.lock().map_err(|_| HbError::Internal)?;
        let attempts = limits.entry(format!("{endpoint}:{key}")).or_default();
        while attempts
            .front()
            .is_some_and(|timestamp| now.saturating_sub(*timestamp) >= 60)
        {
            attempts.pop_front();
        }
        if attempts.len() >= limit as usize {
            return Err(HbError::RateLimited);
        }
        attempts.push_back(now);
        Ok(())
    }

    pub async fn register(
        &self,
        collection: &str,
        credentials: Credentials,
    ) -> HbResult<AuthResponse> {
        let definition = SchemaManager::new(&self.db)
            .get_collection(collection)
            .await?;
        if definition.collection_type != CollectionType::Auth {
            return Err(HbError::validation(
                "authentication endpoint requires an auth collection",
            ));
        }
        validate_password(&credentials.password)?;
        let email = normalize_email(&credentials.email)?;
        let password_hash = hash_password(credentials.password).await?;
        let token_key = Uuid::now_v7().to_string();
        let id = Uuid::now_v7().to_string();
        let mut data = credentials.profile;
        for protected in AUTH_PROTECTED_FIELDS {
            data.remove(protected);
        }
        for field in definition
            .fields
            .iter()
            .filter(|field| field.field_type == FieldType::File)
        {
            if let Some(value) = data.get(&field.name) {
                if value.is_null() || value.as_array().is_some_and(Vec::is_empty) {
                    data.remove(&field.name);
                } else {
                    return Err(HbError::UnsupportedMediaType(format!(
                        "file field '{}' must be uploaded as multipart",
                        field.name
                    )));
                }
            }
        }
        let mut profile = Value::Object(data.clone());
        validate_record(&definition, &mut profile, true)?;
        data.insert("email".into(), Value::String(email.clone()));
        data.insert("password_hash".into(), Value::String(password_hash));
        data.insert("token_key".into(), Value::String(token_key.clone()));
        data.insert("verified".into(), Value::Bool(false));
        data.insert("role".into(), Value::String("user".into()));
        data.insert("failed_attempts".into(), json!(0));
        data.insert("locked_until".into(), Value::Null);

        let table = quote_table(collection)?;
        let sql = format!("CREATE ONLY type::record('{table}', $id) CONTENT $data RETURN AFTER");
        let mut response = self
            .db
            .inner()
            .query(sql)
            .bind(("id", id))
            .bind(("data", Value::Object(data)))
            .await
            .map_err(database_error)?
            .check()
            .map_err(database_error)?;
        let created: Option<Value> = response.take(0).map_err(database_error)?;
        let user = user_from_value(collection, false, created.ok_or(HbError::Internal)?)?;
        self.issue_pair(&user, &token_key, None).await
    }

    pub async fn login(
        &self,
        collection: &str,
        credentials: Credentials,
        admin: bool,
    ) -> HbResult<AuthResponse> {
        if !admin {
            let definition = SchemaManager::new(&self.db)
                .get_collection(collection)
                .await?;
            if definition.collection_type != CollectionType::Auth {
                return Err(HbError::validation(
                    "authentication endpoint requires an auth collection",
                ));
            }
        }
        let email = normalize_email(&credentials.email)?;
        let table = if admin {
            "_admins"
        } else {
            quote_table(collection)?
        };
        let mut response = self
            .db
            .inner()
            .query(format!(
                "SELECT * FROM `{table}` WHERE email = $email LIMIT 1"
            ))
            .bind(("email", email))
            .await
            .map_err(database_error)?
            .check()
            .map_err(database_error)?;
        let mut accounts: Vec<Value> = response.take(0).map_err(database_error)?;
        let Some(account) = accounts.pop() else {
            delay_failed_login(0).await;
            return Err(HbError::AuthRequired);
        };
        let locked_until = account
            .get("locked_until")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        if locked_until > now() {
            return Err(HbError::AccountLocked);
        }
        let hash = account
            .get("password_hash")
            .and_then(Value::as_str)
            .ok_or(HbError::Internal)?
            .to_owned();
        if !verify_password(credentials.password, hash).await? {
            let attempts = account
                .get("failed_attempts")
                .and_then(Value::as_u64)
                .unwrap_or(0)
                .saturating_add(1);
            let lock = (attempts >= u64::from(self.config.lockout_threshold))
                .then(|| now() + self.config.lockout_seconds);
            self.update_login_state(table, record_id(&account)?, attempts, lock)
                .await?;
            delay_failed_login(attempts).await;
            return if lock.is_some() {
                Err(HbError::AccountLocked)
            } else {
                Err(HbError::AuthRequired)
            };
        }
        self.update_login_state(table, record_id(&account)?, 0, None)
            .await?;
        let token_key = account
            .get("token_key")
            .and_then(Value::as_str)
            .ok_or(HbError::Internal)?
            .to_owned();
        let user = user_from_value(collection, admin, account)?;
        self.issue_pair(&user, &token_key, None).await
    }

    pub async fn refresh(
        &self,
        refresh_token: &str,
        expected_admin: bool,
        expected_collection: Option<&str>,
    ) -> HbResult<AuthResponse> {
        let claims = self.decode_token(refresh_token, "refresh")?;
        if claims.admin != expected_admin
            || expected_collection.is_some_and(|collection| claims.collection != collection)
        {
            return Err(HbError::AuthRequired);
        }
        let hash = token_hash(refresh_token);
        let mut response = self
            .db
            .inner()
            .query(
                "UPDATE `_auth_refresh_tokens` SET used_at = time::now() \
                 WHERE jti = $jti AND token_hash = $hash AND expires_at > $now \
                   AND (used_at IS NONE OR used_at IS NULL) \
                   AND (revoked_at IS NONE OR revoked_at IS NULL) RETURN AFTER",
            )
            .bind(("jti", claims.jti.clone()))
            .bind(("hash", hash))
            .bind(("now", now()))
            .await
            .map_err(database_error)?
            .check()
            .map_err(database_error)?;
        let updated: Vec<Value> = response.take(0).map_err(database_error)?;
        if updated.iter().all(Value::is_null) {
            self.revoke_family(&claims).await?;
            return Err(HbError::AuthRequired);
        }
        let account = self.load_account(&claims).await?;
        let current_key = account
            .get("token_key")
            .and_then(Value::as_str)
            .ok_or(HbError::AuthRequired)?
            .to_owned();
        if current_key != claims.token_key {
            return Err(HbError::AuthRequired);
        }
        let user = user_from_value(&claims.collection, claims.admin, account)?;
        self.issue_pair(&user, &current_key, claims.family).await
    }

    pub async fn authenticate(&self, token: &str) -> HbResult<AuthIdentity> {
        Ok(self.authenticate_with_expiry(token).await?.identity)
    }

    pub async fn authenticate_with_expiry(&self, token: &str) -> HbResult<Authentication> {
        let claims = self.decode_token(token, "access")?;
        let account = self.load_account(&claims).await?;
        if account.get("token_key").and_then(Value::as_str) != Some(claims.token_key.as_str()) {
            return Err(HbError::AuthRequired);
        }
        let id = full_record_id(&account)?;
        let expires_at = claims.exp;
        let identity = if claims.admin {
            AuthIdentity::Admin {
                id,
                email: claims.email,
                role: claims.role,
                token_key: claims.token_key,
            }
        } else {
            AuthIdentity::User {
                id,
                collection: claims.collection,
                email: claims.email,
                role: claims.role,
                token_key: claims.token_key,
            }
        };
        Ok(Authentication {
            identity,
            expires_at,
        })
    }

    pub fn issue_file_token(
        &self,
        identity: &AuthIdentity,
        collection: &str,
        record_id: &str,
        field: &str,
    ) -> HbResult<FileToken> {
        let (sub, account_collection, email, admin, token_key) = match identity {
            AuthIdentity::User {
                id,
                collection,
                email,
                token_key,
                ..
            } => (
                id.as_str(),
                collection.as_str(),
                email.as_str(),
                false,
                token_key.as_str(),
            ),
            AuthIdentity::Admin {
                id,
                email,
                token_key,
                ..
            } => (
                id.as_str(),
                "_admins",
                email.as_str(),
                true,
                token_key.as_str(),
            ),
            AuthIdentity::Anonymous => return Err(HbError::AuthRequired),
        };
        let issued = now();
        let claims = FileTokenClaims {
            sub: sub.into(),
            account_collection: account_collection.into(),
            email: email.into(),
            admin,
            token_key: token_key.into(),
            typ: "file".into(),
            iat: issued,
            exp: issued + self.file_token_ttl_seconds,
            jti: Uuid::now_v7().to_string(),
            collection: collection.into(),
            record_id: record_id.into(),
            field: field.into(),
        };
        Ok(FileToken {
            token: encode(
                &Header::new(Algorithm::HS256),
                &claims,
                &EncodingKey::from_secret(&self.secret),
            )
            .map_err(|_| HbError::Internal)?,
            expires_in: self.file_token_ttl_seconds,
        })
    }

    pub async fn verify_file_token(&self, token: &str) -> HbResult<FileTokenScope> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_required_spec_claims(&["exp", "iat", "sub"]);
        let claims =
            decode::<FileTokenClaims>(token, &DecodingKey::from_secret(&self.secret), &validation)
                .map_err(|error| match error.kind() {
                    ErrorKind::ExpiredSignature => HbError::TokenExpired,
                    _ => HbError::AuthRequired,
                })?
                .claims;
        if claims.typ != "file" {
            return Err(HbError::AuthRequired);
        }
        let account_claims = TokenClaims {
            sub: claims.sub,
            collection: claims.account_collection,
            role: String::new(),
            email: claims.email,
            admin: claims.admin,
            token_key: claims.token_key.clone(),
            typ: "access".into(),
            iat: claims.iat,
            exp: claims.exp,
            jti: claims.jti,
            family: None,
        };
        let account = self.load_account(&account_claims).await?;
        if account.get("token_key").and_then(Value::as_str) != Some(claims.token_key.as_str()) {
            return Err(HbError::AuthRequired);
        }
        Ok(FileTokenScope {
            collection: claims.collection,
            record_id: claims.record_id,
            field: claims.field,
        })
    }

    pub async fn me(&self, identity: &AuthIdentity) -> HbResult<AuthUser> {
        let (collection, id, email, admin) = match identity {
            AuthIdentity::User {
                id,
                collection,
                email,
                ..
            } => (collection.as_str(), id.as_str(), email.as_str(), false),
            AuthIdentity::Admin { id, email, .. } => ("_admins", id.as_str(), email.as_str(), true),
            AuthIdentity::Anonymous => return Err(HbError::AuthRequired),
        };
        let claims = TokenClaims {
            sub: id.into(),
            collection: collection.into(),
            role: String::new(),
            email: email.into(),
            admin,
            token_key: String::new(),
            typ: "access".into(),
            iat: 0,
            exp: 0,
            jti: String::new(),
            family: None,
        };
        user_from_value(collection, admin, self.load_account(&claims).await?)
    }

    async fn issue_pair(
        &self,
        user: &AuthUser,
        token_key: &str,
        family: Option<String>,
    ) -> HbResult<AuthResponse> {
        let issued = now();
        let family = family.unwrap_or_else(|| Uuid::now_v7().to_string());
        let access_jti = Uuid::now_v7().to_string();
        let refresh_jti = Uuid::now_v7().to_string();
        let base = TokenClaims {
            sub: user.id.clone(),
            collection: user.collection.clone(),
            role: user.role.clone(),
            email: user.email.clone(),
            admin: user.admin,
            token_key: token_key.into(),
            typ: "access".into(),
            iat: issued,
            exp: issued + self.config.access_token_ttl_seconds,
            jti: access_jti,
            family: None,
        };
        let access_token = self.encode_token(&base)?;
        let mut refresh_claims = base;
        refresh_claims.typ = "refresh".into();
        refresh_claims.exp = issued + self.config.refresh_token_ttl_seconds;
        refresh_claims.jti = refresh_jti.clone();
        refresh_claims.family = Some(family.clone());
        let refresh_token = self.encode_token(&refresh_claims)?;
        let refresh_data = json!({
            "jti": refresh_jti,
            "family": family,
            "subject": user.id,
            "collection": user.collection,
            "admin": user.admin,
            "token_hash": token_hash(&refresh_token),
            "expires_at": refresh_claims.exp,
            "used_at": Value::Null,
            "revoked_at": Value::Null,
        });
        let response = self
            .db
            .inner()
            .query("CREATE `_auth_refresh_tokens` CONTENT $hb_refresh")
            .bind(("hb_refresh", refresh_data))
            .await
            .map_err(database_error)?;
        response.check().map_err(database_error)?;
        Ok(AuthResponse {
            access_token,
            refresh_token,
            token_type: "Bearer",
            expires_in: self.config.access_token_ttl_seconds,
            user: user.clone(),
        })
    }

    fn encode_token(&self, claims: &TokenClaims) -> HbResult<String> {
        encode(
            &Header::new(Algorithm::HS256),
            claims,
            &EncodingKey::from_secret(&self.secret),
        )
        .map_err(|_| HbError::Internal)
    }

    fn decode_token(&self, token: &str, expected_type: &str) -> HbResult<TokenClaims> {
        let mut validation = Validation::new(Algorithm::HS256);
        validation.set_required_spec_claims(&["exp", "iat", "sub"]);
        let decoded =
            decode::<TokenClaims>(token, &DecodingKey::from_secret(&self.secret), &validation)
                .map_err(|error| match error.kind() {
                    ErrorKind::ExpiredSignature => HbError::TokenExpired,
                    _ => HbError::AuthRequired,
                })?;
        if decoded.claims.typ != expected_type {
            return Err(HbError::AuthRequired);
        }
        Ok(decoded.claims)
    }

    async fn load_account(&self, claims: &TokenClaims) -> HbResult<Value> {
        let table = if claims.admin {
            "_admins"
        } else {
            quote_table(&claims.collection)?
        };
        let mut response = self
            .db
            .inner()
            .query(format!(
                "SELECT * FROM `{table}` WHERE email = $email LIMIT 1"
            ))
            .bind(("email", claims.email.clone()))
            .await
            .map_err(database_error)?
            .check()
            .map_err(database_error)?;
        let records: Vec<Value> = response.take(0).map_err(database_error)?;
        let account = records.into_iter().next().ok_or(HbError::AuthRequired)?;
        if full_record_id(&account)? != claims.sub {
            return Err(HbError::AuthRequired);
        }
        Ok(account)
    }

    async fn update_login_state(
        &self,
        table: &str,
        id: String,
        attempts: u64,
        locked_until: Option<u64>,
    ) -> HbResult<()> {
        let response = self
            .db
            .inner()
            .query(format!(
                "UPDATE ONLY type::record('{table}', $id) SET failed_attempts = $attempts, locked_until = $locked"
            ))
            .bind(("id", id))
            .bind(("attempts", attempts))
            .bind(("locked", locked_until))
            .await
            .map_err(database_error)?;
        response.check().map_err(database_error)?;
        Ok(())
    }

    async fn revoke_family(&self, claims: &TokenClaims) -> HbResult<()> {
        let family = claims.family.as_deref().ok_or(HbError::AuthRequired)?;
        let new_key = Uuid::now_v7().to_string();
        let table = if claims.admin {
            "_admins"
        } else {
            quote_table(&claims.collection)?
        };
        let response = self
            .db
            .inner()
            .query(format!(
                "BEGIN TRANSACTION; \
                 UPDATE `_auth_refresh_tokens` SET revoked_at = time::now() WHERE family = $family; \
                 UPDATE `{table}` SET token_key = $token_key WHERE email = $email; \
                 COMMIT TRANSACTION;"
            ))
            .bind(("family", family.to_owned()))
            .bind(("email", claims.email.clone()))
            .bind(("token_key", new_key))
            .await
            .map_err(database_error)?;
        response.check().map_err(database_error)?;
        Ok(())
    }

    async fn bootstrap_admin(&self) -> HbResult<()> {
        let email = self.config.bootstrap_admin_email.clone();
        let password = self.config.bootstrap_admin_password.clone();
        if email.is_some() != password.is_some() {
            return Err(HbError::validation(
                "HB_BOOTSTRAP_ADMIN_EMAIL and HB_BOOTSTRAP_ADMIN_PASSWORD must be set together",
            ));
        }
        let Some(email) = email else { return Ok(()) };
        let password = password.expect("bootstrap password was checked");
        validate_password(&password)?;
        let mut response = self
            .db
            .inner()
            .query("SELECT count() AS total FROM `_admins` GROUP ALL")
            .await
            .map_err(database_error)?
            .check()
            .map_err(database_error)?;
        let counts: Vec<Value> = response.take(0).map_err(database_error)?;
        if counts
            .first()
            .and_then(|v| v.get("total"))
            .and_then(Value::as_u64)
            .unwrap_or(0)
            > 0
        {
            return Ok(());
        }
        let data = json!({
            "email": normalize_email(&email)?,
            "password_hash": hash_password(password).await?,
            "token_key": Uuid::now_v7().to_string(),
            "verified": true,
            "role": "admin",
            "failed_attempts": 0,
            "locked_until": Value::Null,
        });
        let response = self
            .db
            .inner()
            .query("CREATE ONLY type::record('_admins', $id) CONTENT $data")
            .bind(("id", Uuid::now_v7().to_string()))
            .bind(("data", data))
            .await
            .map_err(database_error)?;
        response.check().map_err(database_error)?;
        Ok(())
    }
}

const AUTH_PROTECTED_FIELDS: [&str; 9] = [
    "id",
    "email",
    "password",
    "password_hash",
    "token_key",
    "verified",
    "role",
    "failed_attempts",
    "locked_until",
];

fn normalize_email(value: &str) -> HbResult<String> {
    let email = value.trim().to_ascii_lowercase();
    email
        .parse::<EmailAddress>()
        .map_err(|_| HbError::validation("email is invalid"))?;
    Ok(email)
}

fn validate_password(password: &str) -> HbResult<()> {
    if password.chars().count() < 12 {
        return Err(HbError::validation(
            "password must contain at least 12 characters",
        ));
    }
    Ok(())
}

async fn hash_password(password: String) -> HbResult<String> {
    tokio::task::spawn_blocking(move || {
        let salt = SaltString::generate(&mut OsRng);
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(|_| HbError::Internal)
    })
    .await
    .map_err(|_| HbError::Internal)?
}

async fn verify_password(password: String, hash: String) -> HbResult<bool> {
    tokio::task::spawn_blocking(move || {
        let parsed = PasswordHash::new(&hash).map_err(|_| HbError::Internal)?;
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok())
    })
    .await
    .map_err(|_| HbError::Internal)?
}

async fn delay_failed_login(attempts: u64) {
    let exponent = attempts.saturating_sub(1).min(4) as u32;
    let millis = 250_u64.saturating_mul(2_u64.pow(exponent)).min(4_000);
    tokio::time::sleep(std::time::Duration::from_millis(millis)).await;
}

fn load_secret(config: &HbConfig) -> anyhow::Result<Vec<u8>> {
    if let Some(secret) = &config.auth.jwt_secret {
        return Ok(secret.as_bytes().to_vec());
    }
    if config.database.engine == "memory" {
        let mut secret = vec![0_u8; 48];
        rand::thread_rng().fill_bytes(&mut secret);
        return Ok(secret);
    }
    let path = PathBuf::from(&config.paths.data_dir)
        .join("auth")
        .join("jwt-secret");
    if path.exists() {
        let secret = std::fs::read(&path)?;
        anyhow::ensure!(
            secret.len() >= 32,
            "persisted JWT secret is shorter than 32 bytes"
        );
        return Ok(secret);
    }
    persist_new_secret(&path)
}

fn persist_new_secret(path: &Path) -> anyhow::Result<Vec<u8>> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("invalid JWT secret path"))?;
    std::fs::create_dir_all(parent)?;
    let mut secret = vec![0_u8; 48];
    rand::thread_rng().fill_bytes(&mut secret);
    let temporary = parent.join(format!(".jwt-secret-{}.tmp", Uuid::now_v7()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)?;
    file.write_all(&secret)?;
    file.sync_all()?;
    match std::fs::rename(&temporary, path) {
        Ok(()) => Ok(secret),
        Err(_error) if path.exists() => {
            let _ = std::fs::remove_file(temporary);
            let existing = std::fs::read(path)?;
            anyhow::ensure!(
                existing.len() >= 32,
                "persisted JWT secret is shorter than 32 bytes"
            );
            Ok(existing)
        }
        Err(error) => Err(error.into()),
    }
}

fn token_hash(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn record_id(value: &Value) -> HbResult<String> {
    let raw = id_string(value.get("id").ok_or(HbError::Internal)?).ok_or(HbError::Internal)?;
    Ok(raw
        .split_once(':')
        .map_or(raw.as_str(), |(_, id)| id)
        .to_owned())
}

fn full_record_id(value: &Value) -> HbResult<String> {
    id_string(value.get("id").ok_or(HbError::Internal)?).ok_or(HbError::Internal)
}

fn user_from_value(collection: &str, admin: bool, mut value: Value) -> HbResult<AuthUser> {
    normalize_id(&mut value);
    let object = value.as_object_mut().ok_or(HbError::Internal)?;
    let id = object
        .remove("id")
        .and_then(|v| v.as_str().map(str::to_owned))
        .ok_or(HbError::Internal)?;
    let email = object
        .remove("email")
        .and_then(|v| v.as_str().map(str::to_owned))
        .ok_or(HbError::Internal)?;
    let role = object
        .remove("role")
        .and_then(|v| v.as_str().map(str::to_owned))
        .unwrap_or_else(|| if admin { "admin" } else { "user" }.into());
    let verified = object
        .remove("verified")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let created_at = object.remove("created_at");
    let updated_at = object.remove("updated_at");
    for field in [
        "password_hash",
        "token_key",
        "failed_attempts",
        "locked_until",
        "deleted_at",
    ] {
        object.remove(field);
    }
    Ok(AuthUser {
        id,
        collection: if admin {
            "_admins".into()
        } else {
            collection.into()
        },
        email,
        role,
        verified,
        admin,
        created_at,
        updated_at,
        profile: object.clone(),
    })
}

fn normalize_id(value: &mut Value) {
    if let Some(id) = value.get_mut("id")
        && let Some(raw) = id_string(id)
    {
        *id = Value::String(raw);
    }
}

fn id_string(value: &Value) -> Option<String> {
    if let Some(value) = value.as_str() {
        return Some(value.to_owned());
    }
    let table = value.get("table")?.as_str()?;
    let key = value
        .get("key")?
        .as_str()
        .map(str::to_owned)
        .or_else(|| value.get("key")?.as_u64().map(|key| key.to_string()))?;
    Some(format!("{table}:{key}"))
}

fn quote_table(table: &str) -> HbResult<&str> {
    if table == "_users"
        || (!table.is_empty()
            && table
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_alphabetic())
            && table.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
    {
        Ok(table)
    } else {
        Err(HbError::validation("invalid auth collection"))
    }
}

fn database_error(error: impl std::fmt::Display) -> HbError {
    let message = error.to_string();
    if message.to_ascii_lowercase().contains("unique")
        || message.to_ascii_lowercase().contains("already exists")
        || message.to_ascii_lowercase().contains("already contains")
    {
        HbError::Conflict("email is already registered".into())
    } else {
        HbError::Database(message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn admin_service() -> (DbClient, AuthService, AuthIdentity) {
        let db = DbClient::memory().await.unwrap();
        let mut config = HbConfig::default();
        config.database.engine = "memory".into();
        config.auth.bootstrap_admin_email = Some("admin@example.com".into());
        config.auth.bootstrap_admin_password = Some("correct horse battery staple".into());
        let service = AuthService::new(db.clone(), &config).await.unwrap();
        let response = service
            .login(
                "_admins",
                Credentials {
                    email: "admin@example.com".into(),
                    password: "correct horse battery staple".into(),
                    profile: Map::new(),
                },
                true,
            )
            .await
            .unwrap();
        let identity = service.authenticate(&response.access_token).await.unwrap();
        (db, service, identity)
    }

    #[tokio::test]
    async fn file_tokens_expire_when_the_account_token_key_changes() {
        let (db, service, identity) = admin_service().await;
        let token = service
            .issue_file_token(&identity, "assets", "record", "avatar")
            .unwrap();
        assert_eq!(
            service.verify_file_token(&token.token).await.unwrap(),
            FileTokenScope {
                collection: "assets".into(),
                record_id: "record".into(),
                field: "avatar".into(),
            }
        );
        db.inner()
            .query("UPDATE `_admins` SET token_key = $token_key")
            .bind(("token_key", Uuid::now_v7().to_string()))
            .await
            .unwrap()
            .check()
            .unwrap();
        assert!(matches!(
            service.verify_file_token(&token.token).await,
            Err(HbError::AuthRequired)
        ));
    }

    #[tokio::test]
    async fn expired_file_tokens_report_token_expired() {
        let (_db, service, identity) = admin_service().await;
        let (sub, email, token_key) = match identity {
            AuthIdentity::Admin {
                id,
                email,
                token_key,
                ..
            } => (id, email, token_key),
            _ => unreachable!(),
        };
        let issued = now().saturating_sub(120);
        let claims = FileTokenClaims {
            sub,
            account_collection: "_admins".into(),
            email,
            admin: true,
            token_key,
            typ: "file".into(),
            iat: issued,
            exp: issued + 1,
            jti: Uuid::now_v7().to_string(),
            collection: "assets".into(),
            record_id: "record".into(),
            field: "avatar".into(),
        };
        let token = encode(
            &Header::new(Algorithm::HS256),
            &claims,
            &EncodingKey::from_secret(&service.secret),
        )
        .unwrap();
        assert!(matches!(
            service.verify_file_token(&token).await,
            Err(HbError::TokenExpired)
        ));
    }
}
