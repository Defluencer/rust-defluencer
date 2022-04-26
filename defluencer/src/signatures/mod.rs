pub mod dag_jose;

#[cfg(target_arch = "wasm32")]
mod ethereum;
#[cfg(target_arch = "wasm32")]
pub use ethereum::EthereumWebSigner;

#[cfg(not(target_arch = "wasm32"))]
mod eddsa;
#[cfg(not(target_arch = "wasm32"))]
pub use eddsa::EdDSASigner;

mod test_signer;
pub use test_signer::TestSigner;

use crate::errors::Error;

use async_trait::async_trait;

use cid::Cid;

/// Signer create Dag-Jose blocks.
#[async_trait(?Send)]
pub trait Signer {
    async fn sign(&self, cid: Cid) -> Result<Cid, Error>;
}
