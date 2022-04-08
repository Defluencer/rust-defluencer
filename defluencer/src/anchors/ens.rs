pub struct ENSAnchor {
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
    pub fn new(web3: Web3, domain: String) -> Self {
        Self { web3, domain }
    }
}
