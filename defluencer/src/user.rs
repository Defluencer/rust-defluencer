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
    identity::Identity,
    media::{
        blog::{FullPost, MicroPost},
        video::{Day, Hour, Minute, Video},
    },
    types::{IPLDLink, IPNSAddress},
};

use serde::Serialize;

pub struct User<T>
where
    T: Signer,
{
    signer: T,
    ipfs: IpfsService,
    identity: IPLDLink,
}

impl<T> User<T>
where
    T: Signer,
{
    pub fn new(ipfs: IpfsService, signer: T, identity: Cid) -> Self {
        Self {
            ipfs,
            signer,
            identity: identity.into(),
        }
    }

    /// Update your identity data.
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn update_identity(
        &mut self,
        display_name: Option<String>,
        avatar: Option<&std::path::Path>,
        channel_ipns: Option<IPNSAddress>,
        channel_ens: Option<String>,
    ) -> Result<(), Error> {
        let mut identity = self
            .ipfs
            .dag_get::<&str, Identity>(self.identity.link, None)
            .await?;

        if let Some(name) = display_name {
            identity.display_name = name;
        }

        if let Some(avatar) = avatar {
            identity.avatar = add_image(&self.ipfs, avatar).await?.into();
        }

        if let Some(ipns) = channel_ipns {
            identity.channel_ipns = Some(ipns);
        }

        if let Some(ens) = channel_ens {
            identity.channel_ens = Some(ens);
        }

        let cid = self.ipfs.dag_put(&identity, Codec::default()).await?;

        self.identity = cid.into();

        Ok(())
    }

    /// Update your identity data.
    #[cfg(target_arch = "wasm32")]
    pub async fn update_identity(
        &mut self,
        display_name: Option<String>,
        avatar: Option<web_sys::File>,
        channel_ipns: Option<IPNSAddress>,
        channel_ens: Option<String>,
    ) -> Result<(), Error> {
        let mut identity = self
            .ipfs
            .dag_get::<&str, Identity>(self.identity.link, None)
            .await?;

        if let Some(name) = display_name {
            identity.display_name = name;
        }

        if let Some(avatar) = avatar {
            identity.avatar = add_image(&self.ipfs, avatar).await?.into();
        }

        if let Some(ipns) = channel_ipns {
            identity.channel_ipns = Some(ipns);
        }

        if let Some(ens) = channel_ens {
            identity.channel_ens = Some(ens);
        }

        let cid = self.ipfs.dag_put(&identity, Codec::default()).await?;

        self.identity = cid.into();

        Ok(())
    }

    /// Create a new micro blog post.
    pub async fn create_micro_blog_post(&self, content: String) -> Result<Cid, Error> {
        let micro_post = MicroPost {
            identity: self.identity,
            content,
            user_timestamp: Utc::now().timestamp(),
        };

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

        let full_post = FullPost {
            identity: self.identity,
            user_timestamp: Utc::now().timestamp(),
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

        let full_post = FullPost {
            identity: self.identity,
            user_timestamp: Utc::now().timestamp(),
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

        let video_post = Video {
            identity: self.identity,
            user_timestamp: Utc::now().timestamp(),
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

        let video_post = Video {
            identity: self.identity,
            user_timestamp: Utc::now().timestamp(),
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
            identity: self.identity,
            user_timestamp: Utc::now().timestamp(),
            origin,
            text,
        };

        self.add_content(&comment, true).await
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
        let days: Day = self.ipfs.dag_get(video, Some("/time")).await?;

        let mut duration = 0.0;

        for (i, ipld) in days.links_to_hours.iter().enumerate().rev().take(1) {
            duration += (i * 3600) as f64; // 3600 second in 1 hour

            let hours: Hour = self.ipfs.dag_get(ipld.link, Option::<&str>::None).await?;

            for (i, ipld) in hours.links_to_minutes.iter().enumerate().rev().take(1) {
                duration += (i * 60) as f64; // 60 second in 1 minute

                let minutes: Minute = self.ipfs.dag_get(ipld.link, Option::<&str>::None).await?;

                duration += (minutes.links_to_seconds.len() - 1) as f64;
            }
        }

        Ok(duration)
    }

    // TODO live chat
    // When sending chat message create a DAG-JOSE block, the link is the peer id of the chatter.
    // Verifying the signature once is enough since the cid of the DAG-JOSE block can't change
}
