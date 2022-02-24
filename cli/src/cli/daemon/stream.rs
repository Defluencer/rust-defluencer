use crate::{
    actors::{Archivist, ChatAggregator, SetupAggregator, VideoAggregator},
    config::Configuration,
    server::start_server,
};

use tokio::sync::mpsc::unbounded_channel;

use ipfs_api::IpfsService;

use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Stream {
    /// Disable chat archiving.
    #[structopt(long)]
    no_chat: bool,

    /// Disable all archiving.
    #[structopt(long)]
    no_archive: bool,
}

pub async fn stream_cli(stream: Stream) {
    let Stream {
        no_chat,
        no_archive,
    } = stream;

    let ipfs = IpfsService::default();

    if ipfs.peer_id().await.is_err() {
        eprintln!("❗ IPFS must be started beforehand. Aborting...");
        return;
    }

    println!("Initialization...");

    let config = match Configuration::from_file().await {
        Ok(conf) => conf,
        Err(e) => {
            eprintln!("❗ Configuration file not found. {}", e);
            return;
        }
    };

    let Configuration {
        input_socket_addr,
        mut archive,
        mut video,
        chat,
    } = config;

    let mut handles = Vec::with_capacity(4);

    let topic = chat.topic.clone();

    let archive_tx = {
        if !no_archive {
            let (archive_tx, archive_rx) = unbounded_channel();

            if !no_chat {
                let chat = match ChatAggregator::new(ipfs.clone(), archive_tx.clone(), chat).await {
                    Ok(chat) => chat,
                    Err(e) => {
                        eprintln!("❗ IPFS: {:#?}", e);
                        return;
                    }
                };

                let chat_handle = tokio::spawn(chat.start());

                handles.push(chat_handle);
            }

            archive.archive_live_chat = !no_chat;

            let archivist = Archivist::new(ipfs.clone(), archive_rx);

            let archive_handle = tokio::spawn(archivist.start());

            handles.push(archive_handle);

            Some(archive_tx)
        } else {
            None
        }
    };

    let (video_tx, video_rx) = unbounded_channel();

    video.pubsub_enable = true;

    let video = VideoAggregator::new(ipfs.clone(), video_rx, archive_tx.clone(), video);

    let video_handle = tokio::spawn(video.start());

    handles.push(video_handle);

    let (setup_tx, setup_rx) = unbounded_channel();

    let setup = SetupAggregator::new(ipfs.clone(), setup_rx, video_tx.clone());

    let setup_handle = tokio::spawn(setup.start());

    handles.push(setup_handle);

    let server_handle = tokio::spawn(start_server(
        input_socket_addr,
        video_tx,
        setup_tx,
        archive_tx,
        ipfs,
        topic,
    ));

    handles.push(server_handle);

    for handle in handles {
        if let Err(e) = handle.await {
            eprintln!("❗ Main: {}", e);
        }
    }
}
