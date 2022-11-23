#[cfg(not(target_arch = "wasm32"))]
mod bitcoin;

#[cfg(not(target_arch = "wasm32"))]
pub use self::bitcoin::BitcoinSigner;

#[cfg(not(target_arch = "wasm32"))]
mod ethereum;

#[cfg(not(target_arch = "wasm32"))]
pub use ethereum::EthereumSigner;

#[cfg(target_arch = "wasm32")]
mod metamask;

#[cfg(target_arch = "wasm32")]
pub use metamask::MetamaskSigner;

use crate::errors::Error;

use async_trait::async_trait;

use k256::ecdsa::{Signature, VerifyingKey};

use super::signed_link::HashAlgorithm;

#[async_trait(?Send)]
pub trait Signer {
    async fn sign(
        &self,
        singing_input: &[u8],
    ) -> Result<(VerifyingKey, Signature, HashAlgorithm), Error>;
}
