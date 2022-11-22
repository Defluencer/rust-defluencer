use signature::{Signature, Signer};

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
    S: Signature,
{
    fn crypto_key(&self) -> CryptoKey;
}

pub trait AsyncRecordSigner<S>: AsyncSigner<S>
where
    S: Signature + Send + 'static,
{
    fn crypto_key(&self) -> CryptoKey;
}
