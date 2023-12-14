use async_trait::async_trait;

use signature::{SignatureEncoding, Signer};

use async_signature::AsyncSigner;

use crate::CryptoKey;

/// Impl'd the trait is not enough to create valid records.
///
/// Different IPFS implementations don't support the same digital signatures schemes.
///
/// Always Ask yourself;
/// - What is being signed exactly?
/// - What hash algorithm is used?
/// - With which signature algorithm?
/// - In what format?
pub trait RecordSigner<S>: Signer<S>
where
    S: SignatureEncoding,
{
    fn crypto_key(&self) -> CryptoKey;
}

#[async_trait]
pub trait AsyncRecordSigner<S>: AsyncSigner<S>
where
    S: SignatureEncoding + Send + 'static,
{
    async fn crypto_key(&self) -> CryptoKey;
}
