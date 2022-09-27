#[derive(Debug, thiserror::Error)]
pub enum WalletGenerationError {
    #[error("Failed to generate wallet: {message}")]
    Other { message: String },
}
