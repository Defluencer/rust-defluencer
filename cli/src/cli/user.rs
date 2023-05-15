use std::path::PathBuf;

use cid::Cid;

use clap::{Parser, Subcommand};

use defluencer::{
    crypto::{
        ledger::{BitcoinLedgerApp, EthereumLedgerApp},
        signers::BitcoinSigner,
        signers::EthereumSigner,
        signers::Signer,
    },
    errors::Error,
    user::User,
};

use ipfs_api::{responses::Codec, IpfsService};

use linked_data::identity::Identity;

#[derive(clap::ValueEnum, Clone, Debug)]
enum Blockchain {
    Bitcoin,
    Ethereum,
}

#[derive(Debug, Parser)]
pub struct UserCLI {
    /// Bitcoin or Ethereum based signatures.
    #[arg(value_enum, default_value = "bitcoin")]
    blockchain: Blockchain,

    /// Account index (BIP-44).
    #[arg(long, default_value = "0")]
    account: u32,

    /// Creators identity CID
    #[arg(long)]
    creator: Cid,

    #[command(subcommand)]
    cmd: Media,
}

pub async fn user_cli(cli: UserCLI) {
    let res = match cli.blockchain {
        Blockchain::Bitcoin => {
            let app = BitcoinLedgerApp::default();

            let signer = BitcoinSigner::new(app, cli.account);

            let addr = match signer.get_public_address() {
                Ok(addr) => addr,
                Err(e) => {
                    eprintln!("❗ Wallet: {:#?}", e);
                    return;
                }
            };

            match cli.cmd {
                Media::Microblog(args) => micro_blog(args, cli.creator, addr, signer).await,
                Media::Blog(args) => blog(args, cli.creator, addr, signer).await,
                Media::Video(args) => video(args, cli.creator, addr, signer).await,
            }
        }
        Blockchain::Ethereum => {
            let app = EthereumLedgerApp::default();

            let signer = EthereumSigner::new(app, cli.account);

            let addr = match signer.get_public_address() {
                Ok(addr) => addr,
                Err(e) => {
                    eprintln!("❗ Wallet: {:#?}", e);
                    return;
                }
            };

            match cli.cmd {
                Media::Microblog(args) => micro_blog(args, cli.creator, addr, signer).await,
                Media::Blog(args) => blog(args, cli.creator, addr, signer).await,
                Media::Video(args) => video(args, cli.creator, addr, signer).await,
            }
        }
    };

    if let Err(e) = res {
        eprintln!("❗ IPFS: {:#?}", e);
    }
}

#[derive(Debug, Subcommand)]
enum Media {
    /// Create new micro post.
    Microblog(MicroBlog),

    /// Create new blog post.
    Blog(Blog),

    /// Create new video post.
    Video(Video),
}

#[derive(Debug, Parser)]
pub struct MicroBlog {
    /// The micro post text content.
    #[arg(long)]
    content: String,

    /// Cid of the media being commented on. (Optional)
    #[arg(long)]
    origin: Option<Cid>,
}

async fn micro_blog(
    args: MicroBlog,
    identity: Cid,
    addr: String,
    signer: impl Signer + Clone,
) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let id = ipfs
        .dag_get::<&str, Identity>(identity, None, Codec::default())
        .await?;

    let addr = Some(addr);
    if id.eth_addr != addr && id.btc_addr != addr {
        eprintln!("❗ Wallet address mismatch.");
        return Ok(());
    }

    let user = User::new(ipfs, signer, identity);

    println!("Confirm Signature...");

    let (cid, _) = user
        .create_micro_blog_post(args.content, args.origin, false)
        .await?;

    println!("✅ Created Micro Blog Post\nCID: {}", cid);

    Ok(())
}

#[derive(Debug, Parser)]
pub struct Blog {
    /// The blog post title.
    #[arg(long)]
    title: String,

    /// Path to the markdown file.
    #[arg(long)]
    content: PathBuf,

    /// Path to the thumbnail image. (Optional)
    #[arg(long)]
    image: Option<PathBuf>,

    /// Total word count. (Optional)
    #[arg(long)]
    word_count: Option<u64>,
}

async fn blog(
    args: Blog,
    identity: Cid,
    addr: String,
    signer: impl Signer + Clone,
) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let id = ipfs
        .dag_get::<&str, Identity>(identity, None, Codec::default())
        .await?;

    let addr = Some(addr);
    if id.eth_addr != addr && id.btc_addr != addr {
        eprintln!("❗ Wallet address mismatch.");
        return Ok(());
    }

    let Blog {
        title,
        image,
        content,
        word_count,
    } = args;

    let user = User::new(ipfs, signer, identity);

    println!("Confirm Signature...");

    let (cid, _) = user
        .create_blog_post(title, image, content, word_count, false)
        .await?;

    println!("✅ Created Blog Post\nCID: {}", cid);

    Ok(())
}

#[derive(Debug, Parser)]
pub struct Video {
    /// The new video title.
    #[arg(long)]
    title: String,

    /// Path to the video thumbnail image. (Optional)
    #[arg(long)]
    image: Option<PathBuf>,

    /// Processed video timecode CID.
    #[arg(long)]
    video: Cid,
}

async fn video(
    args: Video,
    identity: Cid,
    addr: String,
    signer: impl Signer + Clone,
) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let id = ipfs
        .dag_get::<&str, Identity>(identity, None, Codec::default())
        .await?;

    let addr = Some(addr);
    if id.eth_addr != addr && id.btc_addr != addr {
        eprintln!("❗ Wallet address mismatch.");
        return Ok(());
    }

    let Video {
        title,
        image,
        video,
    } = args;

    let user = User::new(ipfs, signer, identity);

    println!("Confirm Signature...");

    let (cid, _) = user.create_video_post(title, video, image, false).await?;

    println!("✅ Created Video\nCID: {}", cid);

    Ok(())
}
