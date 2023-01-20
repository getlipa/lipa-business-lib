pub mod errors;
mod graphql;
mod jwt;
pub mod provider;
pub mod secrets;
mod signing;

pub use provider::AuthLevel;

use crate::errors::{AuthResult, AuthRuntimeErrorCode};
use crate::jwt::parse_token;
use crate::provider::AuthProvider;
use crate::secrets::KeyPair;

use lipa_errors::{MapToLipaError, OptionToError};
use std::cmp::{max, min};
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

#[derive(Clone)]
struct AdjustedToken {
    raw: String,
    expires_at: SystemTime,
}

pub struct Auth {
    provider: Mutex<AuthProvider>,
    token: Mutex<AdjustedToken>,
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
        let token = adjust_token(provider.query_token()?)?;
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

        let token = adjust_token(provider.query_token()?)?;
        *self.token.lock().unwrap() = token;
        self.get_token_if_valid()?
            .ok_or_permanent_failure("Newly refreshed token is not valid long enough")
    }

    // Not exposed in UDL, used in tests.
    pub fn refresh_token(&self) -> AuthResult<String> {
        let mut provider = self.provider.lock().unwrap();
        let token = adjust_token(provider.query_token()?)?;
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
}

fn adjust_token(raw_token: String) -> AuthResult<AdjustedToken> {
    let token = parse_token(raw_token).map_to_runtime_error(
        AuthRuntimeErrorCode::AuthServiceError,
        "Auth service returned invalid JWT",
    )?;

    let token_validity_period = token
        .expires_at
        .duration_since(token.received_at)
        .map_to_runtime_error(
            AuthRuntimeErrorCode::AuthServiceError,
            "Expiration date of JWT is in the past",
        )?;

    let leeway = compute_leeway(token_validity_period)?;
    let expires_at = token.expires_at - leeway;
    debug_assert!(token.received_at < expires_at);

    Ok(AdjustedToken {
        raw: token.raw,
        expires_at,
    })
}

fn compute_leeway(period: Duration) -> AuthResult<Duration> {
    let leeway_10_percents = period
        .checked_div(100 / 10)
        .ok_or_permanent_failure("Failed to divide duration")?;

    let leeway_50_percents = period
        .checked_div(100 / 50)
        .ok_or_permanent_failure("Failed to divide duration")?;

    // At least 10 seconds.
    let lower_bound = max(Duration::from_secs(10), leeway_10_percents);
    // At most 30 seconds.
    let upper_bound = min(Duration::from_secs(30), leeway_50_percents);
    // If 50% < 10 seconds, use 50% of the period.
    let leeway = min(lower_bound, upper_bound);

    Ok(leeway)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[rustfmt::skip]
    fn test_compute_leeway() -> AuthResult<()> {
        assert_eq!(compute_leeway(secs(    10))?, secs( 5));
        assert_eq!(compute_leeway(secs(    20))?, secs(10));
        assert_eq!(compute_leeway(secs(    30))?, secs(10));
        assert_eq!(compute_leeway(secs(    60))?, secs(10));
        assert_eq!(compute_leeway(secs(2 * 60))?, secs(12));
        assert_eq!(compute_leeway(secs(3 * 60))?, secs(18));
        assert_eq!(compute_leeway(secs(4 * 60))?, secs(24));
        assert_eq!(compute_leeway(secs(5 * 60))?, secs(30));
        assert_eq!(compute_leeway(secs(6 * 60))?, secs(30));
        Ok(())
    }

    fn secs(secs: u64) -> Duration {
        Duration::from_secs(secs)
    }
}
