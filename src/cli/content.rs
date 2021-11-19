use std::{
    convert::TryFrom,
    path::{Path, PathBuf},
};

use crate::utils::{
    dag_nodes::{get_from_ipns, ipfs_dag_get_node_async, ipfs_dag_put_node_async, update_ipns},
    OPTIONS,
};

use serde::{de::DeserializeOwned, Serialize};

use ipfs_api::{response::Error, IpfsClient};

use linked_data::{
    blog::{FullPost, MicroPost},
    comments::Commentary,
    feed::{FeedAnchor, Media},
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
    let ipfs = IpfsClient::default();

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
    let ipfs = IpfsClient::default();

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
    let ipfs = IpfsClient::default();

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
    let ipfs = IpfsClient::default();

    let UpdateMicroPost { cid, content } = command;

    let (old_feed_cid, mut feed, mut metadata) = unload_feed::<MicroPost>(&ipfs, cid).await?;

    metadata.update(content);

    reload_feed(&ipfs, cid, &metadata, &mut feed).await?;

    let ofc = old_feed_cid.to_string();
    if let Err(e) = ipfs.pin_rm(&ofc, false).await {
        eprintln!("❗ IPFS could not unpin {}. Error: {}", ofc, e);
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
    let ipfs = IpfsClient::default();

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

    let ofc = old_feed_cid.to_string();
    if let Err(e) = ipfs.pin_rm(&ofc, false).await {
        eprintln!("❗ IPFS could not unpin {}. Error: {}", ofc, e);
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
    let ipfs = IpfsClient::default();

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

    let ofc = old_feed_cid.to_string();
    if let Err(e) = ipfs.pin_rm(&ofc, false).await {
        eprintln!("❗ IPFS could not unpin {}. Error: {}", ofc, e);
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
    let ipfs = IpfsClient::default();

    let DeleteContent { cid } = command;

    let ((old_feed_cid, mut feed), (old_comments_cid, mut list)) = tokio::try_join!(
        get_from_ipns::<FeedAnchor>(&ipfs, FEED_KEY),
        get_from_ipns::<Commentary>(&ipfs, COMMENTS_KEY)
    )?;

    let index = match feed.content.iter().position(|&probe| probe.link == cid) {
        Some(idx) => idx,
        None => return Err(Error::Uncategorized("Index Not Found".into())),
    };

    let content = feed.content.remove(index);

    if let Some(comments) = list.comments.remove(&content.link) {
        //TODO find a way to do that concurently
        for comment in comments.iter() {
            let cid = comment.link.to_string();

            if let Err(e) = ipfs.pin_rm(&cid, false).await {
                eprintln!("❗ IPFS could not unpin {}. Error: {}", cid, e);
            }
        }
    }

    let content_cid = content.link.to_string();
    let old_feed_cid = old_feed_cid.to_string();
    let old_comments_cid = old_comments_cid.to_string();

    tokio::try_join!(
        update_ipns(&ipfs, FEED_KEY, &feed),
        update_ipns(&ipfs, COMMENTS_KEY, &list)
    )?;

    if let Err(e) = ipfs.pin_rm(&content_cid, true).await {
        eprintln!("❗ IPFS could not unpin {}. Error: {}", content_cid, e);
    }

    if let Err(e) = ipfs.pin_rm(&old_feed_cid, false).await {
        eprintln!("❗ IPFS could not unpin {}. Error: {}", old_feed_cid, e);
    }

    if let Err(e) = ipfs.pin_rm(&old_comments_cid, false).await {
        eprintln!("❗ IPFS could not unpin {}. Error: {}", old_comments_cid, e);
    }

    println!("✅ Comments Cleared & Deleted Content {}", cid);

    Ok(())
}

async fn repair_content() -> Result<(), Error> {
    let ipfs = IpfsClient::default();

    if let Ok((old_feed_cid, _)) = get_from_ipns::<FeedAnchor>(&ipfs, FEED_KEY).await {
        println!("Unpinnig Old Content Feed...");

        let ofc = old_feed_cid.to_string();
        if let Err(e) = ipfs.pin_rm(&ofc, false).await {
            eprintln!("❗ IPFS could not unpin {}. Error: {}", ofc, e);
        }
    }

    println!("Searching...");
    let pins = ipfs.pin_ls(None, Some("recursive")).await?;

    let mut content = Vec::with_capacity(100);

    for cid in pins.keys.into_keys() {
        if let Ok(media) = ipfs_dag_get_node_async::<Media>(&ipfs, &cid).await {
            let cid = Cid::try_from(cid).expect("Invalid Cid");
            content.push((cid, media));
        }
    }

    println!("Found {} Media Objects", content.len());

    println!("Sorting...");
    content.sort_unstable_by_key(|(_, media)| media.timestamp());

    let content = content.into_iter().map(|(cid, _)| cid.into()).collect();

    let content_feed = FeedAnchor { content };

    println!("Updating Content Feed...");
    update_ipns(&ipfs, FEED_KEY, &content_feed).await?;

    println!("✅ Repaired Content Feed");

    Ok(())
}

/*** Utils below ****/

async fn add_image(ipfs: &IpfsClient, path: &Path) -> Result<Cid, Error> {
    let mime_type = match MimeGuess::from_path(path).first_raw() {
        Some(mime) => mime.to_owned(),
        None => return Err(Error::Uncategorized("Cannot Guess Image Mime Type".into())),
    };

    #[cfg(debug_assertions)]
    println!("Image Mime Type => {}", &mime_type);

    let file = tokio::fs::File::open(path).await?;
    let response = ipfs.add_with_options(file, OPTIONS).await?;
    let cid = Cid::try_from(response.hash).expect("Cid Validation");

    let mime_typed = MimeTyped {
        mime_type,
        data: cid.into(),
    };

    ipfs_dag_put_node_async(ipfs, &mime_typed).await
}

async fn add_markdown(ipfs: &IpfsClient, path: &Path) -> Result<Cid, Error> {
    if path.extension().is_none() || path.extension().unwrap() != "md" {
        return Err(Error::Uncategorized("Markdown file only".into()));
    };

    let file = tokio::fs::File::open(path).await?;
    let response = ipfs.add_with_options(file, OPTIONS).await?;
    let cid = Cid::try_from(response.hash).expect("Cid Validation");

    Ok(cid)
}

/// Serialize and pin content then update IPNS.
async fn add_content_to_feed<T>(ipfs: &IpfsClient, metadata: &T) -> Result<Cid, Error>
where
    T: Serialize,
{
    println!("Creating...");

    let content_cid = ipfs_dag_put_node_async(ipfs, metadata).await?;

    println!("Pinning...");
    if let Err(e) = ipfs.pin_add(&content_cid.to_string(), true).await {
        eprintln!(
            "❗ IPFS could not pin {}. Error: {}",
            content_cid.to_string(),
            e
        );
    }

    println!("Updating Content Feed...");
    let (old_feed_cid, mut feed) = get_from_ipns::<FeedAnchor>(ipfs, FEED_KEY).await?;

    feed.content.push(content_cid.into());

    update_ipns(ipfs, FEED_KEY, &feed).await?;

    let ofc = old_feed_cid.to_string();
    if let Err(e) = ipfs.pin_rm(&ofc, false).await {
        eprintln!("❗ IPFS could not unpin {}. Error: {}", ofc, e);
    }

    Ok(content_cid)
}

/// Unpin then return feed and cid.
async fn unload_feed<T>(ipfs: &IpfsClient, cid: Cid) -> Result<(Cid, FeedAnchor, T), Error>
where
    T: DeserializeOwned,
{
    println!("Old Content => {}", cid);

    let (old_feed_cid, feed) = get_from_ipns::<FeedAnchor>(ipfs, FEED_KEY).await?;

    println!("Unpinning...");
    let cid = cid.to_string();
    if let Err(e) = ipfs.pin_rm(&cid, true).await {
        eprintln!("❗ IPFS could not unpin {}. Error: {}", cid, e);
    }

    let metadata: T = ipfs_dag_get_node_async(ipfs, &cid).await?;

    Ok((old_feed_cid, feed, metadata))
}

/// Serialize and pin metadata then update feed and update IPNS.
async fn reload_feed<T>(
    ipfs: &IpfsClient,
    cid: Cid,
    metadata: &T,
    feed: &mut FeedAnchor,
) -> Result<(), Error>
where
    T: Serialize,
{
    let new_cid = ipfs_dag_put_node_async(ipfs, metadata).await?;
    println!("New Content => {}", new_cid);

    println!("Pinning...");
    if let Err(e) = ipfs.pin_add(&new_cid.to_string(), true).await {
        eprintln!(
            "❗ IPFS could not pin {}. Error: {}",
            new_cid.to_string(),
            e
        );
    }

    println!("Updating Content Feed...");

    let idx = match feed.content.iter().position(|&probe| probe.link == cid) {
        Some(idx) => idx,
        None => return Err(Error::Uncategorized("Index Not Found".into())),
    };

    feed.content[idx] = new_cid.into();

    update_ipns(ipfs, FEED_KEY, feed).await?;

    Ok(())
}

async fn get_video_duration(ipfs: &IpfsClient, video: &Cid) -> Result<f64, Error> {
    let path = format!("{}/time", video.to_string());

    let days: DayNode = ipfs_dag_get_node_async(ipfs, &path).await?;

    let mut duration = 0.0;

    for (i, ipld) in days.links_to_hours.iter().enumerate().rev().take(1) {
        duration += (i * 3600) as f64; // 3600 second in 1 hour

        let hours: HourNode = ipfs_dag_get_node_async(ipfs, &ipld.link.to_string()).await?;

        for (i, ipld) in hours.links_to_minutes.iter().enumerate().rev().take(1) {
            duration += (i * 60) as f64; // 60 second in 1 minute

            let minutes: MinuteNode = ipfs_dag_get_node_async(ipfs, &ipld.link.to_string()).await?;

            duration += (minutes.links_to_seconds.len() - 1) as f64;
        }
    }

    Ok(duration)
}
