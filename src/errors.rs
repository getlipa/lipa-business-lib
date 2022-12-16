//! LipaError enum with helper functions.
//!
//! # Examples
//!
//! ```ignore
//! fn foo(x: u32) -> LipaResult<String> {
//!     if x <= 10 {
//!         return Err(invalid_input("x must be greater than 10"));
//!     }
//!     foreign_function().map_to_runtime_error("Foreign code failed")?;
//!     internal_function().prefix_error("Internal function failed")?;
//!     another_internal_function().lift_invalid_input("Another failure")?;
//! }
//! ```

use std::fmt::{Display, Formatter};

#[derive(Debug, PartialEq, Eq)]
pub enum RuntimeErrorCode {
    RemoteServiceUnavailable,
    ElectrumServiceUnavailable,
    NotEnoughFunds,
    GenericError,
}

impl Display for RuntimeErrorCode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum LipaError {
    /// Invalid input.
    /// Consider fixing the input and retrying the request.
    #[error("InvalidInput: {message}")]
    InvalidInput { message: String },

    /// Recoverable problem (e.g. network issue, problem with en external service).
    /// Consider retrying the request.
    #[error("RuntimeError: {code} - {message}")]
    RuntimeError {
        code: RuntimeErrorCode,
        message: String,
    },

    /// Unrecoverable problem (e.g. internal invariant broken).
    /// Consider suggesting the user to report the issue to the developers.
    #[error("PermanentFailure: {message}")]
    PermanentFailure { message: String },
}

#[allow(dead_code)]
pub fn invalid_input<E: ToString>(e: E) -> LipaError {
    LipaError::InvalidInput {
        message: e.to_string(),
    }
}

#[allow(dead_code)]
pub fn runtime_error<E: ToString>(code: RuntimeErrorCode, e: E) -> LipaError {
    LipaError::RuntimeError {
        code,
        message: e.to_string(),
    }
}

pub fn permanent_failure<E: ToString>(e: E) -> LipaError {
    LipaError::PermanentFailure {
        message: e.to_string(),
    }
}

pub type LipaResult<T> = Result<T, LipaError>;

pub trait LipaResultTrait<T> {
    /// Lift `InvalidInput` error into `PermanentFailure`.
    ///
    /// Use the method when you want to propagate an error from an internal
    /// function to the caller.
    /// Reasoning is that if you got `InvalidInput` it means you failed to
    /// validate the input for the internal function yourself, so for you it
    /// becomes `PermanentFailure`.
    fn lift_invalid_input(self) -> LipaResult<T>;

    fn prefix_error<M: ToString + 'static>(self, message: M) -> LipaResult<T>;
}

impl<T> LipaResultTrait<T> for LipaResult<T> {
    fn lift_invalid_input(self) -> LipaResult<T> {
        self.map_err(|e| match e {
            LipaError::InvalidInput { message } => LipaError::PermanentFailure {
                message: format!("InvalidInput: {}", message),
            },
            another_error => another_error,
        })
    }

    fn prefix_error<M: ToString + 'static>(self, prefix: M) -> LipaResult<T> {
        self.map_err(|e| match e {
            LipaError::InvalidInput { message } => LipaError::InvalidInput {
                message: format!("{}: {}", prefix.to_string(), message),
            },
            LipaError::RuntimeError { code, message } => LipaError::RuntimeError {
                code,
                message: format!("{}: {}", prefix.to_string(), message),
            },
            LipaError::PermanentFailure { message } => LipaError::PermanentFailure {
                message: format!("{}: {}", prefix.to_string(), message),
            },
        })
    }
}

pub trait MapToLipaError<T, E: ToString> {
    fn map_to_invalid_input<M: ToString>(self, message: M) -> LipaResult<T>;
    fn map_to_runtime_error<M: ToString>(self, code: RuntimeErrorCode, message: M)
        -> LipaResult<T>;
    fn map_to_permanent_failure<M: ToString>(self, message: M) -> LipaResult<T>;
}

impl<T, E: ToString> MapToLipaError<T, E> for Result<T, E> {
    fn map_to_invalid_input<M: ToString>(self, message: M) -> LipaResult<T> {
        self.map_err(move |e| LipaError::InvalidInput {
            message: format!("{}: {}", message.to_string(), e.to_string()),
        })
    }

    fn map_to_runtime_error<M: ToString>(
        self,
        code: RuntimeErrorCode,
        message: M,
    ) -> LipaResult<T> {
        self.map_err(move |e| LipaError::RuntimeError {
            code,
            message: format!("{}: {}", message.to_string(), e.to_string()),
        })
    }

    fn map_to_permanent_failure<M: ToString>(self, message: M) -> LipaResult<T> {
        self.map_err(move |e| LipaError::PermanentFailure {
            message: format!("{}: {}", message.to_string(), e.to_string()),
        })
    }
}

pub trait MapToLipaErrorForUnitType<T> {
    fn map_to_invalid_input<M: ToString>(self, message: M) -> LipaResult<T>;
    fn map_to_runtime_error<M: ToString>(self, code: RuntimeErrorCode, message: M)
        -> LipaResult<T>;
    fn map_to_permanent_failure<M: ToString>(self, message: M) -> LipaResult<T>;
}

impl<T> MapToLipaErrorForUnitType<T> for Result<T, ()> {
    fn map_to_invalid_input<M: ToString>(self, message: M) -> LipaResult<T> {
        self.map_err(move |()| LipaError::InvalidInput {
            message: message.to_string(),
        })
    }

    fn map_to_runtime_error<M: ToString>(
        self,
        code: RuntimeErrorCode,
        message: M,
    ) -> LipaResult<T> {
        self.map_err(move |()| LipaError::RuntimeError {
            code,
            message: message.to_string(),
        })
    }

    fn map_to_permanent_failure<M: ToString>(self, message: M) -> LipaResult<T> {
        self.map_err(move |()| LipaError::PermanentFailure {
            message: message.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RuntimeErrorCode::RemoteServiceUnavailable;

    #[test]
    fn test_map_to_lipa_errors() {
        use std::io::{Error, ErrorKind, Result};

        let io_error: Result<()> = Err(Error::new(ErrorKind::Other, "File not found"));
        let lipa_error = io_error
            .map_to_runtime_error(RemoteServiceUnavailable, "No backup")
            .unwrap_err();
        assert_eq!(
            lipa_error.to_string(),
            "RuntimeError: RemoteServiceUnavailable - No backup: File not found"
        );

        let error: std::result::Result<(), ()> = Err(());
        let lipa_error = error
            .map_to_runtime_error(RemoteServiceUnavailable, "No backup")
            .unwrap_err();
        assert_eq!(
            lipa_error.to_string(),
            "RuntimeError: RemoteServiceUnavailable - No backup"
        );
    }

    #[test]
    fn test_lift_invalid_input() {
        let result: LipaResult<()> =
            Err(invalid_input("Number must be positive")).lift_invalid_input();
        assert_eq!(
            result.unwrap_err().to_string(),
            "PermanentFailure: InvalidInput: Number must be positive"
        );

        let result: LipaResult<()> =
            Err(runtime_error(RemoteServiceUnavailable, "Socket timeout")).lift_invalid_input();
        assert_eq!(
            result.unwrap_err().to_string(),
            "RuntimeError: RemoteServiceUnavailable - Socket timeout"
        );

        let result: LipaResult<()> =
            Err(permanent_failure("Devision by zero")).lift_invalid_input();
        assert_eq!(
            result.unwrap_err().to_string(),
            "PermanentFailure: Devision by zero"
        );
    }

    #[test]
    fn test_prefix_error() {
        let result: LipaResult<()> =
            Err(invalid_input("Number must be positive")).prefix_error("Invalid amount");
        assert_eq!(
            result.unwrap_err().to_string(),
            "InvalidInput: Invalid amount: Number must be positive"
        );
    }
}
