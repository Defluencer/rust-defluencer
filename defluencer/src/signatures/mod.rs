#[cfg(not(target_arch = "wasm32"))]
pub mod bitcoin;
pub mod dag_jose;
pub mod ethereum;
pub mod signed_link;

#[cfg(not(target_arch = "wasm32"))]
pub mod ledger;

use crate::errors::Error;

use async_trait::async_trait;

use k256::ecdsa::{Signature, VerifyingKey};

use self::signed_link::HashAlgorithm;

#[async_trait(?Send)]
pub trait Signer {
    async fn sign(
        &self,
        singing_input: &[u8],
    ) -> Result<(VerifyingKey, Signature, HashAlgorithm), Error>;
}
