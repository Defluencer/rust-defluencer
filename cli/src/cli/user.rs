use std::path::PathBuf;

use cid::Cid;

use clap::Parser;

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

use ipfs_api::IpfsService;

use linked_data::identity::Identity;

#[derive(clap::ValueEnum, Clone, Debug)]
enum Blockchain {
    Bitcoin,
    Ethereum,
}

#[derive(Debug, Parser)]
pub struct UserCLI {
    /// Bitcoin or Ethereum based signatures.
    #[clap(value_enum, default_value = "bitcoin")]
    blockchain: Blockchain,

    /// Account index (BIP-44).
    #[clap(long, default_value = "0")]
    account: u32,

    /// Creators identity CID
    #[clap(short, long)]
    creator: Cid,

    #[clap(subcommand)]
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

#[derive(Debug, Parser)]
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
    #[clap(short, long)]
    content: String,

    /// Cid of the media being commented on.
    #[clap(short, long)]
    origin: Option<Cid>,
}

async fn micro_blog(
    args: MicroBlog,
    identity: Cid,
    addr: String,
    signer: impl Signer + Clone,
) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let id = ipfs.dag_get::<&str, Identity>(identity, None).await?;

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
    #[clap(long)]
    title: String,

    /// Path to the markdown file.
    #[clap(short, long)]
    content: PathBuf,

    /// Path to the thumbnail image.
    #[clap(short, long)]
    image: Option<PathBuf>,

    /// Total word count.
    #[clap(short, long)]
    word_count: Option<u64>,
}

async fn blog(
    args: Blog,
    identity: Cid,
    addr: String,
    signer: impl Signer + Clone,
) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let id = ipfs.dag_get::<&str, Identity>(identity, None).await?;

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
    #[clap(long)]
    title: String,

    /// Path to the video thumbnail image.
    #[clap(short, long)]
    image: Option<PathBuf>,

    /// Processed video timecode CID.
    #[clap(short, long)]
    video: Cid,
}

async fn video(
    args: Video,
    identity: Cid,
    addr: String,
    signer: impl Signer + Clone,
) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let id = ipfs.dag_get::<&str, Identity>(identity, None).await?;

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

    println!("Confirm Signature On Your Hardware Wallet...");

    let (cid, _) = user.create_video_post(title, video, image, false).await?;

    println!("✅ Created Video\nCID: {}", cid);

    Ok(())
}
