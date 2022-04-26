use crate::{
    actors::{SetupData, VideoData},
    server::services::put_requests,
};

use std::{convert::Infallible, net::SocketAddr};

use tokio::sync::{mpsc::UnboundedSender, watch::Receiver};

use hyper::{
    service::{make_service_fn, service_fn},
    Server,
};

use ipfs_api::IpfsService;

pub async fn start_server(
    server_addr: SocketAddr,
    video_tx: UnboundedSender<VideoData>,
    setup_tx: UnboundedSender<SetupData>,
    ipfs: IpfsService,
    mut shutdown: Receiver<()>,
) {
    let service = make_service_fn(move |_| {
        let ipfs = ipfs.clone();
        let video_tx = video_tx.clone();
        let setup_tx = setup_tx.clone();

        async move {
            Ok::<_, Infallible>(service_fn(move |req| {
                put_requests(req, video_tx.clone(), setup_tx.clone(), ipfs.clone())
            }))
        }
    });

    let server = Server::bind(&server_addr)
        .http1_half_close(true) //FFMPEG requirement
        .serve(service);

    println!("✅ Ingess Server Online");

    let graceful = server.with_graceful_shutdown(async {
        if let Err(e) = shutdown.changed().await {
            eprintln!("{}", e);
        }
    });

    if let Err(e) = graceful.await {
        eprintln!("Server: {}", e);
    }

    println!("❌ Ingess Server Offline");
}
