use defluencer::{errors::Error, Defluencer};

use linked_data::types::PeerId;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct LiveCLI {
    /// Channel local key name.
    #[structopt(short, long)]
    key_name: String,

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
    let defluencer = Defluencer::default();

    let LiveCLI {
        key_name,
        peer_id,
        video_topic,
        chat_topic,
        archiving,
    } = cli;

    if let Some(channel) = defluencer.get_local_channel(key_name).await? {
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
    }

    Ok(())
}
