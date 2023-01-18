use lipa_errors::{LipaError, LipaResult};
use std::fmt::{Display, Formatter};

#[derive(Debug, PartialEq, Eq)]
pub enum AuthRuntimeErrorCode {
    GenericError,
}

impl Display for AuthRuntimeErrorCode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub type AuthError = LipaError<AuthRuntimeErrorCode>;

pub type AuthResult<T> = LipaResult<T, AuthRuntimeErrorCode>;
