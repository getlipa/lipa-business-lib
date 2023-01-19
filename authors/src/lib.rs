pub mod errors;
mod graphql;
mod jwt;
pub mod provider;
pub mod secrets;
mod signing;

pub use provider::AuthLevel;

use crate::errors::{AuthResult, AuthRuntimeErrorCode};
use crate::jwt::{parse_token, Token};
use crate::provider::AuthProvider;
use crate::secrets::KeyPair;

use lipa_errors::{MapToLipaError, OptionToError};
use std::sync::Mutex;
use std::time::SystemTime;

const TOKEN_VALIDITY_LEEWAY_PERCENTS: u32 = 10;

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
        let token = Self::parse_token(provider.query_token()?)?;
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

        let token = Self::parse_token(provider.query_token()?)?;
        *self.token.lock().unwrap() = token;
        self.get_token_if_valid()?
            .ok_or_permanent_failure("Newly refreshed token is not valid long enough")
    }

    // Not exposed in UDL, used in tests.
    pub fn refresh_token(&self) -> AuthResult<String> {
        let mut provider = self.provider.lock().unwrap();
        let token = Self::parse_token(provider.query_token()?)?;
        *self.token.lock().unwrap() = token;
        self.get_token_if_valid()?
            .ok_or_permanent_failure("Newly refreshed token is not valid long enough")
    }

    fn get_token_if_valid(&self) -> AuthResult<Option<String>> {
        let now = SystemTime::now();
        let token = self.token.lock().unwrap();
        if now < token.expires_at {
            Ok(Some(token.raw.clone()))
        } else {
            Ok(None)
        }
    }

    fn parse_token(raw_token: String) -> AuthResult<Token> {
        let mut token = parse_token(raw_token).map_to_runtime_error(
            AuthRuntimeErrorCode::AuthServiceError,
            "Auth service returned invalid JWT",
        )?;
        let token_validity_period = token
            .expires_at
            .duration_since(token.received_at)
            .map_to_runtime_error(
                AuthRuntimeErrorCode::AuthServiceError,
                "expiration date of JWT is in the past",
            )?;
        let leeway = token_validity_period
            .checked_div(100 / TOKEN_VALIDITY_LEEWAY_PERCENTS)
            .ok_or_permanent_failure("Failed to divide duration")?;
        token.expires_at -= leeway;
        debug_assert!(token.received_at < token.expires_at);

        Ok(token)
    }
}
