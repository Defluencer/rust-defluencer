use cid::Cid;

use defluencer::{channel::Channel, errors::Error, signatures::TestSigner};

use hex::FromHex;

use ipfs_api::IpfsService;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Moderation {
    /// Channel IPNS Address.
    #[structopt(short, long)]
    address: Cid,

    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(Debug, StructOpt)]
enum Command {
    /// Manage list of banned users.
    Ban(BanCommands),

    /// Manage list of moderators.
    Mods(ModCommands),
}

pub async fn moderation_cli(cli: Moderation) {
    let res = match cli.cmd {
        Command::Ban(args) => match args.cmd {
            BanCommand::Add(args) => ban_user(cli.address, args).await,
            BanCommand::Remove(args) => unban_user(cli.address, args).await,
        },
        Command::Mods(args) => match args.cmd {
            ModCommand::Add(args) => mod_user(cli.address, args).await,
            ModCommand::Remove(args) => unmod_user(cli.address, args).await,
        },
    };

    if let Err(e) = res {
        eprintln!("❗ IPFS: {:#?}", e);
    }
}

#[derive(Debug, StructOpt)]
struct BanCommands {
    #[structopt(subcommand)]
    cmd: BanCommand,
}

#[derive(Debug, StructOpt)]
enum BanCommand {
    /// Ban users.
    Add(Ban),

    /// Unban users.
    Remove(UnBan),
}

#[derive(Debug, StructOpt)]
pub struct Ban {
    /// Ethereum Address.
    #[structopt(short, long)]
    address: String,
}

async fn ban_user(addr: Cid, args: Ban) -> Result<(), Error> {
    let address = parse_address(&args.address);

    let ipfs = IpfsService::default();

    let signer = TestSigner::default(); //TODO

    let channel = Channel::new(ipfs, addr.into(), signer);

    if channel.ban_user(address).await?.is_some() {
        println!("✅ User {} Banned", args.address);

        return Ok(());
    }

    println!("❗ User {} was already banned", args.address);

    Ok(())
}

#[derive(Debug, StructOpt)]
pub struct UnBan {
    /// Ethereum Address.
    #[structopt(short, long)]
    address: String,
}

async fn unban_user(addr: Cid, args: UnBan) -> Result<(), Error> {
    let address = parse_address(&args.address);

    let ipfs = IpfsService::default();

    let signer = TestSigner::default(); //TODO

    let channel = Channel::new(ipfs, addr.into(), signer);

    if channel.unban_user(&address).await?.is_some() {
        println!("✅ User {} Unbanned", args.address);

        return Ok(());
    }

    println!("❗ User {} was not banned", args.address);

    Ok(())
}

#[derive(Debug, StructOpt)]
struct ModCommands {
    #[structopt(subcommand)]
    cmd: ModCommand,
}

#[derive(Debug, StructOpt)]
enum ModCommand {
    /// Promote user to moderator position.
    Add(Mod),

    /// Demote user from moderator position.
    Remove(UnMod),
}

#[derive(Debug, StructOpt)]
pub struct Mod {
    /// Ethereum address.
    #[structopt(long)]
    address: String,
}

async fn mod_user(addr: Cid, args: Mod) -> Result<(), Error> {
    let address = parse_address(&args.address);

    let ipfs = IpfsService::default();

    let signer = TestSigner::default(); //TODO

    let channel = Channel::new(ipfs, addr.into(), signer);

    if channel.add_moderator(address).await?.is_some() {
        println!("✅ User {} Promoted To Moderator Position", args.address);

        return Ok(());
    }

    println!("❗ User {} was already banned", args.address);

    Ok(())
}

#[derive(Debug, StructOpt)]
pub struct UnMod {
    /// Ethereum address.
    #[structopt(long)]
    address: String,
}

async fn unmod_user(addr: Cid, args: UnMod) -> Result<(), Error> {
    let address = parse_address(&args.address);

    let ipfs = IpfsService::default();

    let signer = TestSigner::default(); //TODO

    let channel = Channel::new(ipfs, addr.into(), signer);

    if channel.remove_moderator(&address).await?.is_some() {
        println!("✅ Moderator {} Demoted", args.address);

        return Ok(());
    }

    println!("❗ User {} Was Not A Moderator", args.address);

    Ok(())
}

fn parse_address(addrs: &str) -> [u8; 20] {
    if let Some(end) = addrs.strip_prefix("0x") {
        return <[u8; 20]>::from_hex(end).expect("Invalid Ethereum Address");
    }

    <[u8; 20]>::from_hex(&addrs).expect("Invalid Ethereum Address")
}
