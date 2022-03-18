use crate::{
    errors::Error,
    signatures::Signer,
    utils::{add_image, add_markdown},
};

use chrono::Utc;

use cid::Cid;

use ipfs_api::{responses::Codec, IpfsService};

use linked_data::{
    comments::Comment,
    media::{
        blog::{FullPost, MicroPost},
        video::{DayNode, HourNode, MinuteNode, VideoMetadata},
    },
};

use serde::Serialize;

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
        let (image, markdown) = tokio::try_join!(
            add_image(&self.ipfs, image),
            add_markdown(&self.ipfs, markdown)
        )?;

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
        let (image, markdown) = futures::try_join!(
            add_image(&self.ipfs, image),
            add_markdown(&self.ipfs, markdown)
        )?;

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
        video: Cid,
        thumbnail: &std::path::Path,
    ) -> Result<Cid, Error> {
        let (image, duration) =
            tokio::try_join!(add_image(&self.ipfs, thumbnail), self.video_duration(video))?;

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
        video: Cid,
        thumbnail: web_sys::File,
    ) -> Result<Cid, Error> {
        let (image, duration) =
            futures::try_join!(add_image(&self.ipfs, thumbnail), self.video_duration(video))?;

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
}
