use cid::Cid;

use defluencer::{
    channel::{local::LocalUpdater, Channel},
    errors::Error,
};

use heck::ToSnakeCase;

use ipfs_api::IpfsService;

use clap::Parser;

use linked_data::{
    identity::Identity,
    types::{IPNSAddress, PeerId},
};

//TODO add --no-signature option then make having a signature the default.
// Require Ldeger Nano App for IPNS record creation

#[derive(Debug, Parser)]
pub struct ChannelCLI {
    /* /// Bitcoin or Ethereum based signatures.
    #[clap(arg_enum, default_value = "bitcoin")]
    blockchain: Blockchain,

    /// Account index (BIP-44).
    #[clap(long, default_value = "0")]
    account: u32, */
    /// Identity CID.
    #[clap(short, long)]
    identity: Cid,

    #[clap(subcommand)]
    cmd: Command,
}

/* #[derive(clap::ArgEnum, Clone, Debug)]
enum Blockchain {
    Bitcoin,
    Ethereum,
} */

#[derive(Debug, Parser)]
enum Command {
    /// Create a new channel.
    Create,

    /// Manage your content.
    Content(Manage),

    /// Manage your comments.
    Comment(Manage),

    /// Manage your followees.
    Follow(Friends),

    /// Update your live settings.
    Live(Live),

    /// Moderate live chat.
    Moderation(Moderation),
}

pub async fn channel_cli(cli: ChannelCLI) {
    /* let res = match cli.blockchain {
        Blockchain::Bitcoin => {
            let app = BitcoinLedgerApp::default();

            let signer = BitcoinSigner::new(app, cli.account);

            match cli.cmd {
                Command::Create => create_channel(cli.identity).await,
                Command::Content(args) => match args.cmd {
                    AddRemoveCommand::Add(args) => add_content(cli.identity, args).await,
                    AddRemoveCommand::Remove(args) => remove_content(cli.identity, args).await,
                },
                Command::Comment(args) => match args.cmd {
                    AddRemoveCommand::Add(args) => add_comment(cli.identity, args).await,
                    AddRemoveCommand::Remove(args) => remove_comment(cli.identity, args).await,
                },
                //Command::Identity(args) => update_identity(args, ipns, signer).await,
                Command::Follow(args) => match args.cmd {
                    FollowCommand::Add(args) => add_followee(cli.identity, args).await,
                    FollowCommand::Remove(args) => remove_followee(cli.identity, args).await,
                },
                Command::Live(args) => update_live(cli.identity, args).await,
                Command::Moderation(args) => match args.cmd {
                    ModerationCommand::Ban(args) => ban_user(cli.identity, args).await,
                    ModerationCommand::Unban(args) => unban_user(cli.identity, args).await,
                    ModerationCommand::Mod(args) => mod_user(cli.identity, args).await,
                    ModerationCommand::Unmod(args) => unmod_user(cli.identity, args).await,
                },
            }
        }
        Blockchain::Ethereum => {
            let app = EthereumLedgerApp::default();

            let signer = EthereumSigner::new(app, cli.account);

            match cli.cmd {
                Command::Create => create_channel(cli.identity).await,
                Command::Content(args) => match args.cmd {
                    AddRemoveCommand::Add(args) => add_content(cli.identity, args).await,
                    AddRemoveCommand::Remove(args) => remove_content(cli.identity, args).await,
                },
                Command::Comment(args) => match args.cmd {
                    AddRemoveCommand::Add(args) => add_comment(cli.identity, args).await,
                    AddRemoveCommand::Remove(args) => remove_comment(cli.identity, args).await,
                },
                //Command::Identity(args) => update_identity(args, ipns, signer).await,
                Command::Follow(args) => match args.cmd {
                    FollowCommand::Add(args) => add_followee(cli.identity, args).await,
                    FollowCommand::Remove(args) => remove_followee(cli.identity, args).await,
                },
                Command::Live(args) => update_live(cli.identity, args).await,
                Command::Moderation(args) => match args.cmd {
                    ModerationCommand::Ban(args) => ban_user(cli.identity, args).await,
                    ModerationCommand::Unban(args) => unban_user(cli.identity, args).await,
                    ModerationCommand::Mod(args) => mod_user(cli.identity, args).await,
                    ModerationCommand::Unmod(args) => unmod_user(cli.identity, args).await,
                },
            }
        }
    }; */

    let res = match cli.cmd {
        Command::Create => create_channel(cli.identity).await,
        Command::Content(args) => match args.cmd {
            AddRemoveCommand::Add(args) => add_content(cli.identity, args).await,
            AddRemoveCommand::Remove(args) => remove_content(cli.identity, args).await,
        },
        Command::Comment(args) => match args.cmd {
            AddRemoveCommand::Add(args) => add_comment(cli.identity, args).await,
            AddRemoveCommand::Remove(args) => remove_comment(cli.identity, args).await,
        },
        Command::Follow(args) => match args.cmd {
            FollowCommand::Add(args) => add_followee(cli.identity, args).await,
            FollowCommand::Remove(args) => remove_followee(cli.identity, args).await,
        },
        Command::Live(args) => update_live(cli.identity, args).await,
        Command::Moderation(args) => match args.cmd {
            ModerationCommand::Ban(args) => ban_user(cli.identity, args).await,
            ModerationCommand::Unban(args) => unban_user(cli.identity, args).await,
            ModerationCommand::Mod(args) => mod_user(cli.identity, args).await,
            ModerationCommand::Unmod(args) => unmod_user(cli.identity, args).await,
        },
    };

    if let Err(e) = res {
        eprintln!("❗ IPFS: {:#?}", e);
    }
}

async fn create_channel(identity: Cid) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    println!("Wait For Your Channel To Be Created...");

    let (channel, identity) = Channel::create_local(ipfs, identity).await?;

    println!(
        "✅ Created Identity {} With Channel {} Included",
        identity,
        channel.get_address()
    );

    Ok(())
}

#[derive(Debug, Parser)]
pub struct Manage {
    #[clap(subcommand)]
    cmd: AddRemoveCommand,
}

#[derive(Debug, Parser)]
enum AddRemoveCommand {
    /// Add content/comment to your channel.
    Add(Content),

    /// Remove content/comment from your channel.
    Remove(Content),
}

#[derive(Debug, Parser)]
pub struct Content {
    /// The CID of the content/comment.
    #[clap(short, long)]
    cid: Cid,
}

async fn local_setup(identity: Cid) -> Result<Channel<LocalUpdater>, Error> {
    let ipfs = IpfsService::default();

    let identity = ipfs.dag_get::<String, Identity>(identity, None).await?;
    let addr = identity.ipns_addr.expect("IPNS Address");
    let key = identity.name.to_snake_case();

    let updater = LocalUpdater::new(ipfs.clone(), key);
    let channel = Channel::new(ipfs, addr, updater);

    Ok(channel)
}

async fn add_content(identity: Cid, args: Content) -> Result<(), Error> {
    let channel = local_setup(identity).await?;

    println!("Wait For Your Channel To Add Content...");

    channel.add_content(args.cid).await?;

    println!("✅ Added Content {}", args.cid);

    Ok(())
}

async fn remove_content(identity: Cid, args: Content) -> Result<(), Error> {
    let channel = local_setup(identity).await?;

    println!("Wait For Your Channel To Remove Content...");

    channel.remove_content(args.cid).await?;

    println!("✅ Removed Content {}", args.cid);

    Ok(())
}

async fn add_comment(identity: Cid, args: Content) -> Result<(), Error> {
    let channel = local_setup(identity).await?;

    println!("Wait For Your Channel To Add Comment...");

    channel.add_comment(args.cid).await?;

    println!("✅ Added Comment {}", args.cid);

    Ok(())
}

async fn remove_comment(identity: Cid, args: Content) -> Result<(), Error> {
    let channel = local_setup(identity).await?;

    println!("Wait For Your Channel To Remove Comment.");

    channel.remove_comment(args.cid).await?;

    println!("✅ Removed Comment {}", args.cid);

    Ok(())
}

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
    address: IPNSAddress,
}

async fn add_followee(identity: Cid, args: Followee) -> Result<(), Error> {
    let channel = local_setup(identity).await?;

    println!("Wait For Your Channel To Add Followee...");

    channel.follow(args.address).await?;

    println!("✅ Added Followee {}", args.address);

    Ok(())
}

async fn remove_followee(identity: Cid, args: Followee) -> Result<(), Error> {
    let channel = local_setup(identity).await?;

    println!("Wait For Your Channel To Remove Followee...");

    channel.unfollow(args.address).await?;

    println!("✅ Removed Followee {}", args.address);

    Ok(())
}

#[derive(Debug, Parser)]
pub struct Live {
    /// Peer Id of the node live streaming.
    #[clap(short, long)]
    peer_id: Option<PeerId>,

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

async fn update_live(identity: Cid, args: Live) -> Result<(), Error> {
    let Live {
        peer_id,
        video_topic,
        chat_topic,
        archiving,
    } = args;

    let channel = local_setup(identity).await?;

    /* let peer_id = if let Some(peer) = peer_id {
        match PeerId::try_from(peer) {
            Ok(peer) => Some(peer.into()),
            Err(e) => {
                eprintln!("{}", e);

                None
            }
        }
    } else {
        None
    }; */

    println!("Wait For Your Channel To Update Live Settings...");

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
    Ban(EthAddress),

    /// Unban users.
    Unban(EthAddress),

    /// Promote user to moderator position.
    Mod(EthAddress),

    /// Demote user from moderator position.
    Unmod(EthAddress),
}

#[derive(Debug, Parser)]
pub struct EthAddress {
    /// Ethereum address.
    #[clap(long)]
    address: String,
}

async fn ban_user(identity: Cid, args: EthAddress) -> Result<(), Error> {
    let address = parse_address(&args.address);

    let channel = local_setup(identity).await?;

    println!("Wait For Your Channel To Ban A User...");

    if channel.ban_user(address).await?.is_some() {
        println!("✅ User {} Banned", args.address);

        return Ok(());
    }

    println!("❗ User {} was already banned", args.address);

    Ok(())
}

async fn unban_user(identity: Cid, args: EthAddress) -> Result<(), Error> {
    let address = parse_address(&args.address);

    let channel = local_setup(identity).await?;

    println!("Wait For Your Channel To Unban A User.");

    if channel.unban_user(&address).await?.is_some() {
        println!("✅ User {} Unbanned", args.address);

        return Ok(());
    }

    println!("❗ User {} was not banned", args.address);

    Ok(())
}

async fn mod_user(identity: Cid, args: EthAddress) -> Result<(), Error> {
    let address = parse_address(&args.address);

    let channel = local_setup(identity).await?;

    println!("Wait For Your Channel To Add A Moderator.");

    if channel.add_moderator(address).await?.is_some() {
        println!("✅ User {} Promoted To Moderator", args.address);

        return Ok(());
    }

    println!("❗ User {} was already banned", args.address);

    Ok(())
}

async fn unmod_user(identity: Cid, args: EthAddress) -> Result<(), Error> {
    let address = parse_address(&args.address);

    let channel = local_setup(identity).await?;

    println!("Wait For Your Channel To Remove A Moderator.");

    if channel.remove_moderator(&address).await?.is_some() {
        println!("✅ Moderator {} Demoted", args.address);

        return Ok(());
    }

    println!("❗ User {} Was Not A Moderator", args.address);

    Ok(())
}

fn parse_address(addrs: &str) -> [u8; 20] {
    use hex::FromHex;

    if let Some(end) = addrs.strip_prefix("0x") {
        return <[u8; 20]>::from_hex(end).expect("Invalid Ethereum Address");
    }

    <[u8; 20]>::from_hex(&addrs).expect("Invalid Ethereum Address")
}
