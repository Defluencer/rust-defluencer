use crate::{
    actors::{Archivist, SetupAggregator, VideoAggregator},
    config::Configuration,
    server::start_server,
};

use tokio::sync::mpsc::unbounded_channel;

use ipfs_api::IpfsService;

use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct File {}

pub async fn file_cli(_file: File) {
    let ipfs = IpfsService::default();

    if let Err(e) = ipfs.peer_id().await {
        eprintln!("❗ IPFS must be started beforehand. {}", e);
        return;
    }

    println!("Initialization...");

    let config = match Configuration::from_file().await {
        Ok(conf) => conf,
        Err(e) => {
            eprintln!("❗ Configuration file not found. {}", e);

            println!("Default configuration will be used.");
            Configuration::default()
        }
    };

    let Configuration {
        input_socket_addr,
        mut archive,
        mut video,
        chat,
    } = config;

    let mut handles = Vec::with_capacity(4);

    let (archive_tx, archive_rx) = unbounded_channel();

    archive.archive_live_chat = false;

    let archivist = Archivist::new(ipfs.clone(), archive_rx);

    let archive_handle = tokio::spawn(archivist.start());

    handles.push(archive_handle);

    let (video_tx, video_rx) = unbounded_channel();

    video.pubsub_enable = false;

    let video = VideoAggregator::new(ipfs.clone(), video_rx, Some(archive_tx.clone()), video);

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
        Some(archive_tx),
        ipfs,
        chat.topic,
    ));

    handles.push(server_handle);

    for handle in handles {
        if let Err(e) = handle.await {
            eprintln!("❗ Main: {}", e);
        }
    }
}
