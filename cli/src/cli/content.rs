use std::path::PathBuf;

use cid::Cid;

use defluencer::{channel::Channel, errors::Error, signatures::TestSigner, user::User, Defluencer};

use futures_util::pin_mut;

use ipfs_api::IpfsService;

use linked_data::channel::ChannelMetadata;

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

    /// Stream a channel's content.
    Stream(Stream),

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
    /// Channel IPNS Address.
    #[structopt(short, long)]
    address: Cid,

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
        Command::Stream(args) => stream(args).await,
        Command::Remove(remove) => delete(remove.address, remove.cid).await,
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

    let signer = TestSigner::default(); // TODO

    let user = User::new(ipfs, signer, identity);

    let cid = user.create_micro_blog_post(args.content).await?;

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

    let signer = TestSigner::default(); // TODO

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

    let signer = TestSigner::default(); // TODO

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

    /// The comment text.
    #[structopt(short, long)]
    content: String,
}

async fn comment(identity: Cid, args: Comment) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let signer = TestSigner::default(); // TODO

    let user = User::new(ipfs, signer, identity);

    let cid = user.create_comment(args.origin, args.content).await?;

    println!("✅ Added Comment {}", cid);

    Ok(())
}

#[derive(Debug, StructOpt)]
pub struct Stream {
    /// Channel IPNS Address.
    #[structopt(short, long)]
    address: Cid,
}

async fn stream(args: Stream) -> Result<(), Error> {
    use futures_util::TryStreamExt;

    let ipfs = IpfsService::default();
    let defluencer = Defluencer::default();

    let cid = ipfs.name_resolve(args.address.into()).await?;

    let metadata = ipfs.dag_get::<&str, ChannelMetadata>(cid, None).await?;

    let index = match metadata.content_index {
        Some(ipns) => ipns,
        None => {
            eprintln!("❗ This channel has no content.");
            return Ok(());
        }
    };

    let stream = defluencer.stream_content_chronologically(index);
    pin_mut!(stream);

    while let Some(cid) = stream.try_next().await? {
        println!("{}", cid);
    }

    println!("✅ Content Stream End");

    Ok(())
}

async fn delete(addr: Cid, content_cid: Cid) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let signer = TestSigner::default(); //TODO

    let channel = Channel::new(ipfs, addr.into(), signer);

    channel.remove_content(content_cid).await?;

    println!("✅ Comments Cleared & Removed Content {}", content_cid);

    Ok(())
}
