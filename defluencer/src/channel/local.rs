use async_trait::async_trait;

use cid::Cid;

use ipfs_api::IpfsService;

use crate::errors::Error;

use super::IpnsUpdater;

/// Local IPNS updater. Keys reside in the local IPFS node.
#[derive(Clone)]
pub struct LocalUpdater {
    ipfs: IpfsService,
    key: String,
}

impl LocalUpdater {
    pub fn new(ipfs: IpfsService, key: String) -> Self {
        Self { ipfs, key }
    }
}

#[async_trait(?Send)]
impl IpnsUpdater for LocalUpdater {
    async fn update(&self, cid: Cid) -> Result<(), Error> {
        self.ipfs.name_publish(cid, self.key.clone()).await?;

        Ok(())
    }
}
