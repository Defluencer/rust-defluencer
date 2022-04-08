use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
};

use tokio_util::io::ReaderStream;

use serde::{de::DeserializeOwned, Serialize};

use ipfs_api::{errors::Error, responses::PinMode, IpfsService};

use linked_data::{
    blog::{FullPost, MicroPost},
    comments::Commentary,
    content::{FeedAnchor, Media},
    mime_type::MimeTyped,
    video::{DayNode, HourNode, MinuteNode, VideoMetadata},
};

use mime_guess::MimeGuess;

use cid::Cid;

use structopt::StructOpt;

pub const FEED_KEY: &str = "feed";
pub const COMMENTS_KEY: &str = "comments";

#[derive(Debug, StructOpt)]
pub struct Content {
    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(Debug, StructOpt)]
enum Command {
    /// Publish new content to your feed.
    Add(AddContent),

    /// Update content on your feed. Will clear all comments.
    Update(UpdateContent),

    /// Delete content from your feed.
    Delete(DeleteContent),

    /// Search for pinned media objects, order them chronologicaly then recreate content feed.
    Repair,
}

pub async fn content_feed_cli(cli: Content) {
    let res = match cli.cmd {
        Command::Add(add) => match add {
            AddContent::MicroBlog(blog) => add_micro_blog(blog).await,
            AddContent::Blog(blog) => add_blog(blog).await,
            AddContent::Video(video) => add_video(video).await,
        },
        Command::Update(update) => match update {
            UpdateContent::MicroBlog(blog) => update_micro_blog(blog).await,
            UpdateContent::Blog(blog) => update_blog(blog).await,
            UpdateContent::Video(video) => update_video(video).await,
        },
        Command::Delete(delete) => delete_content(delete).await,
        Command::Repair => repair_content().await,
    };

    if let Err(e) = res {
        eprintln!("❗ IPFS: {:#?}", e);
    }
}

#[derive(Debug, StructOpt)]
enum AddContent {
    /// Create new micro post.
    MicroBlog(AddMicroPost),

    /// Create new blog post.
    Blog(AddPost),

    /// Create new video post.
    Video(AddVideo),
}

#[derive(Debug, StructOpt)]
pub struct AddMicroPost {
    /// The micro post content.
    #[structopt(short, long)]
    content: String,
}

async fn add_micro_blog(command: AddMicroPost) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let AddMicroPost { content } = command;

    let metadata = MicroPost::create(content);

    let cid = add_content_to_feed(&ipfs, &metadata).await?;

    println!("✅ Added Weblog {}", cid);

    Ok(())
}

#[derive(Debug, StructOpt)]
pub struct AddPost {
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

async fn add_blog(command: AddPost) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let AddPost {
        title,
        image,
        content,
    } = command;

    let image = add_image(&ipfs, &image).await?;

    #[cfg(debug_assertions)]
    println!("Image Cid => {:?}", &image);

    let content = add_markdown(&ipfs, &content).await?;

    #[cfg(debug_assertions)]
    println!("Markdown Cid => {:?}", &image);

    let metadata = FullPost::create(title, image, content);

    let cid = add_content_to_feed(&ipfs, &metadata).await?;

    println!("✅ Added Weblog {}", cid);

    Ok(())
}

#[derive(Debug, StructOpt)]
pub struct AddVideo {
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

async fn add_video(command: AddVideo) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let AddVideo {
        title,
        image,
        video,
    } = command;

    let image = add_image(&ipfs, &image).await?;

    let duration = get_video_duration(&ipfs, &video).await?;
    let metadata = VideoMetadata::create(title, duration, image, video);

    let cid = add_content_to_feed(&ipfs, &metadata).await?;

    println!("✅ Added Video {}", cid);

    Ok(())
}

#[derive(Debug, StructOpt)]
enum UpdateContent {
    /// Update micro blog post.
    MicroBlog(UpdateMicroPost),

    /// Update blog post.
    Blog(UpdatePost),

    /// Update video post.
    Video(UpdateVideo),
}

#[derive(Debug, StructOpt)]
pub struct UpdateMicroPost {
    /// CID of the post to update.
    #[structopt(long)]
    cid: Cid,

    /// The new content.
    #[structopt(short, long)]
    content: String,
}

async fn update_micro_blog(command: UpdateMicroPost) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let UpdateMicroPost { cid, content } = command;

    let (old_feed_cid, mut feed, mut metadata) = unload_feed::<MicroPost>(&ipfs, cid).await?;

    metadata.update(content);

    reload_feed(&ipfs, cid, &metadata, &mut feed).await?;

    if let Err(e) = ipfs.pin_rm(&old_feed_cid, false).await {
        eprintln!("❗ IPFS could not unpin {}. Error: {}", old_feed_cid, e);
    }

    println!("✅ Comments Cleared & Updated Weblog");

    Ok(())
}

#[derive(Debug, StructOpt)]
pub struct UpdatePost {
    /// CID of the post to update.
    #[structopt(long)]
    cid: Cid,

    /// The new title.
    #[structopt(long)]
    title: Option<String>,

    /// Path to the new thumbnail image.
    #[structopt(long, parse(from_os_str))]
    image: Option<PathBuf>,

    /// Path to the new makdown file.
    #[structopt(long, parse(from_os_str))]
    content: Option<PathBuf>,
}

async fn update_blog(command: UpdatePost) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let UpdatePost {
        cid,
        title,
        image,
        content,
    } = command;

    let (old_feed_cid, mut feed, mut metadata) = unload_feed::<FullPost>(&ipfs, cid).await?;

    let image = if let Some(image) = image {
        Some(add_image(&ipfs, &image).await?)
    } else {
        None
    };

    let content = if let Some(content) = content {
        Some(add_markdown(&ipfs, &content).await?)
    } else {
        None
    };

    metadata.update(title, image, content);

    reload_feed(&ipfs, cid, &metadata, &mut feed).await?;

    if let Err(e) = ipfs.pin_rm(&old_feed_cid, false).await {
        eprintln!("❗ IPFS could not unpin {}. Error: {}", old_feed_cid, e);
    }

    println!("✅ Comments Cleared & Updated Weblog");

    Ok(())
}

#[derive(Debug, StructOpt)]
pub struct UpdateVideo {
    /// CID of the video to update.
    #[structopt(long)]
    cid: Cid,

    /// The new video title.
    #[structopt(long)]
    title: Option<String>,

    /// Path to the new video thumbnail image.
    #[structopt(long, parse(from_os_str))]
    image: Option<PathBuf>,

    /// The new processed video timecode CID.
    #[structopt(long)]
    video: Option<Cid>,
}

async fn update_video(command: UpdateVideo) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let UpdateVideo {
        cid,
        title,
        image,
        video,
    } = command;

    let (old_feed_cid, mut feed, mut metadata) = unload_feed::<VideoMetadata>(&ipfs, cid).await?;

    let duration = match video {
        Some(cid) => Some(get_video_duration(&ipfs, &cid).await?),
        None => None,
    };

    let image = if let Some(image) = image {
        Some(add_image(&ipfs, &image).await?)
    } else {
        None
    };

    metadata.update(title, image, video, duration);

    reload_feed(&ipfs, cid, &metadata, &mut feed).await?;

    if let Err(e) = ipfs.pin_rm(&old_feed_cid, false).await {
        eprintln!("❗ IPFS could not unpin {}. Error: {}", old_feed_cid, e);
    }

    println!("✅ Comments Cleared & Updated Video");

    Ok(())
}

#[derive(Debug, StructOpt)]
pub struct DeleteContent {
    /// The CID of the content to delete.
    /// Will also delete your comments.
    #[structopt(short, long)]
    cid: Cid,
}

async fn delete_content(command: DeleteContent) -> Result<(), Error> {
    println!("Deleting Content...");
    let ipfs = IpfsService::default();

    let DeleteContent { cid } = command;

    let (feed_res, com_res) =
        tokio::try_join!(ipfs.ipns_get(FEED_KEY), ipfs.ipns_get(COMMENTS_KEY))?;

    let (old_feed_cid, mut feed): (Cid, FeedAnchor) = feed_res.unwrap();
    let (old_comments_cid, mut list): (Cid, Commentary) = com_res.unwrap();

    let index = match feed.content.iter().position(|&probe| probe.link == cid) {
        Some(idx) => idx,
        None => return Err(std::io::Error::from(ErrorKind::NotFound).into()),
    };

    let content = feed.content.remove(index);

    if let Some(comments) = list.comments.remove(&content.link) {
        //TODO find a way to do that concurently
        for comment in comments.iter() {
            if let Err(e) = ipfs.pin_rm(&comment.link, false).await {
                eprintln!("❗ IPFS could not unpin {}. Error: {}", comment.link, e);
            }
        }
    }

    tokio::try_join!(
        ipfs.ipns_put(FEED_KEY, false, &feed),
        ipfs.ipns_put(COMMENTS_KEY, false, &list)
    )?;

    ipfs.pin_rm(&content.link, true).await?;
    ipfs.pin_rm(&old_feed_cid, false).await?;
    ipfs.pin_rm(&old_comments_cid, false).await?;

    println!("✅ Comments Cleared & Deleted Content {}", cid);

    Ok(())
}

async fn repair_content() -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let res: Option<(Cid, FeedAnchor)> = ipfs.ipns_get(FEED_KEY).await?;

    if let Some((old_feed_cid, _)) = res {
        println!("Unpinnig Old Content Feed...");

        if let Err(e) = ipfs.pin_rm(&old_feed_cid, false).await {
            eprintln!("❗ IPFS could not unpin {}. Error: {}", old_feed_cid, e);
        }
    }

    println!("Searching...");
    let pins = ipfs.pin_ls(PinMode::Recursive).await?;

    let mut content: Vec<(Cid, Media)> = Vec::with_capacity(pins.len());

    for cid in pins.into_keys() {
        if let Ok(media) = ipfs.dag_get(&cid, Option::<&str>::None).await {
            content.push((cid, media));
        }
    }

    println!("Found {} Media Objects", content.len());

    println!("Sorting...");
    content.sort_unstable_by_key(|(_, media)| media.timestamp());

    let content = content.into_iter().map(|(cid, _)| cid.into()).collect();

    let content_feed = FeedAnchor { content };

    println!("Updating Content Feed...");
    ipfs.ipns_put(FEED_KEY, false, &content_feed).await?;

    println!("✅ Repaired Content Feed");

    Ok(())
}

/*** Utils below ****/

async fn add_image(ipfs: &IpfsService, path: &Path) -> Result<Cid, Error> {
    let mime_type = match MimeGuess::from_path(path).first_raw() {
        Some(mime) => mime.to_owned(),
        None => return Err(std::io::Error::from(ErrorKind::InvalidInput).into()),
    };

    #[cfg(debug_assertions)]
    println!("Image Mime Type => {}", &mime_type);

    let file = tokio::fs::File::open(path).await?;
    let stream = ReaderStream::new(file);
    let cid = ipfs.add(stream).await?;

    let mime_typed = MimeTyped {
        mime_type,
        data: cid.into(),
    };

    ipfs.dag_put(&mime_typed).await
}

async fn add_markdown(ipfs: &IpfsService, path: &Path) -> Result<Cid, Error> {
    if path.extension().is_none() || path.extension().unwrap() != "md" {
        return Err(std::io::Error::from(ErrorKind::InvalidInput).into());
    };

    let file = tokio::fs::File::open(path).await?;
    let stream = ReaderStream::new(file);

    let cid = ipfs.add(stream).await?;

    Ok(cid)
}

/// Serialize and pin content then update IPNS.
async fn add_content_to_feed<T>(ipfs: &IpfsService, metadata: &T) -> Result<Cid, Error>
where
    T: Serialize,
{
    println!("Creating...");

    let content_cid = ipfs.dag_put(metadata).await?;

    println!("Pinning...");
    if let Err(e) = ipfs.pin_add(&content_cid, true).await {
        eprintln!("❗ IPFS could not pin {}. Error: {}", content_cid, e);
    }

    println!("Updating Content Feed...");
    let res = ipfs.ipns_get(FEED_KEY).await?;
    let (old_feed_cid, mut feed): (Cid, FeedAnchor) = res.unwrap();

    feed.content.push(content_cid.into());

    ipfs.ipns_put(FEED_KEY, false, &feed).await?;

    if let Err(e) = ipfs.pin_rm(&old_feed_cid, false).await {
        eprintln!("❗ IPFS could not unpin {}. Error: {}", old_feed_cid, e);
    }

    Ok(content_cid)
}

/// Unpin then return feed and cid.
async fn unload_feed<T>(ipfs: &IpfsService, cid: Cid) -> Result<(Cid, FeedAnchor, T), Error>
where
    T: DeserializeOwned,
{
    println!("Old Content => {}", cid);

    let res = ipfs.ipns_get(FEED_KEY).await?;
    let (old_feed_cid, feed) = res.unwrap();

    println!("Unpinning...");
    if let Err(e) = ipfs.pin_rm(&cid, true).await {
        eprintln!("❗ IPFS could not unpin {}. Error: {}", cid, e);
    }

    let metadata: T = ipfs.dag_get(&cid, Option::<&str>::None).await?;

    Ok((old_feed_cid, feed, metadata))
}

/// Serialize and pin metadata then update feed and update IPNS.
async fn reload_feed<T>(
    ipfs: &IpfsService,
    cid: Cid,
    metadata: &T,
    feed: &mut FeedAnchor,
) -> Result<(), Error>
where
    T: Serialize,
{
    let new_cid = ipfs.dag_put(metadata).await?;
    println!("New Content => {}", new_cid);

    println!("Pinning...");
    if let Err(e) = ipfs.pin_add(&new_cid, true).await {
        eprintln!("❗ IPFS could not pin {}. Error: {}", new_cid, e);
    }

    println!("Updating Content Feed...");

    let idx = match feed.content.iter().position(|&probe| probe.link == cid) {
        Some(idx) => idx,
        None => return Err(std::io::Error::from(ErrorKind::NotFound).into()),
    };

    feed.content[idx] = new_cid.into();

    ipfs.ipns_put(FEED_KEY, false, feed).await?;

    Ok(())
}

async fn get_video_duration(ipfs: &IpfsService, video: &Cid) -> Result<f64, Error> {
    let days: DayNode = ipfs.dag_get(video, Some("/time")).await?;

    let mut duration = 0.0;

    for (i, ipld) in days.links_to_hours.iter().enumerate().rev().take(1) {
        duration += (i * 3600) as f64; // 3600 second in 1 hour

        let hours: HourNode = ipfs.dag_get(&ipld.link, Option::<&str>::None).await?;

        for (i, ipld) in hours.links_to_minutes.iter().enumerate().rev().take(1) {
            duration += (i * 60) as f64; // 60 second in 1 minute

            let minutes: MinuteNode = ipfs.dag_get(&ipld.link, Option::<&str>::None).await?;

            duration += (minutes.links_to_seconds.len() - 1) as f64;
        }
    }

    Ok(duration)
}
