use crate::errors::Error;

use cid::Cid;

use ipfs_api::{responses::Codec, IpfsService};

use linked_data::{indexes::log::ChainLink, IPLDLink};

pub(crate) async fn log_index_add(
    ipfs: &IpfsService,
    index: Option<IPLDLink>,
    add_cid: Cid,
) -> Result<Cid, Error> {
    let mut chainlink = match index {
        Some(index) => ipfs.dag_get::<&str, ChainLink>(index.link, None).await?,
        None => ChainLink::default(),
    };

    chainlink.media = add_cid.into();
    chainlink.previous = index;

    let cid = ipfs.dag_put(&chainlink, Codec::default()).await?;

    Ok(cid)
}

pub(crate) async fn log_index_remove(
    ipfs: &IpfsService,
    index: IPLDLink,
    remove_cid: Cid,
) -> Result<Cid, Error> {
    let mut chainlinks = Vec::default();
    let mut previous: Option<IPLDLink> = Some(index);

    loop {
        let cid = match previous {
            Some(ipld) => ipld.link,
            None => break,
        };

        let chainlink = ipfs.dag_get::<&str, ChainLink>(cid, None).await?;

        if chainlink.media.link == remove_cid {
            previous = chainlink.previous;

            break;
        } else {
            previous = chainlink.previous;

            chainlinks.push(chainlink);
        }
    }

    for mut chainlink in chainlinks.into_iter().rev() {
        chainlink.previous = previous;

        let cid = ipfs.dag_put(&chainlink, Codec::default()).await?;

        previous = Some(cid.into());
    }

    Ok(previous.unwrap().link)
}
