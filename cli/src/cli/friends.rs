use cid::Cid;

use defluencer::{channel::Channel, errors::Error, signatures::test_signer::TestSigner};

use ipfs_api::IpfsService;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Friends {
    /// Channel IPNS Address.
    #[structopt(short, long)]
    address: Cid,

    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(Debug, StructOpt)]
enum Command {
    /// Add a new followee to your list.
    Add(Followee),

    /// Remove a followee from your list.
    Remove(Followee),
}

pub async fn friends_cli(cli: Friends) {
    let res = match cli.cmd {
        Command::Add(args) => add(cli.address, args).await,
        Command::Remove(args) => remove(cli.address, args).await,
    };

    if let Err(e) = res {
        eprintln!("❗ IPFS: {:#?}", e);
    }
}

#[derive(Debug, StructOpt)]
pub struct Followee {
    /// Followee's current identity CID.
    #[structopt(short, long)]
    identity: Cid,
}

async fn add(addr: Cid, args: Followee) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let signer = TestSigner::default(); //TODO

    let channel = Channel::new(ipfs, addr.into(), signer);

    channel.follow(args.identity.into()).await?;

    println!("✅ Followee Added");

    Ok(())
}

async fn remove(addr: Cid, args: Followee) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let signer = TestSigner::default(); //TODO

    let channel = Channel::new(ipfs, addr.into(), signer);

    channel.unfollow(args.identity.into()).await?;

    println!("✅ Followee Removed");

    Ok(())
}
