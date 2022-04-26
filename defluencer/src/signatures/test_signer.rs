use async_trait::async_trait;

use cid::Cid;

use crate::errors::Error;

#[derive(Default)]
pub struct TestSigner {}

#[async_trait(?Send)]
impl super::Signer for TestSigner {
    async fn sign(&self, _cid: Cid) -> Result<Cid, Error> {
        unimplemented!()
    }
}
