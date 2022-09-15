use signature::{Signature, Signer};

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
pub trait RecordSigner<U>: Signer<U>
where
    U: Signature,
{
    fn crypto_key(&self) -> CryptoKey;
}
