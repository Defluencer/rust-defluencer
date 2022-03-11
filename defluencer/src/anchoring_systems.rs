use async_trait::async_trait;

use cid::Cid;
use ipfs_api::IpfsService;

use crate::errors::Error;

/// Anchoring Systems take beacon cids and "anchor" them.
///
/// The methods used varies but blockchain or cryptography are usually used.
#[async_trait(?Send)]
pub trait AnchoringSystem {
    async fn anchor(&self, beacon_cid: Cid) -> Result<(), Error>;

    async fn retreive(&self) -> Result<Cid, Error>;
}

#[derive(Clone)]
pub struct IPNSAnchor {
    ipfs: IpfsService,
    key: String,
}

#[async_trait(?Send)]
impl AnchoringSystem for IPNSAnchor {
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

impl IPNSAnchor {
    pub fn new(ipfs: IpfsService, key: String) -> Self {
        Self { ipfs, key }
    }
}

/* pub struct ENSAnchor {
    web3: Web3,
    domain: String,
}

#[async_trait]
impl AnchoringSystem for ENSAnchor {
    async fn anchor(&self, beacon_cid: Cid) -> Result<(), Error> {
       todo!()
    }

    async fn retreive(&self) -> Result<Cid, Error> {
        todo!()
    }
}

impl ENSAnchor {
    pub fn new(web3: Web3 domain: String) -> Self {
        Self { web3, domain }
    }
} */
