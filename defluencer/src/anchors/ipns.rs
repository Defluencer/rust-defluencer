use async_trait::async_trait;

use cid::Cid;
use ipfs_api::IpfsService;

use crate::errors::Error;

#[derive(Clone)]
pub struct IPNSAnchor {
    ipfs: IpfsService,
    key: String,
}

impl IPNSAnchor {
    pub fn new(ipfs: IpfsService, key: String) -> Self {
        Self { ipfs, key }
    }
}

#[async_trait(?Send)]
impl super::Anchor for IPNSAnchor {
    async fn anchor(&self, beacon_cid: Cid) -> Result<(), Error> {
        self.ipfs.name_publish(beacon_cid, self.key.clone()).await?;

        Ok(())
    }

    async fn retreive(&self) -> Result<Cid, Error> {
        let key_list = self.ipfs.key_list().await?;

        let cid = match key_list.get(&self.key) {
            Some(keypair) => *keypair,
            None => return Err(ipfs_api::errors::Error::Ipns.into()),
        };

        let cid = self.ipfs.name_resolve(cid).await?;

        Ok(cid)
    }
}
