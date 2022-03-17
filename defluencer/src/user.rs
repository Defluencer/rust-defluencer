use crate::{errors::Error, signatures::Signer};

use chrono::Utc;

use cid::Cid;

use ipfs_api::{responses::Codec, IpfsService};

use linked_data::{
    comments::Comment,
    media::{
        blog::{FullPost, MicroPost},
        mime_type::MimeTyped,
        video::{DayNode, HourNode, MinuteNode, VideoMetadata},
    },
};

use serde::Serialize;

type MarkdownCid = Cid;
type ImageCid = Cid;
type VideoCid = Cid;

#[derive(Clone)]
pub struct Channel<T>
where
    T: Signer,
{
    signer: T,
    ipfs: IpfsService,
}

impl<T> Channel<T>
where
    T: Signer,
{
    /// Create a new micro blog post.
    pub async fn create_micro_blog_post(&self, content: String) -> Result<Cid, Error> {
        let date_time = Utc::now();
        let timestamp = date_time.timestamp();

        let micro_post = MicroPost { timestamp, content };

        self.add_content(&micro_post, true).await
    }

    /// Create a new blog post.
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn create_blog_post(
        &self,
        title: String,
        image: &std::path::Path,
        markdown: &std::path::Path,
    ) -> Result<Cid, Error> {
        let (image, markdown) =
            tokio::try_join!(self.add_image(image), self.add_markdown(markdown))?;

        let date_time = Utc::now();
        let timestamp = date_time.timestamp();

        let full_post = FullPost {
            timestamp,
            content: markdown.into(),
            image: image.into(),
            title,
        };

        self.add_content(&full_post, true).await
    }

    /// Create a new blog post.
    #[cfg(target_arch = "wasm32")]
    pub async fn create_blog_post(
        &self,
        title: String,
        image: web_sys::File,
        markdown: web_sys::File,
    ) -> Result<Cid, Error> {
        let (image, markdown) =
            futures::try_join!(self.add_image(image), self.add_markdown(markdown))?;

        let date_time = Utc::now();
        let timestamp = date_time.timestamp();

        let full_post = FullPost {
            timestamp,
            content: markdown.into(),
            image: image.into(),
            title,
        };

        self.add_content(&full_post, true).await
    }

    /// Create a new video post.
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn create_video_post(
        &self,
        title: String,
        video: VideoCid,
        thumbnail: &std::path::Path,
    ) -> Result<Cid, Error> {
        let (image, duration) =
            tokio::try_join!(self.add_image(thumbnail), self.video_duration(video))?;

        let date_time = Utc::now();
        let timestamp = date_time.timestamp();

        let video_post = VideoMetadata {
            timestamp,
            image: image.into(),
            title,
            duration,
            video: video.into(),
        };

        self.add_content(&video_post, true).await
    }

    /// Create a new video post.
    #[cfg(target_arch = "wasm32")]
    pub async fn create_video_post(
        &self,
        title: String,
        video: VideoCid,
        thumbnail: web_sys::File,
    ) -> Result<Cid, Error> {
        let (image, duration) =
            futures::try_join!(self.add_image(thumbnail), self.video_duration(video))?;

        let date_time = Utc::now();
        let timestamp = date_time.timestamp();

        let video_post = VideoMetadata {
            timestamp,
            image: image.into(),
            title,
            duration,
            video: video.into(),
        };

        self.add_content(&video_post, true).await
    }

    /// Create a new comment on the specified media.
    pub async fn create_comment(&self, origin: Cid, text: String) -> Result<Cid, Error> {
        let comment = Comment {
            timestamp: Utc::now().timestamp(),
            origin: origin.into(),
            text,
        };

        self.add_content(&comment, false).await
    }

    async fn add_content<V>(&self, metadata: &V, pin: bool) -> Result<Cid, Error>
    where
        V: ?Sized + Serialize,
    {
        let content_cid = self.ipfs.dag_put(metadata, Codec::default()).await?;

        let signed_cid = self.signer.sign(content_cid).await?;

        self.ipfs.pin_add(signed_cid, pin).await?;

        Ok(signed_cid)
    }

    async fn video_duration(&self, video: Cid) -> Result<f64, Error> {
        let days: DayNode = self.ipfs.dag_get(video, Some("/time")).await?;

        let mut duration = 0.0;

        for (i, ipld) in days.links_to_hours.iter().enumerate().rev().take(1) {
            duration += (i * 3600) as f64; // 3600 second in 1 hour

            let hours: HourNode = self.ipfs.dag_get(ipld.link, Option::<&str>::None).await?;

            for (i, ipld) in hours.links_to_minutes.iter().enumerate().rev().take(1) {
                duration += (i * 60) as f64; // 60 second in 1 minute

                let minutes: MinuteNode =
                    self.ipfs.dag_get(ipld.link, Option::<&str>::None).await?;

                duration += (minutes.links_to_seconds.len() - 1) as f64;
            }
        }

        Ok(duration)
    }

    /// Add an image to IPFS and return the CID
    #[cfg(not(target_arch = "wasm32"))]
    async fn add_image(&self, path: &std::path::Path) -> Result<ImageCid, Error> {
        let mime_type = match mime_guess::MimeGuess::from_path(path).first_raw() {
            Some(mime) => mime.to_owned(),
            None => return Err(Error::Image),
        };

        if !(mime_type == "image/png" || mime_type == "image/jpeg") {
            return Err(Error::Image);
        };

        let file = tokio::fs::File::open(path).await?;
        let stream = tokio_util::io::ReaderStream::new(file);
        let cid = self.ipfs.add(stream).await?;

        let mime_typed = MimeTyped {
            mime_type,
            data: either::Either::Left(cid.into()),
        };

        let cid = self.ipfs.dag_put(&mime_typed, Codec::default()).await?;

        Ok(cid)
    }

    /// Add a markdown file to IPFS and return the CID
    #[cfg(not(target_arch = "wasm32"))]
    async fn add_markdown(&self, path: &std::path::Path) -> Result<MarkdownCid, Error> {
        let mime_type = match mime_guess::MimeGuess::from_path(path).first_raw() {
            Some(mime) => mime.to_owned(),
            None => return Err(Error::Markdown),
        };

        if mime_type != "text/markdown" {
            return Err(Error::Markdown);
        };

        let file = tokio::fs::File::open(path).await?;
        let stream = tokio_util::io::ReaderStream::new(file);

        let cid = self.ipfs.add(stream).await?;

        Ok(cid)
    }

    /// Add an image to IPFS and return the CID
    #[cfg(target_arch = "wasm32")]
    async fn add_image(&self, file: web_sys::File) -> Result<ImageCid, Error> {
        use futures::AsyncReadExt;
        use wasm_bindgen::JsCast;

        let mime_type = file.type_();

        if !(mime_type == "image/png" || mime_type == "image/jpeg") {
            return Err(Error::Image);
        };

        let size = file.size() as usize;

        // TODO disallow image that are too big.

        let readable_stream =
            wasm_streams::ReadableStream::from_raw(file.stream().unchecked_into());

        let mut async_read = readable_stream.into_async_read();

        let mut bytes = Vec::with_capacity(size);
        async_read.read_to_end(&mut bytes).await?;

        let mime_typed = MimeTyped {
            mime_type,
            data: either::Either::Right(bytes),
        };

        let cid = self.ipfs.dag_put(&mime_typed, Codec::default()).await?;

        Ok(cid)
    }

    /// Add a markdown file to IPFS and return the CID
    #[cfg(target_arch = "wasm32")]
    async fn add_markdown(&self, file: web_sys::File) -> Result<MarkdownCid, Error> {
        use futures::AsyncReadExt;
        use wasm_bindgen::JsCast;

        if file.type_() != "text/markdown" {
            return Err(Error::Markdown);
        };

        let size = file.size() as usize;

        let readable_stream =
            wasm_streams::ReadableStream::from_raw(file.stream().unchecked_into());

        let mut async_read = readable_stream.into_async_read();

        let mut bytes = Vec::with_capacity(size);
        async_read.read_to_end(&mut bytes).await?;
        let bytes = bytes::Bytes::from(bytes);

        let cid = self.ipfs.add(bytes).await?;

        Ok(cid)
    }
}
