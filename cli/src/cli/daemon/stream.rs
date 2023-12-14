use std::net::SocketAddr;

use crate::{
    actors::{Archivist, Setter, Videograph},
    server::start_server,
};

use defluencer::errors::Error;

use linked_data::{
    channel::{live::LiveSettings, ChannelMetadata},
    types::IPNSAddress,
};

use tokio::{
    signal::ctrl_c,
    sync::{mpsc::unbounded_channel, watch},
};

use ipfs_api::{responses::Codec, IpfsService};

use clap::Parser;

#[derive(Debug, Parser)]
pub struct Stream {
    /// Socket Address used to ingress video.
    #[arg(long, default_value = "127.0.0.1:2526")]
    socket_addr: SocketAddr,

    /// Channel IPNS Address.
    #[arg(long)]
    ipns_addr: IPNSAddress,
}

pub async fn stream_cli(args: Stream) {
    let res = stream(args).await;

    if let Err(e) = res {
        eprintln!("❗ IPFS: {:#?}", e);
    }
}

async fn stream(args: Stream) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    println!("Initialization...");

    let peer_id = match ipfs.peer_id().await {
        Ok(peer_id) => peer_id,
        Err(_) => {
            eprintln!("❗ IPFS must be started beforehand.\nAborting...");
            return Ok(());
        }
    };

    let Stream {
        ipns_addr,
        socket_addr,
    } = args;

    let cid = ipfs.name_resolve(ipns_addr).await?;
    let metadata = ipfs
        .dag_get::<&str, ChannelMetadata>(cid, None, Codec::default())
        .await?;

    let settings = match metadata.live {
        Some(ipld) => {
            ipfs.dag_get::<&str, LiveSettings>(ipld.link, None, Codec::default())
                .await?
        }
        None => {
            eprintln!("❗ Channel live settings not found.\nAborting...");
            return Ok(());
        }
    };

    if settings.peer_id != peer_id {
        eprintln!("❗ This peer is not allowed to stream on this channel. Update your channel live settings!\nAborting...");
        return Ok(());
    }

    let mut handles = Vec::with_capacity(6);

    let shutdown = {
        let (tx, rx) = watch::channel::<()>(());

        let handle = tokio::spawn(async move {
            ctrl_c()
                .await
                .expect("Failed to install CTRL+C signal handler");

            if let Err(e) = tx.send(()) {
                eprintln!("{}", e);
            }
        });
        handles.push(handle);

        rx
    };

    let archive_tx = {
        if settings.archiving {
            let (archive_tx, archive_rx) = unbounded_channel();

            /* if let Some(chat_topic) = settings.chat_topic {
                let bans = match settings.bans {
                    Some(ipld) => ipfs.dag_get::<&str, Bans>(ipld.link, None).await?,
                    None => Default::default(),
                };

                let mods = match settings.mods {
                    Some(ipld) => ipfs.dag_get::<&str, Moderators>(ipld.link, None).await?,
                    None => Default::default(),
                };

                let chat = Chatter::new(
                    ipfs.clone(),
                    archive_tx.clone(),
                    shutdown.clone(),
                    chat_topic,
                    bans,
                    mods,
                );
                let handle = tokio::spawn(chat.start());
                handles.push(handle);
            } */

            let archivist = Archivist::new(ipfs.clone(), archive_rx);
            let handle = tokio::spawn(archivist.start());
            handles.push(handle);

            Some(archive_tx)
        } else {
            None
        }
    };

    let (video_tx, video_rx) = unbounded_channel();

    let video = Videograph::new(
        ipfs.clone(),
        video_rx,
        archive_tx.clone(),
        Some(settings.video_topic),
    );
    let handle = tokio::spawn(video.start());
    handles.push(handle);

    let (setup_tx, setup_rx) = unbounded_channel();

    let setup = Setter::new(ipfs.clone(), setup_rx, video_tx.clone());
    let handle = tokio::spawn(setup.start());
    handles.push(handle);

    let handle = tokio::spawn(start_server(
        socket_addr,
        video_tx,
        setup_tx,
        ipfs,
        shutdown,
    ));
    handles.push(handle);

    for handle in handles {
        if let Err(e) = handle.await {
            eprintln!("❗ Main: {}", e);
        }
    }

    Ok(())
}
