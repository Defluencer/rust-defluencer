pub mod dag_jose;

#[cfg(target_arch = "wasm32")]
mod ethereum;
#[cfg(target_arch = "wasm32")]
pub use ethereum::EthereumWebSigner;

mod test_signer;
pub use test_signer::TestSigner;

use crate::errors::Error;

use async_trait::async_trait;

use k256::{ecdsa::Signature, PublicKey};

#[async_trait(?Send)]
pub trait Signer {
    async fn sign(&self, singing_input: Vec<u8>) -> Result<(PublicKey, Signature), Error>;
}
