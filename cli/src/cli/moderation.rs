use defluencer::{errors::Error, Defluencer};
use hex::FromHex;

use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Moderation {
    /// Channel local key name.
    #[structopt(short, long)]
    key_name: String,

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
        Command::Ban(args) => ban_command(cli.key_name, args).await,
        Command::Mods(args) => mod_command(cli.key_name, args).await,
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

async fn ban_command(key: String, cli: BanCommands) -> Result<(), Error> {
    match cli.cmd {
        BanCommand::Add(args) => ban_user(key, args).await,
        BanCommand::Remove(args) => unban_user(key, args).await,
    }
}

#[derive(Debug, StructOpt)]
pub struct Ban {
    /// Ethereum Address.
    #[structopt(short, long)]
    address: String,
}

async fn ban_user(key: String, args: Ban) -> Result<(), Error> {
    let address = parse_address(&args.address);

    let defluencer = Defluencer::default();

    if let Some(channel) = defluencer.get_local_channel(key).await? {
        if channel.ban_user(address).await?.is_some() {
            println!("✅ User {} Banned", args.address);

            return Ok(());
        }

        println!("❗ User {} was already banned", args.address);
    }

    Ok(())
}

#[derive(Debug, StructOpt)]
pub struct UnBan {
    /// Ethereum Address.
    #[structopt(short, long)]
    address: String,
}

async fn unban_user(key: String, args: UnBan) -> Result<(), Error> {
    let address = parse_address(&args.address);

    let defluencer = Defluencer::default();

    if let Some(channel) = defluencer.get_local_channel(key).await? {
        if channel.unban_user(&address).await?.is_some() {
            println!("✅ User {} Unbanned", args.address);

            return Ok(());
        }

        println!("❗ User {} was not banned", args.address);
    }

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

async fn mod_command(key: String, cli: ModCommands) -> Result<(), Error> {
    match cli.cmd {
        ModCommand::Add(args) => mod_user(key, args).await,
        ModCommand::Remove(args) => unmod_user(key, args).await,
    }
}

#[derive(Debug, StructOpt)]
pub struct Mod {
    /// Ethereum address.
    #[structopt(long)]
    address: String,
}

async fn mod_user(key: String, args: Mod) -> Result<(), Error> {
    let address = parse_address(&args.address);

    let defluencer = Defluencer::default();

    if let Some(channel) = defluencer.get_local_channel(key).await? {
        if channel.add_moderator(address).await?.is_some() {
            println!("✅ User {} Promoted To Moderator Position", args.address);

            return Ok(());
        }

        println!("❗ User {} was already banned", args.address);
    }

    Ok(())
}

#[derive(Debug, StructOpt)]
pub struct UnMod {
    /// Ethereum address.
    #[structopt(long)]
    address: String,
}

async fn unmod_user(key: String, args: UnMod) -> Result<(), Error> {
    let address = parse_address(&args.address);

    let defluencer = Defluencer::default();

    if let Some(channel) = defluencer.get_local_channel(key).await? {
        if channel.remove_moderator(&address).await?.is_some() {
            println!("✅ Moderator {} Demoted", args.address);

            return Ok(());
        }

        println!("❗ User {} Was Not A Moderator", args.address);
    }

    Ok(())
}

fn parse_address(addrs: &str) -> [u8; 20] {
    if let Some(end) = addrs.strip_prefix("0x") {
        return <[u8; 20]>::from_hex(end).expect("Invalid Ethereum Address");
    }

    <[u8; 20]>::from_hex(&addrs).expect("Invalid Ethereum Address")
}
