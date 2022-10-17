#[derive(Debug, thiserror::Error)]
pub enum MnemonicGenerationError {
    #[error("Failed to generate entropy: {message}")]
    EntropyGeneration { message: String },

    #[error("Failed to generate mnemonic from entropy: {message}")]
    MnemonicFromEntropy { message: String },
}

#[derive(Debug, thiserror::Error)]
pub enum KeyDerivationError {
    #[error("Failed to parse provided mnemonic: {message}")]
    MnemonicParsing { message: String },

    #[error("Failed to turn Mnemonic into ExtendedKey: {message}")]
    ExtendedKeyFromMnemonic { message: String },

    #[error("Failed to turn ExtendedPrivKey into ExtendedKey: {message}")]
    ExtendedKeyFromXPriv { message: String },

    #[error("Failed to turn ExtendedKey into ExtendedPrivKey")]
    XPrivFromExtendedKey,

    #[error("Failed to parse derivation path: {message}")]
    DerivationPathParse { message: String },

    #[error("Failed to derive the provided path: {message}")]
    Derivation { message: String },
}

#[derive(Debug, thiserror::Error)]
pub enum SigningError {
    #[error("Failed to hash the message: {message}")]
    MessageHashing { message: String },

    #[error("Failed to parse the secret key: {message}")]
    SecretKeyParse { message: String },
}
