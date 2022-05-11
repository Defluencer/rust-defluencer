use std::path::PathBuf;

use cid::Cid;

use defluencer::{channel::Channel, errors::Error, signatures::test_signer::TestSigner};

use ipfs_api::{responses::Codec, IpfsService};

use linked_data::identity::Identity;

use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct IdentityCLI {
    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(Debug, StructOpt)]
enum Command {
    /// Create an new identity not associated with a channel.
    Create(Create),

    /// Update channel identity.
    Channel(ChannelIdentity),
}

pub async fn identity_cli(cli: IdentityCLI) {
    let res = match cli.cmd {
        Command::Create(args) => create(args).await,
        Command::Channel(args) => update(args).await,
    };

    if let Err(e) = res {
        eprintln!("❗ IPFS: {:#?}", e);
    }
}

#[derive(Debug, StructOpt)]
pub struct Create {
    /// Display name.
    #[structopt(short, long)]
    display_name: String,

    /// Path to image file.
    #[structopt(short, long)]
    path: PathBuf,

    /// IPNS address.
    #[structopt(short, long)]
    ipns: Option<Cid>,

    /// ENS address.
    #[structopt(short, long)]
    ens: Option<String>,
}

async fn create(args: Create) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let Create {
        display_name,
        path,
        ipns,
        ens,
    } = args;

    let channel_ipns = match ipns {
        Some(ipns) => Some(ipns.into()),
        None => None,
    };

    let avatar = defluencer::utils::add_image(&ipfs, &path).await?.into();

    //TODO make avatar optional then use default avatar cid as needed

    let identity = Identity {
        display_name,
        avatar,
        channel_ipns,
        channel_ens: ens,
    };

    let cid = ipfs.dag_put(&identity, Codec::default()).await?;

    println!("✅ Created Identity {}", cid);

    Ok(())
}

#[derive(Debug, StructOpt)]
pub struct ChannelIdentity {
    /// Channel IPNS Address.
    #[structopt(short, long)]
    address: Cid,

    /// Display name.
    #[structopt(short, long)]
    name: Option<String>,

    /// Path to image file.
    #[structopt(short, long)]
    path: Option<PathBuf>,

    /// IPNS address.
    #[structopt(short, long)]
    ipns: Option<Cid>,

    /// ENS address.
    #[structopt(short, long)]
    ens: Option<String>,
}

async fn update(args: ChannelIdentity) -> Result<(), Error> {
    let ChannelIdentity {
        address,
        name,
        path,
        ipns,
        ens,
    } = args;

    let ipfs = IpfsService::default();

    let signer = TestSigner::default(); //TODO

    let channel = Channel::new(ipfs, address.into(), signer);

    channel.update_identity(name, path, ipns, ens).await?;

    println!("✅ Updated Identity");

    Ok(())
}
