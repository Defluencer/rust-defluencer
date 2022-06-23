use async_trait::async_trait;

use crate::errors::Error;

use k256::{ecdsa::Signature, PublicKey};

#[derive(Default, Clone)]
pub struct TestSigner {}

#[async_trait(?Send)]
impl super::Signer for TestSigner {
    async fn sign(&self, _signing_input: Vec<u8>) -> Result<(PublicKey, Signature), Error> {
        unimplemented!()
    }
}
