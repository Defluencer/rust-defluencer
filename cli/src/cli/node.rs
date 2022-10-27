use std::path::PathBuf;

use clap::{Parser, Subcommand};

use defluencer::{errors::Error, utils::add_image, Defluencer};

use futures_util::{future::AbortHandle, pin_mut, stream::Abortable, StreamExt};

use ipfs_api::{responses::Codec, IpfsService};

use linked_data::{channel::ChannelMetadata, types::IPNSAddress};

#[derive(Debug, Subcommand)]
pub enum NodeCLI {
    /// Create a new identity. Must have an IPNS address if creating a channel.
    Identity(Identity),

    /* /// Compute channel address from a BTC or ETH account.
    Address(Address), */
    /// Recursively pin all channel data on this node.
    /// CAUTION: The amount of data to download could be MASSIVE.
    Pin(Address),

    /// Recursively unpin all channel data from this node.
    /// CAUTION: The data can now be deleted by the garbage collector at any time.
    Unpin(Address),

    /// Receive channel updates in real time.
    /// The first CID received is the most up to date channel metadata not a live update.
    Subscribe(Address),

    /// Receive requests for content aggregation.
    Aggregate(Address),

    /// Stream all content & comments from a channel.
    Stream(Stream),

    /// Crawl the social web, returns channel metadata CIDs without duplicates.
    Webcrawl(Address),
}

pub async fn node_cli(cli: NodeCLI) {
    let res = match cli {
        NodeCLI::Identity(args) => create_id(args).await,
        //Command::Address(args) => address(args).await,
        NodeCLI::Pin(args) => pin(args).await,
        NodeCLI::Unpin(args) => unpin(args).await,
        NodeCLI::Subscribe(args) => subscribe(args).await,
        NodeCLI::Aggregate(args) => agregate(args).await,
        NodeCLI::Stream(stream_cli) => match stream_cli.cmd {
            SubCommand::Content => stream_content(stream_cli.address).await,
            SubCommand::Comments => stream_comments(stream_cli.address).await,
        },
        NodeCLI::Webcrawl(args) => web_crawl(args).await,
    };

    if let Err(e) = res {
        eprintln!("❗ IPFS: {:#?}", e);
    }
}

#[derive(Debug, Parser)]
pub struct Identity {
    /// Choosen name.
    #[arg(long)]
    name: String,

    /// User short biography. (Optional)
    #[arg(long)]
    bio: Option<String>,

    /// Path to banner image file. (Optional)
    #[arg(long)]
    banner: Option<PathBuf>,

    /// Path to avatar image file. (Optional)
    #[arg(long)]
    avatar: Option<PathBuf>,

    /// IPNS address. (Optional)
    #[arg(long)]
    ipns_addr: Option<IPNSAddress>,

    /// Bitcoin address. (Optional)
    #[arg(long)]
    btc_addr: Option<String>,

    /// Ethereum address. (Optional)
    #[arg(long)]
    eth_addr: Option<String>,
}

async fn create_id(args: Identity) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let Identity {
        name,
        bio,
        banner,
        avatar,
        ipns_addr,
        btc_addr,
        eth_addr,
    } = args;

    let banner = if let Some(path) = banner {
        Some(add_image(&ipfs, path).await?.into())
    } else {
        None
    };

    let avatar = if let Some(path) = avatar {
        Some(add_image(&ipfs, path).await?.into())
    } else {
        None
    };

    let identity = linked_data::identity::Identity {
        name,
        bio,
        banner,
        avatar,
        ipns_addr,
        btc_addr,
        eth_addr,
    };

    let cid = ipfs.dag_put(&identity, Codec::default()).await?;

    println!("✅ User Identity Created\nCID: {}", cid);

    Ok(())
}

/* #[derive(Debug, Parser)]
pub struct Address {
    /// Bitcoin or Ethereum based signatures.
    #[arg(arg_enum, default_value = "bitcoin")]
    blockchain: Blockchain,

    /// Account index (BIP-44).
    #[arg(long, default_value = "0")]
    account: u32,
} */

/* #[derive(arg::ArgEnum, Clone, Debug)]
enum Blockchain {
    Bitcoin,
    Ethereum,
} */

/* async fn address(args: Address) -> Result<(), Error> {
    println!("Authorize Your Hardware Wallet...");

    let ipns: Cid = match args.blockchain {
        Blockchain::Bitcoin => {
            let app = BitcoinLedgerApp::default();

            let (public_key, _) = app.get_extended_pubkey(args.account)?;

            defluencer::utils::pubkey_to_ipns(public_key).into()
        }
        Blockchain::Ethereum => {
            let app = EthereumLedgerApp::default();

            let (public_key, _) = app.get_public_address(args.account)?;

            defluencer::utils::pubkey_to_ipns(public_key).into()
        }
    };

    println!("✅ Channel Address {}", ipns);

    Ok(())
} */

#[derive(Debug, Parser)]
pub struct Address {
    /// Channel IPNS address.
    #[arg(long)]
    address: IPNSAddress,
}

async fn pin(args: Address) -> Result<(), Error> {
    let defluencer = Defluencer::default();

    defluencer.pin_channel(args.address).await?;

    println!("✅ Channel's Content Pinned");

    Ok(())
}

async fn unpin(args: Address) -> Result<(), Error> {
    let defluencer = Defluencer::default();

    defluencer.unpin_channel(args.address).await?;

    println!("✅ Channel's Content Unpinned");

    Ok(())
}

async fn subscribe(args: Address) -> Result<(), Error> {
    use futures_util::TryStreamExt;

    let defluencer = Defluencer::default();

    let (handle, regis) = AbortHandle::new_pair();
    let stream = defluencer.subscribe_channel_updates(args.address);
    let stream = Abortable::new(stream, regis);
    pin_mut!(stream);

    let control = tokio::signal::ctrl_c();
    pin_mut!(control);

    println!("✅ Receiver Ready!\nPress CRTL-C to exit...");

    loop {
        tokio::select! {
            biased;

            _ = &mut control => {
                handle.abort();
                println!("✅ Subscription Stopped");
                return Ok(());
            }

            result = stream.try_next() => match result {
                Ok(option) => match option {
                    Some(cid) => println!("Channel Root Signature: {}", cid),
                    None => continue,
                },
                Err(e) => return Err(e),
            }
        }
    }
}

async fn agregate(args: Address) -> Result<(), Error> {
    use futures_util::TryStreamExt;

    let ipfs = IpfsService::default();
    let defluencer = Defluencer::from(ipfs.clone());

    let cid = ipfs.name_resolve(args.address.into()).await?;

    let meta = ipfs.dag_get::<&str, ChannelMetadata>(cid, None).await?;

    let topic = match meta.agregation_channel {
        Some(tp) => tp,
        None => {
            eprintln!("❗ This channel has no aggregation topic");
            return Ok(());
        }
    };

    let (handle, regis) = AbortHandle::new_pair();
    let stream = defluencer.subscribe_agregation_updates(topic);
    let stream = Abortable::new(stream, regis);
    pin_mut!(stream);

    let control = tokio::signal::ctrl_c();
    pin_mut!(control);

    println!("✅ Receiver Ready!\nPress CRTL-C to exit...");

    loop {
        tokio::select! {
            biased;

            _ = &mut control => {
                handle.abort();
                println!("✅ Aggregation Stopped");
                return Ok(());
            }

            result = stream.try_next() => match result {
                Ok(option) => match option {
                    Some(cid) => println!("Content CID: {}", cid),
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
    #[arg(long)]
    address: IPNSAddress,

    #[command(subcommand)]
    cmd: SubCommand,
}

#[derive(Debug, Subcommand)]
pub enum SubCommand {
    /// Stream chronologicaly all the content on a channel.
    Content,

    /// Stream all the comments on a channel.
    Comments,
}

async fn stream_comments(addr: IPNSAddress) -> Result<(), Error> {
    use futures_util::TryStreamExt;

    let ipfs = IpfsService::default();
    let defluencer = Defluencer::from(ipfs.clone());

    let cid = ipfs.name_resolve(addr.into()).await?;
    let metadata = ipfs.dag_get::<&str, ChannelMetadata>(cid, None).await?;

    let index = match metadata.comment_index {
        Some(ipns) => ipns,
        None => {
            eprintln!("❗ This channel has no comments.");
            return Ok(());
        }
    };

    let stream = defluencer.stream_all_comments(index);

    pin_mut!(stream);

    println!("✅ Streaming Comments CIDs...");

    while let Some((media, comment)) = stream.try_next().await? {
        println!("Media: {} Comment: {}", media, comment);
    }

    println!("✅ Comments Stream Finished");

    Ok(())
}

async fn stream_content(addr: IPNSAddress) -> Result<(), Error> {
    use futures_util::TryStreamExt;

    let ipfs = IpfsService::default();
    let defluencer = Defluencer::from(ipfs.clone());

    let cid = ipfs.name_resolve(addr.into()).await?;
    let metadata = ipfs.dag_get::<&str, ChannelMetadata>(cid, None).await?;

    let index = match metadata.content_index {
        Some(ipns) => ipns,
        None => {
            eprintln!("❗ This channel has no content.");
            return Ok(());
        }
    };

    let stream = defluencer.stream_content_rev_chrono(index);

    pin_mut!(stream);

    println!("✅ Streaming Content CIDs...");

    while let Some(cid) = stream.try_next().await? {
        println!("{}", cid);
    }

    println!("✅ Content Stream Finished");

    Ok(())
}

async fn web_crawl(args: Address) -> Result<(), Error> {
    let defluencer = Defluencer::default();

    let stream = defluencer.streaming_web_crawl(std::iter::once(args.address));
    let control = tokio::signal::ctrl_c();

    pin_mut!(stream);
    pin_mut!(control);

    println!("✅ Crawling Start\nPress CRTL-C to exit...");

    loop {
        tokio::select! {
            biased;

            _ = &mut control => {
                println!("✅ Web Crawl Stopped");
                return Ok(());
            }

            option = stream.next() => match option {
                Some(result) => match result {
                    Ok((cid, _channel)) => {
                        println!("Channel Metadata CID: {}",  cid);
                    },
                    Err(_) => continue,

                },
                None => {
                    println!("✅ Web Crawl Finished");
                    return Ok(())},
            }
        }
    }
}
