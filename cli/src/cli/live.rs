use cid::Cid;

use defluencer::{channel::Channel, errors::Error, signatures::TestSigner};

use ipfs_api::IpfsService;
use linked_data::types::PeerId;

use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct LiveCLI {
    /// Channel IPNS Address.
    #[structopt(short, long)]
    address: Cid,

    /// Peer Id of the node live streaming.
    #[structopt(short, long)]
    peer_id: Option<String>,

    /// PubSub Topic for live video.
    #[structopt(short, long)]
    video_topic: Option<String>,

    /// PubSub Topic for live chat.
    #[structopt(short, long)]
    chat_topic: Option<String>,

    /// Should live chat be archived.
    #[structopt(short, long)]
    archiving: Option<bool>,
}

pub async fn live_cli(cli: LiveCLI) {
    let res = update(cli).await;

    if let Err(e) = res {
        eprintln!("❗ IPFS: {:#?}", e);
    }
}

async fn update(cli: LiveCLI) -> Result<(), Error> {
    let LiveCLI {
        address,
        peer_id,
        video_topic,
        chat_topic,
        archiving,
    } = cli;

    let ipfs = IpfsService::default();

    let signer = TestSigner::default(); //TODO

    let channel = Channel::new(ipfs, address.into(), signer);

    let peer_id = if let Some(peer) = peer_id {
        match PeerId::try_from(peer) {
            Ok(peer) => Some(peer.into()),
            Err(e) => {
                eprintln!("{}", e);

                None
            }
        }
    } else {
        None
    };

    let cid = channel
        .update_live_settings(peer_id, video_topic, chat_topic, archiving)
        .await?;

    println!("✅ Live Settings Updated\nCID: {}", cid);

    Ok(())
}
