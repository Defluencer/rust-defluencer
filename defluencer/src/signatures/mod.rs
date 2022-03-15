pub mod dag_jose;

mod eddsa;
mod ethereum;

#[cfg(target_arch = "wasm32")]
pub use ethereum::ENSSignature;

#[cfg(not(target_arch = "wasm32"))]
pub use eddsa::EdDSASigner;

use crate::errors::Error;

use async_trait::async_trait;

use cid::Cid;

#[async_trait(?Send)]
pub trait Signer {
    async fn sign(&self, cid: Cid) -> Result<Cid, Error>;
}
