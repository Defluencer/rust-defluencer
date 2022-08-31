use std::path::PathBuf;

use cid::Cid;

use clap::Parser;

use defluencer::{
    errors::Error,
    signatures::{
        bitcoin::BitcoinSigner,
        ethereum::EthereumSigner,
        ledger::{BitcoinLedgerApp, EthereumLedgerApp},
        Signer,
    },
    user::User,
    utils::add_image,
};

use ipfs_api::{responses::Codec, IpfsService};

use linked_data::{identity::Identity, types::IPNSAddress};

use heck::ToSnakeCase;

#[derive(Debug, Parser)]
pub struct UserCLI {
    /// Bitcoin or Ethereum based signatures.
    #[clap(arg_enum, default_value = "bitcoin")]
    blockchain: Blockchain,

    /// Account index (BIP-44).
    #[clap(long, default_value = "0")]
    account: u32,

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
    /// Create a new user identity.
    Create(Create),

    /// Create new content.
    Content(Content),
}

pub async fn user_cli(cli: UserCLI) {
    let res = match cli.blockchain {
        Blockchain::Bitcoin => {
            let app = BitcoinLedgerApp::default();

            let signer = BitcoinSigner::new(app, cli.account);

            let addr = signer.get_public_address()?;

            match cli.cmd {
                Command::Create(args) => create_user(args, addr).await,
                Command::Content(content) => match content.cmd {
                    Media::Microblog(args) => micro_blog(content.creator, args, signer).await,
                    Media::Blog(args) => blog(content.creator, args, signer).await,
                    Media::Video(args) => video(content.creator, args, signer).await,
                    Media::Comment(args) => comment(content.creator, args, signer).await,
                },
            }
        }
        Blockchain::Ethereum => {
            let app = EthereumLedgerApp::default();

            let signer = EthereumSigner::new(app, cli.account);

            let addr = signer.get_public_address()?;

            match cli.cmd {
                Command::Create(args) => create_user(args, addr).await,
                Command::Content(content) => match content.cmd {
                    Media::Microblog(args) => micro_blog(content.creator, args, signer).await,
                    Media::Blog(args) => blog(content.creator, args, signer).await,
                    Media::Video(args) => video(content.creator, args, signer).await,
                    Media::Comment(args) => comment(content.creator, args, signer).await,
                },
            }
        }
    };

    if let Err(e) = res {
        eprintln!("❗ IPFS: {:#?}", e);
    }
}

#[derive(Debug, Parser)]
pub struct Create {
    /// Display name.
    #[clap(short, long)]
    display_name: String,

    /// Path to avatar image file.
    #[clap(short, long)]
    path: Option<PathBuf>,

    /// Create Channel Too?
    #[clap(short, long)]
    channel: bool,
}

async fn create_user(args: Create, addr: String) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let channel_ipns = if args.channel {
        let key = args.display_name.to_snake_case();
        let key_pair = ipfs.key_gen(key.clone()).await?;

        let ipns = IPNSAddress::try_from(key_pair.id.as_str())?;

        Some(ipns)
    } else {
        None
    };

    let avatar = if let Some(path) = args.path {
        Some(add_image(&ipfs, &path).await?.into())
    } else {
        None
    };

    let identity = Identity {
        display_name: args.display_name,
        avatar,
        channel_ipns,
        addr: Some(addr),
    };

    let cid = ipfs.dag_put(&identity, Codec::default()).await?;

    println!("✅ User Identity Created\nCID: {}", cid);

    Ok(())
}

#[derive(Debug, Parser)]
pub struct Content {
    /// Creators identity CID
    #[clap(short, long)]
    creator: Cid,

    #[clap(subcommand)]
    cmd: Media,
}

#[derive(Debug, Parser)]
enum Media {
    /// Create new micro post.
    Microblog(MicroBlog),

    /// Create new blog post.
    Blog(Blog),

    /// Create new video post.
    Video(Video),

    /// Create new comment.
    Comment(Comment),
}

#[derive(Debug, Parser)]
pub struct MicroBlog {
    /// The micro post text content.
    #[clap(short, long)]
    content: String,
}

async fn micro_blog(
    identity: Cid,
    args: MicroBlog,
    signer: impl Signer + Clone,
) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let user = User::new(ipfs, signer, identity);

    println!("Confirm Signature On Your Hardware Wallet...");

    let cid = user.create_micro_blog_post(args.content).await?;

    println!("✅ Created Micro Blog Post\nCID: {}", cid);

    Ok(())
}

#[derive(Debug, Parser)]
pub struct Blog {
    /// The blog post title.
    #[clap(long)]
    title: String,

    /// Path to the thumbnail image.
    #[clap(long, parse(from_os_str))]
    image: PathBuf,

    /// Path to the markdown file.
    #[clap(long, parse(from_os_str))]
    content: PathBuf,
}

async fn blog(identity: Cid, args: Blog, signer: impl Signer + Clone) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let Blog {
        title,
        image,
        content,
    } = args;

    let user = User::new(ipfs, signer, identity);

    println!("Confirm Signature On Your Hardware Wallet...");

    let cid = user.create_blog_post(title, &image, &content).await?;

    println!("✅ Created Blog Post\nCID: {}", cid);

    Ok(())
}

#[derive(Debug, Parser)]
pub struct Video {
    /// The new video title.
    #[clap(long)]
    title: String,

    /// Path to the video thumbnail image.
    #[clap(long, parse(from_os_str))]
    image: PathBuf,

    /// Processed video timecode CID.
    #[clap(long)]
    video: Cid,
}

async fn video(identity: Cid, args: Video, signer: impl Signer + Clone) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let Video {
        title,
        image,
        video,
    } = args;

    let user = User::new(ipfs, signer, identity);

    println!("Confirm Signature On Your Hardware Wallet...");

    let cid = user.create_video_post(title, video, &image).await?;

    println!("✅ Created Video\nCID: {}", cid);

    Ok(())
}

#[derive(Debug, Parser)]
pub struct Comment {
    /// Origin CID AKA the media being commented on.
    #[clap(long)]
    origin: Cid,

    /// The comment text.
    #[clap(short, long)]
    content: String,
}

async fn comment(identity: Cid, args: Comment, signer: impl Signer + Clone) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let user = User::new(ipfs, signer, identity);

    println!("Confirm Signature On Your Hardware Wallet...");

    let cid = user.create_comment(args.origin, args.content).await?;

    println!("✅ Created Comment\nCID: {}", cid);

    Ok(())
}
