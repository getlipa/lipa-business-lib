use std::fmt::{Display, Formatter};

#[derive(Debug, PartialEq, Eq)]
pub enum WalletRuntimeErrorCode {
    ElectrumServiceUnavailable,
    NotEnoughFunds,
    RemoteServiceUnavailable,
    SendToOurselves,
    GenericError,
}

impl Display for WalletRuntimeErrorCode {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

pub type Error = perro::Error<WalletRuntimeErrorCode>;

pub(crate) type Result<T> = std::result::Result<T, perro::Error<WalletRuntimeErrorCode>>;
