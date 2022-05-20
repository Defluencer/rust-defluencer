#[cfg(not(target_arch = "wasm32"))]
pub mod bitcoin;

pub mod dag_jose;
pub mod ethereum;
pub mod test_signer;

#[cfg(not(target_arch = "wasm32"))]
pub mod ledger;

use crate::errors::Error;

use async_trait::async_trait;

use k256::{ecdsa::Signature, PublicKey};

#[async_trait(?Send)]
pub trait Signer {
    async fn sign(&self, singing_input: Vec<u8>) -> Result<(PublicKey, Signature), Error>;
}
