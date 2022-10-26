use std::net::SocketAddr;

use crate::{
    actors::{Archivist, Setter, Videograph},
    server::start_server,
};

use defluencer::errors::Error;

use tokio::{
    signal::ctrl_c,
    sync::{mpsc::unbounded_channel, watch},
};

use ipfs_api::IpfsService;

use clap::Parser;

#[derive(Debug, Parser)]
pub struct File {
    /// Socket Address used to ingress video.
    #[arg(long, default_value = "127.0.0.1:2526")]
    socket_addr: SocketAddr,
}

pub async fn file_cli(args: File) {
    let res = file(args).await;

    if let Err(e) = res {
        eprintln!("❗ IPFS: {:#?}", e);
    }
}

async fn file(args: File) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    println!("Initialization...");

    if let Err(_) = ipfs.peer_id().await {
        eprintln!("❗ IPFS must be started beforehand.\nAborting...");
        return Ok(());
    }

    let File { socket_addr } = args;

    let mut handles = Vec::with_capacity(5);

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

    let (archive_tx, archive_rx) = unbounded_channel();

    let archivist = Archivist::new(ipfs.clone(), archive_rx);
    let handle = tokio::spawn(archivist.start());
    handles.push(handle);

    let (video_tx, video_rx) = unbounded_channel();

    let video = Videograph::new(ipfs.clone(), video_rx, Some(archive_tx.clone()), None);
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
