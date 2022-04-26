use defluencer::{errors::Error, Defluencer};

use futures_util::{stream::FuturesUnordered, StreamExt};

use ipfs_api::IpfsService;

use linked_data::channel::ChannelMetadata;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct ChannelCLI {
    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(Debug, StructOpt)]
enum Command {
    /// Create a new channel.
    Create(Create),

    /// Pin a channel.
    /// Will recursively pin all associated data.
    /// The amount of data to be pinned could be MASSIVE use carefully.
    Pin(Pinning),

    /// Unpin a channel.
    /// Recursively unpin all associated data.
    Unpin(Pinning),

    /// Import a channel from a secret phrase.
    Import(Import),

    /// List all local channels on this IPFS node.
    List,
}

pub async fn channel_cli(cli: ChannelCLI) {
    let res = match cli.cmd {
        Command::Create(args) => create(args).await,
        Command::Pin(args) => pin(args).await,
        Command::Unpin(args) => unpin(args).await,
        Command::Import(args) => import(args).await,
        Command::List => list().await,
    };

    if let Err(e) = res {
        eprintln!("❗ IPFS: {:#?}", e);
    }
}

#[derive(Debug, StructOpt)]
pub struct Create {
    /// Your choosen channel name.
    #[structopt(short, long)]
    display_name: String,
}

async fn create(args: Create) -> Result<(), Error> {
    let defluencer = Defluencer::default();

    let (mnemonic, channel, ipns) = defluencer.create_local_channel(args.display_name).await?;

    println!(
        "✅ Channel Created\nIPNS Address: {}\nLocal Key Name: {}\nSecret Phrase: {}",
        ipns,
        channel.get_name(),
        mnemonic.phrase()
    );

    Ok(())
}

#[derive(Debug, StructOpt)]
pub struct Pinning {
    /// Channel local key name.
    #[structopt(short, long)]
    key_name: String,
}

async fn pin(args: Pinning) -> Result<(), Error> {
    let defluencer = Defluencer::default();

    if let Some(channel) = defluencer.get_local_channel(args.key_name).await? {
        channel.pin_channel().await?;

        println!("Channel's Content Pinned ✅");
    }

    Ok(())
}

async fn unpin(args: Pinning) -> Result<(), Error> {
    let defluencer = Defluencer::default();

    if let Some(channel) = defluencer.get_local_channel(args.key_name).await? {
        channel.unpin_channel().await?;

        println!("Channel's Content Unpinned ✅");
    }

    Ok(())
}

#[derive(Debug, StructOpt)]
pub struct Import {
    /// The channel name.
    #[structopt(short, long)]
    display_name: String,

    /// The secret phrase given at channel creation.
    #[structopt(short, long)]
    secret_phrase: String,

    /// Should pin channel content?
    #[structopt(short, long)]
    pin: bool,
}

async fn import(args: Import) -> Result<(), Error> {
    let defluencer = Defluencer::default();

    let channel = defluencer
        .import_channel(args.display_name, args.secret_phrase, args.pin)
        .await?;

    println!(
        "Channel Imported ✅\nLocal Key Name: {}",
        channel.get_name()
    );

    Ok(())
}

async fn list() -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let list = ipfs.key_list().await?;

    println!("Local Keys:");

    let stream: FuturesUnordered<_> = list
        .into_iter()
        .map(|(name, ipns)| {
            let ipfs = ipfs.clone();

            async move {
                let result = ipfs.name_resolve(ipns).await;

                (name, result)
            }
        })
        .collect();

    let list: Vec<String> = stream
        .filter_map(|(name, result)| {
            let ipfs = ipfs.clone();

            async move {
                match result {
                    Ok(cid) => match ipfs.dag_get::<&str, ChannelMetadata>(cid, None).await {
                        Ok(_) => Some(name),
                        Err(_) => None,
                    },
                    Err(_) => None,
                }
            }
        })
        .collect()
        .await;

    for name in list {
        println!("{}", name);
    }

    Ok(())
}
