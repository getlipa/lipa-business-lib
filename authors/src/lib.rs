pub mod errors;
mod graphql;
pub mod provider;
pub mod secrets;
mod signing;

pub use provider::AuthLevel;

use crate::errors::{AuthResult, AuthRuntimeErrorCode};
use crate::secrets::KeyPair;
use base64::{engine::general_purpose, Engine as _};
use lipa_errors::{MapToLipaError, OptionToError};
use provider::AuthProvider;
use serde_json::Value;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

const TOKEN_TO_BE_YET_VALID: Duration = Duration::from_secs(10);

#[derive(Clone)]
struct Token {
    raw: String,
    expires_at: SystemTime,
}

pub struct Auth {
    provider: Mutex<AuthProvider>,
    token: Mutex<Token>,
}

impl Auth {
    pub fn new(
        backend_url: String,
        auth_level: AuthLevel,
        wallet_keypair: KeyPair,
        auth_keypair: KeyPair,
    ) -> AuthResult<Self> {
        let mut provider =
            AuthProvider::new(backend_url, auth_level, wallet_keypair, auth_keypair)?;
        let token = parse_token(provider.query_token()?)?;
        Ok(Auth {
            provider: Mutex::new(provider),
            token: Mutex::new(token),
        })
    }

    pub fn query_token(&self) -> AuthResult<String> {
        if let Some(token) = self.get_token_if_valid()? {
            return Ok(token);
        }

        let mut provider = self.provider.lock().unwrap();
        // Anyone else refreshed the token by chance?...
        if let Some(token) = self.get_token_if_valid()? {
            return Ok(token);
        }

        let token = parse_token(provider.query_token()?)?;
        *self.token.lock().unwrap() = token;
        self.get_token_if_valid()?
            .ok_or_permanent_failure("Newly refreshed token is not valid long enough")
    }

    // Not exposed in UDL, used in tests.
    pub fn refresh_token(&self) -> AuthResult<String> {
        let mut provider = self.provider.lock().unwrap();
        let token = parse_token(provider.query_token()?)?;
        *self.token.lock().unwrap() = token;
        self.get_token_if_valid()?
            .ok_or_permanent_failure("Newly refreshed token is not valid long enough")
    }

    fn get_token_if_valid(&self) -> AuthResult<Option<String>> {
        let now = SystemTime::now();
        let token = self.token.lock().unwrap();
        // TODO: Substruct 10% of token validity.
        if token.expires_at > now + TOKEN_TO_BE_YET_VALID {
            Ok(Some(token.raw.clone()))
        } else {
            Ok(None)
        }
    }
}

fn parse_token(raw_token: String) -> AuthResult<Token> {
    let splitted_jwt_strings: Vec<_> = raw_token.split('.').collect();

    let jwt_body = splitted_jwt_strings.get(1).ok_or_runtime_error(
        AuthRuntimeErrorCode::GenericError,
        "Failed to get JWT body: JWT String isn't split with '.' characters",
    )?;

    let decoded_jwt_body = general_purpose::STANDARD_NO_PAD
        .decode(jwt_body)
        .map_to_runtime_error(AuthRuntimeErrorCode::GenericError, "Failed to decode JWT")?;
    let converted_jwt_body = String::from_utf8(decoded_jwt_body).map_to_runtime_error(
        AuthRuntimeErrorCode::GenericError,
        "Failed to decode serialized JWT into json",
    )?;

    let parsed_jwt_body = serde_json::from_str::<Value>(&converted_jwt_body).map_to_runtime_error(
        AuthRuntimeErrorCode::GenericError,
        "Failed to get parse JWT json",
    )?;

    let expires_at = get_expiry(&parsed_jwt_body)?;

    /*println!(
        "The parsed token will expiry in {} secs",
        expires_at
            .duration_since(SystemTime::now())
            .unwrap()
            .as_secs()
    );*/
    Ok(Token {
        raw: raw_token,
        expires_at,
    })
}

fn get_expiry(jwt_body: &Value) -> AuthResult<SystemTime> {
    let expiry = jwt_body
        .as_object()
        .ok_or_runtime_error(
            AuthRuntimeErrorCode::GenericError,
            "Failed to get JWT body json object",
        )?
        .get("exp")
        .ok_or_runtime_error(
            AuthRuntimeErrorCode::GenericError,
            "JWT doesn't have an expiry field",
        )?
        .as_u64()
        .ok_or_runtime_error(
            AuthRuntimeErrorCode::GenericError,
            "Failed to parse JWT expiry into unsigned integer",
        )?;

    Ok(SystemTime::UNIX_EPOCH + Duration::from_secs(expiry))
}
