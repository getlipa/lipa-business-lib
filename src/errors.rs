use perro::{PError, PResult};
use std::fmt::{Display, Formatter};

#[derive(Debug, PartialEq, Eq)]
pub enum LblRuntimeErrorCode {
    ElectrumServiceUnavailable,
    NotEnoughFunds,
    RemoteServiceUnavailable,
    SendToOurselves,
    GenericError,
}

impl Display for LblRuntimeErrorCode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

pub type LblError = PError<LblRuntimeErrorCode>;

pub(crate) type LblResult<T> = PResult<T, LblRuntimeErrorCode>;
