use cid::Cid;
use ipfs_api::{errors::Error, IpfsService};

use linked_data::live::Live;

use structopt::StructOpt;

pub const LIVE_KEY: &str = "live";

#[derive(Debug, StructOpt)]
pub struct LiveCLI {
    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(Debug, StructOpt)]
enum Command {
    /// Choose pubsub topics used for the live stream.
    Topics(UpdateTopics),

    /// Choose the IPFS node that will be streaming.
    PeerID(UpdatePeerId),
}

pub async fn live_cli(cli: LiveCLI) {
    let res = match cli.cmd {
        Command::Topics(topics) => update_topics(topics).await,
        Command::PeerID(peer) => update_peer_id(peer).await,
    };

    if let Err(e) = res {
        eprintln!("❗ IPFS: {:#?}", e);
    }
}

#[derive(Debug, StructOpt)]
pub struct UpdateTopics {
    /// Pubsub topic for live chat.
    #[structopt(short, long)]
    chat: Option<String>,

    /// Pubsub topic for live video.
    #[structopt(short, long)]
    video: Option<String>,
}

async fn update_topics(command: UpdateTopics) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let UpdateTopics { chat, video } = command;

    let res = ipfs.ipns_get(LIVE_KEY).await?;
    let (old_live_cid, mut live): (Cid, Live) = res.unwrap();

    if let Some(chat_topic) = chat {
        live.chat_topic = chat_topic;
    }

    if let Some(video_topic) = video {
        live.video_topic = video_topic;
    }

    ipfs.ipns_put(LIVE_KEY, false, &live).await?;

    if let Err(e) = ipfs.pin_rm(&old_live_cid, false).await {
        eprintln!("❗ IPFS could not unpin {}. Error: {}", old_live_cid, e);
    }

    println!("✅ Display Name Updated");

    Ok(())
}

#[derive(Debug, StructOpt)]
pub struct UpdatePeerId {
    /// Streaming node peer ID.
    #[structopt(short, long)]
    peer_id: Cid,
}

async fn update_peer_id(command: UpdatePeerId) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let UpdatePeerId { peer_id } = command;

    let res = ipfs.ipns_get(LIVE_KEY).await?;
    let (old_live_cid, mut live): (Cid, Live) = res.unwrap();

    live.peer_id = peer_id;

    ipfs.ipns_put(LIVE_KEY, false, &live).await?;

    if let Err(e) = ipfs.pin_rm(&old_live_cid, false).await {
        eprintln!("❗ IPFS could not unpin {}. Error: {}", old_live_cid, e);
    }

    println!("✅ Avatar Updated");

    Ok(())
}
