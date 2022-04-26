use cid::Cid;

use defluencer::{errors::Error, Defluencer};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Friends {
    /// Channel local key name.
    #[structopt(short, long)]
    key_name: String,

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
        Command::Add(args) => add(cli.key_name, args).await,
        Command::Remove(args) => remove(cli.key_name, args).await,
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

async fn add(key: String, args: Followee) -> Result<(), Error> {
    let defluencer = Defluencer::default();

    if let Some(channel) = defluencer.get_local_channel(key).await? {
        channel.follow(args.identity.into()).await?;
    }

    println!("✅ Followee Added");

    Ok(())
}

async fn remove(key: String, args: Followee) -> Result<(), Error> {
    let defluencer = Defluencer::default();

    if let Some(channel) = defluencer.get_local_channel(key).await? {
        channel.unfollow(args.identity.into()).await?;
    }

    println!("✅ Followee Removed");

    Ok(())
}
