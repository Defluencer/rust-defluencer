use signature::{Signature, Signer};

use crate::CryptoKey;

pub trait RecordSigner<U>: Signer<U>
where
    U: Signature,
{
    fn crypto_key(&self) -> CryptoKey;
}
