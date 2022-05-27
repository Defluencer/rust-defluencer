use cid::Cid;

use clap::Parser;

use defluencer::{
    errors::Error,
    signatures::ledger::{BitcoinLedgerApp, EthereumLedgerApp},
    Defluencer,
};

use futures_util::{future::AbortHandle, pin_mut, FutureExt, StreamExt};

use ipfs_api::IpfsService;

use linked_data::channel::ChannelMetadata;

#[derive(Debug, Parser)]
pub struct NodeCLI {
    #[clap(subcommand)]
    cmd: Command,
}

#[derive(Debug, Parser)]
enum Command {
    // Compute Channel Address from a BTC or ETH account.
    ChannelAddress(Address),

    /// Recursively pin all channel data on this node.
    ///
    /// CAUTION: The amount of data to download could be MASSIVE.
    Pin(Pinning),

    /// Recursively unpin all channel data from this node.
    Unpin(Pinning),

    /// Receive channel updates in real time.
    Subscribe(Subscribe),

    // Stream all the comments for some content on a channel.
    StreamComments(Stream),

    /// Crawl the social web.
    WebCrawl(WebCrawl),
}

pub async fn node_cli(cli: NodeCLI) {
    let res = match cli.cmd {
        Command::ChannelAddress(args) => address(args).await,
        Command::Pin(args) => pin(args).await,
        Command::Unpin(args) => unpin(args).await,
        Command::Subscribe(args) => subscribe(args).await,
        Command::StreamComments(args) => stream_comments(args).await,
        Command::WebCrawl(args) => web_crawl(args).await,
    };

    if let Err(e) = res {
        eprintln!("❗ IPFS: {:#?}", e);
    }
}

#[derive(Debug, Parser)]
pub struct Address {
    /// Bitcoin or Ethereum based signatures.
    #[clap(arg_enum)]
    blockchain: Blockchain,

    /// Account index (BIP-44).
    #[clap(long)]
    account: u32,
}

#[derive(clap::ArgEnum, Clone, Debug)]
enum Blockchain {
    Bitcoin,
    Ethereum,
}

async fn address(args: Address) -> Result<(), Error> {
    let ipns: Cid = match args.blockchain {
        Blockchain::Bitcoin => {
            let app = BitcoinLedgerApp::default();

            let public_key = match app.get_extended_pubkey(args.account) {
                Ok((public_key, _)) => public_key,
                Err(_) => {
                    eprintln!("❗ User Denied Account Access");
                    return Ok(());
                }
            };

            defluencer::utils::pubkey_to_ipns(public_key).into()
        }
        Blockchain::Ethereum => {
            let app = EthereumLedgerApp::default();

            let public_key = match app.get_public_address(args.account) {
                Ok((public_key, _)) => public_key,
                Err(_) => {
                    eprintln!("❗ User Denied Account Access");
                    return Ok(());
                }
            };

            defluencer::utils::pubkey_to_ipns(public_key).into()
        }
    };

    println!("✅ Channel Address {}", ipns);

    Ok(())
}

#[derive(Debug, Parser)]
pub struct Pinning {
    /// Channel IPNS address.
    #[clap(short, long)]
    address: Cid,
}

async fn pin(args: Pinning) -> Result<(), Error> {
    let defluencer = Defluencer::default();

    defluencer.pin_channel(args.address.into()).await?;

    println!("Channel's Content Pinned ✅");

    Ok(())
}

async fn unpin(args: Pinning) -> Result<(), Error> {
    let defluencer = Defluencer::default();

    defluencer.unpin_channel(args.address.into()).await?;

    println!("Channel's Content Unpinned ✅");

    Ok(())
}

#[derive(Debug, Parser)]
pub struct Subscribe {
    /// Channel IPNS address.
    #[clap(short, long)]
    address: Cid,
}

async fn subscribe(args: Subscribe) -> Result<(), Error> {
    use futures_util::TryStreamExt;

    let defluencer = Defluencer::default();

    let (handle, regis) = AbortHandle::new_pair();
    let stream = defluencer.subscribe_ipns_updates(args.address.into(), regis);
    pin_mut!(stream);

    let control = tokio::signal::ctrl_c();
    pin_mut!(control);

    println!(
        "The first update received is the most up to date channel state.\nPress CRTL-C to exit..."
    );

    loop {
        tokio::select! {
            biased;

            _ = &mut control => {
                handle.abort();
                return Ok(());
            }

            result = stream.try_next() => match result {
                Ok(option) => match option {
                    Some(cid) => println!("New Channel Metadata: {}", cid),
                    None => continue,
                },
                Err(e) => return Err(e),
            }
        }
    }
}

#[derive(Debug, Parser)]
pub struct Stream {
    /// Channel IPNS address.
    #[clap(short, long)]
    address: Cid,

    /// Content CID.
    #[clap(short, long)]
    content: Cid,
}

async fn stream_comments(args: Stream) -> Result<(), Error> {
    use futures_util::TryStreamExt;

    let ipfs = IpfsService::default();
    let defluencer = Defluencer::new(ipfs.clone());

    let cid = ipfs.name_resolve(args.address).await?;
    let metadata = ipfs.dag_get::<&str, ChannelMetadata>(cid, None).await?;

    let index = match metadata.comment_index {
        Some(ipns) => ipns,
        None => {
            eprintln!("❗ This channel has no comments.");
            return Ok(());
        }
    };

    let mut stream = defluencer
        .stream_comments(index, args.content)
        .boxed_local();

    while let Some(cid) = stream.try_next().await? {
        println!("{}", cid);
    }

    println!("✅ Comments Stream Ended");

    Ok(())
}

#[derive(Debug, Parser)]
pub struct WebCrawl {
    /// Channel address of starting point.
    #[clap(short, long)]
    origin: Cid,
}

async fn web_crawl(args: WebCrawl) -> Result<(), Error> {
    let defluencer = Defluencer::default();

    let mut stream = defluencer
        .streaming_web_crawl(args.origin.into())
        .boxed_local();

    let mut control = tokio::signal::ctrl_c().boxed_local();

    let mut degree = 1;

    println!("Crawling Start\nPress CRTL-C to exit...");

    loop {
        tokio::select! {
            biased;

            _ = &mut control => {
                println!("✅ Web Crawl Ended");
                return Ok(());
            }

            option = stream.next() => match option {
                Some(result) => match result {
                    Ok(map) => {
                        println!("Degree: {}\nChannels Metadata: {:#?}", degree, map.keys());

                        degree += 1;
                    },
                    Err(_) => continue,

                },
                None => continue,
            }
        }
    }
}
