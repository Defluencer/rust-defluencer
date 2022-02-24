use hex::FromHex;

use ipfs_api::{errors::Error, IpfsService};

use cid::Cid;

use linked_data::moderation::{Bans, Moderators};
use structopt::StructOpt;

pub const BANS_KEY: &str = "bans";
pub const MODS_KEY: &str = "mods";

#[derive(Debug, StructOpt)]
pub struct Moderation {
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
        Command::Ban(update) => ban_command(update).await,
        Command::Mods(update) => mod_command(update).await,
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

    /// Replace the current list with another.
    ReplaceList(ReplaceBanList),
}

async fn ban_command(cli: BanCommands) -> Result<(), Error> {
    match cli.cmd {
        BanCommand::Add(args) => ban_user(args).await,
        BanCommand::Remove(args) => unban_user(args).await,
        BanCommand::ReplaceList(args) => replace_ban_list(args).await,
    }
}

#[derive(Debug, StructOpt)]
pub struct Ban {
    /// Ethereum Address.
    #[structopt(short, long)]
    address: String,
}

async fn ban_user(args: Ban) -> Result<(), Error> {
    let address = parse_address(&args.address);

    println!("Banning User...");

    let ipfs = IpfsService::default();

    let res = ipfs.ipns_get(BANS_KEY).await?;
    let (old_ban_cid, mut ban_list): (Cid, Bans) = res.unwrap();

    ban_list.banned_addrs.insert(address);

    ipfs.ipns_put(BANS_KEY, false, &ban_list).await?;

    if let Err(e) = ipfs.pin_rm(&old_ban_cid, false).await {
        eprintln!("❗ IPFS could not unpin {}. Error: {}", old_ban_cid, e);
    }

    println!("✅ User {} Banned", args.address);

    Ok(())
}

#[derive(Debug, StructOpt)]
pub struct UnBan {
    /// Ethereum Address.
    #[structopt(short, long)]
    address: String,
}

async fn unban_user(args: UnBan) -> Result<(), Error> {
    let address = parse_address(&args.address);

    println!("Unbanning User...");

    let ipfs = IpfsService::default();

    let res = ipfs.ipns_get(BANS_KEY).await?;
    let (old_ban_cid, mut ban_list): (Cid, Bans) = res.unwrap();

    if ban_list.banned_addrs.remove(&address) {
        ipfs.ipns_put(BANS_KEY, false, &ban_list).await?;

        if let Err(e) = ipfs.pin_rm(&old_ban_cid, false).await {
            eprintln!("❗ IPFS could not unpin {}. Error: {}", old_ban_cid, e);
        }

        println!("✅ User {} Unbanned", args.address);

        return Ok(());
    }

    println!("❗ User {} was not banned", args.address);

    Ok(())
}

#[derive(Debug, StructOpt)]
pub struct ReplaceBanList {
    /// CID of the new ban list.
    #[structopt(long)]
    cid: Cid,
}

async fn replace_ban_list(args: ReplaceBanList) -> Result<(), Error> {
    println!("Replacing Ban List...");

    let ipfs = IpfsService::default();

    let res = ipfs.ipns_get(BANS_KEY).await?;
    let (old_ban_cid, _): (Cid, Bans) = res.unwrap();

    ipfs.pin_add(&args.cid, false).await?;

    ipfs.name_publish(&args.cid, BANS_KEY).await?;

    if let Err(e) = ipfs.pin_rm(&old_ban_cid, false).await {
        eprintln!("❗ IPFS could not unpin {}. Error: {}", old_ban_cid, e);
    }

    println!("✅ Previous Ban List Replaced with {:?}", &args.cid);

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

    /// Replace the current moderator list with another.
    ReplaceModList(ReplaceModList),
}

async fn mod_command(cli: ModCommands) -> Result<(), Error> {
    match cli.cmd {
        ModCommand::Add(args) => mod_user(args).await,
        ModCommand::Remove(args) => unmod_user(args).await,
        ModCommand::ReplaceModList(args) => replace_mod_list(args).await,
    }
}

#[derive(Debug, StructOpt)]
pub struct Mod {
    /// Ethereum address.
    #[structopt(long)]
    address: String,
}

async fn mod_user(args: Mod) -> Result<(), Error> {
    let address = parse_address(&args.address);

    println!("Promoting User...");

    let ipfs = IpfsService::default();

    let res = ipfs.ipns_get(MODS_KEY).await?;
    let (old_mods_cid, mut mods_list): (Cid, Moderators) = res.unwrap();

    mods_list.moderator_addrs.insert(address);

    ipfs.ipns_put(MODS_KEY, false, &mods_list).await?;

    if let Err(e) = ipfs.pin_rm(&old_mods_cid, false).await {
        eprintln!("❗ IPFS could not unpin {}. Error: {}", old_mods_cid, e);
    }

    println!("✅ User {} Promoted To Moderator Position", args.address);

    Ok(())
}

#[derive(Debug, StructOpt)]
pub struct UnMod {
    /// Ethereum address.
    #[structopt(long)]
    address: String,
}

async fn unmod_user(args: UnMod) -> Result<(), Error> {
    let address = parse_address(&args.address);
    println!("Demoting Moderator...");

    let ipfs = IpfsService::default();

    let res = ipfs.ipns_get(MODS_KEY).await?;
    let (old_mods_cid, mut mods_list): (Cid, Moderators) = res.unwrap();

    if mods_list.moderator_addrs.remove(&address) {
        ipfs.ipns_put(MODS_KEY, false, &mods_list).await?;

        if let Err(e) = ipfs.pin_rm(&old_mods_cid, false).await {
            eprintln!("❗ IPFS could not unpin {}. Error: {}", old_mods_cid, e);
        }

        println!("✅ Moderator {} Demoted", args.address);

        return Ok(());
    }

    println!("❗ User {} Was Not A Moderator", args.address);

    Ok(())
}

#[derive(Debug, StructOpt)]
pub struct ReplaceModList {
    /// CID of the new moderator list
    #[structopt(long)]
    cid: Cid,
}

async fn replace_mod_list(args: ReplaceModList) -> Result<(), Error> {
    println!("Replacing Moderator List...");

    let ipfs = IpfsService::default();

    let res = ipfs.ipns_get(MODS_KEY).await?;
    let (old_mods_cid, _): (Cid, Moderators) = res.unwrap();

    ipfs.pin_add(&args.cid, false).await?;

    ipfs.name_publish(&args.cid, MODS_KEY).await?;

    if let Err(e) = ipfs.pin_rm(&old_mods_cid, false).await {
        eprintln!("❗ IPFS could not unpin {}. Error: {}", old_mods_cid, e);
    }

    println!("✅ Previous Moderator List Replaced with {:?}", &args.cid);

    Ok(())
}

fn parse_address(addrs: &str) -> [u8; 20] {
    if let Some(end) = addrs.strip_prefix("0x") {
        return <[u8; 20]>::from_hex(end).expect("Invalid Ethereum Address");
    }

    <[u8; 20]>::from_hex(&addrs).expect("Invalid Ethereum Address")
}
