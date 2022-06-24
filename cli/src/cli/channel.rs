use cid::Cid;

use defluencer::{
    channel::Channel,
    errors::Error,
    signatures::{
        bitcoin::BitcoinSigner,
        ethereum::EthereumSigner,
        ledger::{BitcoinLedgerApp, EthereumLedgerApp},
        Signer,
    },
};

use heck::ToSnakeCase;
use ipfs_api::IpfsService;

use clap::Parser;

use linked_data::{channel::ChannelMetadata, identity::Identity, types::PeerId};

#[derive(Debug, Parser)]
pub struct ChannelCLI {
    /// Bitcoin or Ethereum based signatures.
    #[clap(arg_enum, default_value = "bitcoin")]
    blockchain: Blockchain,

    /// Account index (BIP-44).
    #[clap(long, default_value = "0")]
    account: u32,

    /// Identity CID.
    #[clap(short, long)]
    identity: Cid,

    #[clap(subcommand)]
    cmd: Command,
}

#[derive(clap::ArgEnum, Clone, Debug)]
enum Blockchain {
    Bitcoin,
    Ethereum,
}

#[derive(Debug, Parser)]
enum Command {
    /// Create a new channel.
    Create,

    /// Manage your content.
    Content(ContentLog),

    /* /// Update your identity.
    Identity(Identity), */
    /// Manage your followees.
    Follow(Friends),

    /// Update your live settings.
    Live(Live),

    /// Moderate live chat.
    Moderation(Moderation),
}

pub async fn channel_cli(cli: ChannelCLI) {
    let res = match cli.blockchain {
        Blockchain::Bitcoin => {
            let app = BitcoinLedgerApp::default();

            let signer = BitcoinSigner::new(app, cli.account);

            match cli.cmd {
                Command::Create => create_channel(cli.identity).await,
                Command::Content(args) => match args.cmd {
                    ContentCommand::Add(args) => add_content(cli.identity, args, signer).await,
                    ContentCommand::Remove(args) => {
                        remove_content(cli.identity, args, signer).await
                    }
                },
                //Command::Identity(args) => update_identity(args, ipns, signer).await,
                Command::Follow(args) => match args.cmd {
                    FollowCommand::Add(args) => add_followee(cli.identity, args, signer).await,
                    FollowCommand::Remove(args) => {
                        remove_followee(cli.identity, args, signer).await
                    }
                },
                Command::Live(args) => update_live(cli.identity, args, signer).await,
                Command::Moderation(args) => match args.cmd {
                    ModerationCommand::Ban(args) => ban_user(cli.identity, args, signer).await,
                    ModerationCommand::Unban(args) => unban_user(cli.identity, args, signer).await,
                    ModerationCommand::Mod(args) => mod_user(cli.identity, args, signer).await,
                    ModerationCommand::Unmod(args) => unmod_user(cli.identity, args, signer).await,
                },
            }
        }
        Blockchain::Ethereum => {
            let app = EthereumLedgerApp::default();

            let signer = EthereumSigner::new(app, cli.account);

            match cli.cmd {
                Command::Create => create_channel(cli.identity).await,
                Command::Content(args) => match args.cmd {
                    ContentCommand::Add(args) => add_content(cli.identity, args, signer).await,
                    ContentCommand::Remove(args) => {
                        remove_content(cli.identity, args, signer).await
                    }
                },
                //Command::Identity(args) => update_identity(args, ipns, signer).await,
                Command::Follow(args) => match args.cmd {
                    FollowCommand::Add(args) => add_followee(cli.identity, args, signer).await,
                    FollowCommand::Remove(args) => {
                        remove_followee(cli.identity, args, signer).await
                    }
                },
                Command::Live(args) => update_live(cli.identity, args, signer).await,
                Command::Moderation(args) => match args.cmd {
                    ModerationCommand::Ban(args) => ban_user(cli.identity, args, signer).await,
                    ModerationCommand::Unban(args) => unban_user(cli.identity, args, signer).await,
                    ModerationCommand::Mod(args) => mod_user(cli.identity, args, signer).await,
                    ModerationCommand::Unmod(args) => unmod_user(cli.identity, args, signer).await,
                },
            }
        }
    };

    if let Err(e) = res {
        eprintln!("❗ IPFS: {:#?}", e);
    }
}

/* #[derive(Debug, Parser)]
pub struct Create {} */

async fn create_channel(identity: Cid) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let metadata = ChannelMetadata {
        identity: identity.into(),
        ..Default::default()
    };

    let identity = ipfs.dag_get::<String, Identity>(identity, None).await?;

    let key = identity.display_name.to_snake_case();

    ipfs.ipns_put(key, true, &metadata).await?;

    println!("✅ Created Channel");

    Ok(())
}

#[derive(Debug, Parser)]
pub struct ContentLog {
    #[clap(subcommand)]
    cmd: ContentCommand,
}

#[derive(Debug, Parser)]
enum ContentCommand {
    /// Add content to your channel.
    Add(Content),

    /// Remove content from your channel.
    Remove(Content),
}

#[derive(Debug, Parser)]
pub struct Content {
    /// The CID of the content.
    #[clap(short, long)]
    cid: Cid,
}

async fn add_content(
    identity: Cid,
    args: Content,
    signer: impl Signer + Clone,
) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let identity = ipfs.dag_get::<String, Identity>(identity, None).await?;
    let key = identity.display_name.to_snake_case();

    let channel = Channel::new(ipfs, key, signer);

    println!("Confirm Signature On Your Hardware Wallet...");

    channel.add_content(args.cid).await?;

    println!("✅ Added Content {}", args.cid);

    Ok(())
}

async fn remove_content(
    identity: Cid,
    args: Content,
    signer: impl Signer + Clone,
) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let identity = ipfs.dag_get::<String, Identity>(identity, None).await?;
    let key = identity.display_name.to_snake_case();

    let channel = Channel::new(ipfs, key, signer);

    channel.remove_content(args.cid).await?;

    println!("✅ Comments Cleared & Removed Content {}", args.cid);

    Ok(())
}

/* #[derive(Debug, Parser)]
pub struct Identity {
    /// Display name.
    #[clap(short, long)]
    display_name: Option<String>,

    /// Path to image file.
    #[clap(short, long)]
    path: Option<PathBuf>,
}

async fn update_identity(
    args: Identity,
    ipns: IPNSAddress,
    signer: impl Signer + Clone,
) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let channel = Channel::new(ipfs, ipns, signer);

    let cid = channel
        .update_identity(args.display_name, args.path, Some(ipns.into()))
        .await?;

    println!("✅ Updated Channel Identity {}", cid);

    Ok(())
} */

#[derive(Debug, Parser)]
pub struct Friends {
    #[clap(subcommand)]
    cmd: FollowCommand,
}

#[derive(Debug, Parser)]
enum FollowCommand {
    /// Add a new followee to your list.
    Add(Followee),

    /// Remove a followee from your list.
    Remove(Followee),
}

#[derive(Debug, Parser)]
pub struct Followee {
    /// Followee's channel address.
    #[clap(short, long)]
    address: Cid,
}

async fn add_followee(
    identity: Cid,
    args: Followee,

    signer: impl Signer + Clone,
) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let identity = ipfs.dag_get::<String, Identity>(identity, None).await?;
    let key = identity.display_name.to_snake_case();

    let channel = Channel::new(ipfs, key, signer);

    channel.follow(args.address.into()).await?;

    println!("✅ Added Followee {}", args.address);

    Ok(())
}

async fn remove_followee(
    identity: Cid,
    args: Followee,
    signer: impl Signer + Clone,
) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let identity = ipfs.dag_get::<String, Identity>(identity, None).await?;
    let key = identity.display_name.to_snake_case();

    let channel = Channel::new(ipfs, key, signer);

    channel.unfollow(args.address.into()).await?;

    println!("✅ Removed Followee {}", args.address);

    Ok(())
}

#[derive(Debug, Parser)]
pub struct Live {
    /// Peer Id of the node live streaming.
    #[clap(short, long)]
    peer_id: Option<String>,

    /// PubSub Topic for live video.
    #[clap(short, long)]
    video_topic: Option<String>,

    /// PubSub Topic for live chat.
    #[clap(short, long)]
    chat_topic: Option<String>,

    /// Should live chat be archived.
    #[clap(short, long)]
    archiving: Option<bool>,
}

async fn update_live(identity: Cid, args: Live, signer: impl Signer + Clone) -> Result<(), Error> {
    let Live {
        peer_id,
        video_topic,
        chat_topic,
        archiving,
    } = args;

    let ipfs = IpfsService::default();

    let identity = ipfs.dag_get::<String, Identity>(identity, None).await?;
    let key = identity.display_name.to_snake_case();

    let channel = Channel::new(ipfs, key, signer);

    let peer_id = if let Some(peer) = peer_id {
        match PeerId::try_from(peer) {
            Ok(peer) => Some(peer.into()),
            Err(e) => {
                eprintln!("{}", e);

                None
            }
        }
    } else {
        None
    };

    let cid = channel
        .update_live_settings(peer_id, video_topic, chat_topic, archiving)
        .await?;

    println!("✅ Updated Live Settings {}", cid);

    Ok(())
}

#[derive(Debug, Parser)]
struct Moderation {
    #[clap(subcommand)]
    cmd: ModerationCommand,
}

#[derive(Debug, Parser)]
enum ModerationCommand {
    /// Ban users.
    Ban(Ban),

    /// Unban users.
    Unban(Ban),

    /// Promote user to moderator position.
    Mod(Mod),

    /// Demote user from moderator position.
    Unmod(Mod),
}

#[derive(Debug, Parser)]
pub struct Mod {
    /// Ethereum address.
    #[clap(long)]
    address: String,
}

#[derive(Debug, Parser)]
pub struct Ban {
    /// Ethereum Address.
    #[clap(short, long)]
    address: String,
}

fn parse_address(addrs: &str) -> [u8; 20] {
    use hex::FromHex;

    if let Some(end) = addrs.strip_prefix("0x") {
        return <[u8; 20]>::from_hex(end).expect("Invalid Ethereum Address");
    }

    <[u8; 20]>::from_hex(&addrs).expect("Invalid Ethereum Address")
}

async fn ban_user(identity: Cid, args: Ban, signer: impl Signer + Clone) -> Result<(), Error> {
    let address = parse_address(&args.address);

    let ipfs = IpfsService::default();

    let identity = ipfs.dag_get::<String, Identity>(identity, None).await?;
    let key = identity.display_name.to_snake_case();

    let channel = Channel::new(ipfs, key, signer);

    if channel.ban_user(address).await?.is_some() {
        println!("✅ User {} Banned", args.address);

        return Ok(());
    }

    println!("❗ User {} was already banned", args.address);

    Ok(())
}

async fn unban_user(identity: Cid, args: Ban, signer: impl Signer + Clone) -> Result<(), Error> {
    let address = parse_address(&args.address);

    let ipfs = IpfsService::default();

    let identity = ipfs.dag_get::<String, Identity>(identity, None).await?;
    let key = identity.display_name.to_snake_case();

    let channel = Channel::new(ipfs, key, signer);

    if channel.unban_user(&address).await?.is_some() {
        println!("✅ User {} Unbanned", args.address);

        return Ok(());
    }

    println!("❗ User {} was not banned", args.address);

    Ok(())
}

async fn mod_user(identity: Cid, args: Mod, signer: impl Signer + Clone) -> Result<(), Error> {
    let address = parse_address(&args.address);

    let ipfs = IpfsService::default();

    let identity = ipfs.dag_get::<String, Identity>(identity, None).await?;
    let key = identity.display_name.to_snake_case();

    let channel = Channel::new(ipfs, key, signer);

    if channel.add_moderator(address).await?.is_some() {
        println!("✅ User {} Promoted To Moderator", args.address);

        return Ok(());
    }

    println!("❗ User {} was already banned", args.address);

    Ok(())
}

async fn unmod_user(identity: Cid, args: Mod, signer: impl Signer + Clone) -> Result<(), Error> {
    let address = parse_address(&args.address);

    let ipfs = IpfsService::default();

    let identity = ipfs.dag_get::<String, Identity>(identity, None).await?;
    let key = identity.display_name.to_snake_case();

    let channel = Channel::new(ipfs, key, signer);

    if channel.remove_moderator(&address).await?.is_some() {
        println!("✅ Moderator {} Demoted", args.address);

        return Ok(());
    }

    println!("❗ User {} Was Not A Moderator", args.address);

    Ok(())
}
