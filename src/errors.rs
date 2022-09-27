#[derive(Debug, thiserror::Error)]
pub enum MnemonicGenerationError {
    #[error("Failed to generate entropy: {message}")]
    EntropyGeneration { message: String },

    #[error("Failed to generate mnemonic from entropy: {message}")]
    MnemonicFromEntropy { message: String },
}
