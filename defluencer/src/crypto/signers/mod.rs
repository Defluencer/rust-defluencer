#[cfg(not(target_arch = "wasm32"))]
mod bitcoin;

#[cfg(not(target_arch = "wasm32"))]
pub use self::bitcoin::BitcoinSigner;

#[cfg(not(target_arch = "wasm32"))]
mod ethereum;

#[cfg(not(target_arch = "wasm32"))]
pub use ethereum::EthereumSigner;

#[cfg(target_arch = "wasm32")]
mod web3;

#[cfg(target_arch = "wasm32")]
pub use web3::Web3Signer;

#[cfg(target_arch = "wasm32")]
mod web_crypto;

#[cfg(target_arch = "wasm32")]
pub use web_crypto::WebSigner;

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
