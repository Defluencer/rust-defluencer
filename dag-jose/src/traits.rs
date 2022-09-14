use signature::{Signature, Signer};

use crate::{AlgorithmType, JsonWebKey};

pub trait BlockSigner<U>: Signer<U>
where
    U: Signature,
{
    fn algorithm(&self) -> AlgorithmType;

    fn web_key(&self) -> JsonWebKey;
}
