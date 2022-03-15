mod ipns;

pub use ipns::IPNSAnchor;

use async_trait::async_trait;

use cid::Cid;

use crate::errors::Error;

/// Anchoring Systems take beacon cids and "anchor" them.
///
/// The methods used varies but blockchain or cryptography are usually used.
#[async_trait(?Send)]
pub trait Anchor {
    async fn anchor(&self, beacon_cid: Cid) -> Result<(), Error>;

    async fn retreive(&self) -> Result<Cid, Error>;
}
