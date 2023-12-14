use crate::{
    actors::{SetupData, VideoData},
    server::services::put_requests,
};

use std::net::SocketAddr;

use defluencer::errors::Error;

use tokio::{
    net::TcpListener,
    sync::{mpsc::UnboundedSender, watch::Receiver},
};

use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::TokioIo;

use ipfs_api::IpfsService;

pub async fn start_server(
    server_addr: SocketAddr,
    video_tx: UnboundedSender<VideoData>,
    setup_tx: UnboundedSender<SetupData>,
    ipfs: IpfsService,
    mut shutdown: Receiver<()>,
) -> Result<(), Error> {
    let listener = TcpListener::bind(server_addr).await?;

    println!("✅ Ingess Server Online");

    loop {
        tokio::select! {
            res = listener.accept() => {
                let (tcp, _remote_address) = match res {
                    Ok(val) => val,
                    Err(e) => {
                        eprintln!("Tcp listener error: {:#?}", e);
                        continue
                    }
                };

                let io = TokioIo::new(tcp);

                let video_tx = video_tx.clone();
                let setup_tx = setup_tx.clone();
                let ipfs = ipfs.clone();

                let service = service_fn(move |req| {
                    let video_tx = video_tx.clone();
                    let setup_tx = setup_tx.clone();
                    let ipfs = ipfs.clone();

                    put_requests(req, video_tx, setup_tx, ipfs)
                });

                let fut = http1::Builder::new()
                    .half_close(true)
                    .serve_connection(io, service);

                tokio::task::spawn(fut);
            }

            res = shutdown.changed() => {
                match res {
                    Ok(()) => break,
                    Err(e) => {
                        eprintln!("Shutdown receiver error: {:#?}", e);
                        break
                    }
                }
            }
        }
    }

    println!("❌ Ingess Server Offline");

    Ok(())
}
