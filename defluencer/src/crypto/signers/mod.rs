mod bitcoin;
mod ethereum;

pub use self::bitcoin::BitcoinSigner;
pub use ethereum::EthereumSigner;

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
