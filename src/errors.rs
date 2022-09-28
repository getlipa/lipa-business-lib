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

    #[error("Failed to get a DescriptorKey from a ExtendedPrivKey: {message}")]
    DescriptorKeyFromXPriv { message: String },

    #[error("Failed to get a DescriptorPublicKey from a DescriptorSecretKey: {message}")]
    DescPubKeyFromDescSecretKey { message: String },

    #[error("Failed to get a DescriptorSecretKey from a DescriptorKey")]
    DescSecretKeyFromDescKey,
}

#[derive(Debug, thiserror::Error)]
pub enum WalletError {
    #[error("Failed to create a client to get blockchain data: {message}")]
    ChainBackendClient { message: String },

    #[error("Failed to create a bdk::Wallet instance: {message}")]
    BdkWallet { message: String },

    #[error("Failed to sync with the blockchain: {message}")]
    ChainSync { message: String },

    #[error("Failed to get balance from bdk::Wallet instance: {message}")]
    GetBalance { message: String },
}
