use std::path::PathBuf;

use cid::Cid;

use defluencer::{errors::Error, signatures::TestSigner, user::User, Defluencer};
use ipfs_api::IpfsService;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Content {
    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(Debug, StructOpt)]
enum Command {
    /// Create new media content.
    Create(Create),

    /// Remove media content from your channel.
    Remove(Remove),
}

#[derive(Debug, StructOpt)]
pub struct Create {
    /// Creators identity CID
    #[structopt(short, long)]
    identity: Cid,

    #[structopt(subcommand)]
    cmd: Media,
}

#[derive(Debug, StructOpt)]
enum Media {
    /// Create new micro post.
    MicroBlog(MicroBlog),

    /// Create new blog post.
    Blog(Blog),

    /// Create new video post.
    Video(Video),

    /// Create new comment.
    Comment(Comment),
}

#[derive(Debug, StructOpt)]
pub struct Remove {
    /// Channel local key name.
    #[structopt(short, long)]
    key_name: String,

    /// The CID of the content to remove.
    /// Will also delete your comments.
    #[structopt(short, long)]
    cid: Cid,
}

pub async fn content_cli(cli: Content) {
    let res = match cli.cmd {
        Command::Create(create) => match create.cmd {
            Media::MicroBlog(args) => micro_blog(create.identity, args).await,
            Media::Blog(args) => blog(create.identity, args).await,
            Media::Video(args) => video(create.identity, args).await,
            Media::Comment(args) => comment(create.identity, args).await,
        },
        Command::Remove(remove) => delete(remove.key_name, remove.cid).await,
    };

    if let Err(e) = res {
        eprintln!("❗ IPFS: {:#?}", e);
    }
}

#[derive(Debug, StructOpt)]
pub struct MicroBlog {
    /// The micro post content.
    #[structopt(short, long)]
    content: String,
}

async fn micro_blog(identity: Cid, args: MicroBlog) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let MicroBlog { content } = args;

    let signer = TestSigner::default();

    let user = User::new(ipfs, signer, identity);

    let cid = user.create_micro_blog_post(content).await?;

    println!("✅ Added Micro Blog Post {}", cid);

    Ok(())
}

#[derive(Debug, StructOpt)]
pub struct Blog {
    /// The blog post title.
    #[structopt(long)]
    title: String,

    /// Path to the thumbnail image.
    #[structopt(long, parse(from_os_str))]
    image: PathBuf,

    /// Path to the markdown file.
    #[structopt(long, parse(from_os_str))]
    content: PathBuf,
}

async fn blog(identity: Cid, args: Blog) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let Blog {
        title,
        image,
        content,
    } = args;

    let signer = TestSigner::default();

    let user = User::new(ipfs, signer, identity);

    let cid = user.create_blog_post(title, &image, &content).await?;

    println!("✅ Added Blog Post {}", cid);

    Ok(())
}

#[derive(Debug, StructOpt)]
pub struct Video {
    /// The new video title.
    #[structopt(long)]
    title: String,

    /// Path to the video thumbnail image.
    #[structopt(long, parse(from_os_str))]
    image: PathBuf,

    /// Processed video timecode CID.
    #[structopt(long)]
    video: Cid,
}

async fn video(identity: Cid, args: Video) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let Video {
        title,
        image,
        video,
    } = args;

    let signer = TestSigner::default();

    let user = User::new(ipfs, signer, identity);

    let cid = user.create_video_post(title, video, &image).await?;

    println!("✅ Added Video {}", cid);

    Ok(())
}

#[derive(Debug, StructOpt)]
pub struct Comment {
    /// Comment origin AKA the media being commented on.
    #[structopt(long)]
    origin: Cid,

    /// The comment content.
    #[structopt(short, long)]
    content: String,
}

async fn comment(identity: Cid, args: Comment) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let Comment { origin, content } = args;

    let signer = TestSigner::default();

    let user = User::new(ipfs, signer, identity);

    let cid = user.create_comment(origin, content).await?;

    println!("✅ Added Comment {}", cid);

    Ok(())
}

async fn delete(key: String, content_cid: Cid) -> Result<(), Error> {
    let defluencer = Defluencer::default();

    if let Some(channel) = defluencer.get_local_channel(key).await? {
        channel.remove_content(content_cid).await?;

        println!("✅ Comments Cleared & Removed Content {}", content_cid);
    }

    Ok(())
}
