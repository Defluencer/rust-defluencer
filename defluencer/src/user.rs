use std::borrow::Cow;

use crate::{
    crypto::{signed_link::SignedLink, signers::Signer},
    errors::Error,
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

#[derive(Clone)]
pub struct User<T>
where
    T: Signer + Clone,
{
    ipfs: IpfsService,
    identity: IPLDLink,
    signer: T,
}

impl<T> PartialEq for User<T>
where
    T: Signer + Clone,
{
    fn eq(&self, other: &Self) -> bool {
        self.identity == other.identity
    }
}

impl<T> User<T>
where
    T: Signer + Clone,
{
    pub fn new(ipfs: IpfsService, signer: T, identity: Cid) -> Self {
        Self {
            ipfs,
            signer,
            identity: identity.into(),
        }
    }

    pub fn get_identity(&self) -> Cid {
        self.identity.link
    }

    /// Create a new user.
    pub async fn create(
        &self,
        user_name: impl Into<Cow<'static, str>>,
        ipfs: IpfsService,
        signer: T,
        channel_ipns: Option<IPNSAddress>,
    ) -> Result<Self, Error> {
        let identity = Identity {
            display_name: user_name.into().into_owned(),
            avatar: None,
            channel_ipns,
            addr: None,
        };

        let identity = ipfs.dag_put(&identity, Codec::default()).await?.into();

        let user = Self {
            ipfs,
            signer,
            identity,
        };

        Ok(user)
    }

    /// Update your identity data.
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn update_identity(
        mut self,
        display_name: Option<String>,
        avatar: Option<std::path::PathBuf>,
        channel_ipns: Option<IPNSAddress>,
    ) -> Result<Cid, Error> {
        let mut identity = self
            .ipfs
            .dag_get::<&str, Identity>(self.identity.link, None)
            .await?;

        if let Some(name) = display_name {
            identity.display_name = name;
        }

        if let Some(avatar) = avatar {
            identity.avatar = Some(add_image(&self.ipfs, &avatar).await?.into());
        }

        if let Some(ipns) = channel_ipns {
            identity.channel_ipns = Some(ipns);
        }

        let cid = self.ipfs.dag_put(&identity, Codec::default()).await?;

        self.identity = cid.into();

        Ok(cid)
    }

    /// Update your identity data.
    #[cfg(target_arch = "wasm32")]
    pub async fn update_identity(
        &mut self,
        display_name: Option<String>,
        avatar: Option<web_sys::File>,
        channel_ipns: Option<IPNSAddress>,
    ) -> Result<Cid, Error> {
        let mut identity = self
            .ipfs
            .dag_get::<&str, Identity>(self.identity.link, None)
            .await?;

        if let Some(name) = display_name {
            identity.display_name = name;
        }

        if let Some(avatar) = avatar {
            identity.avatar = Some(add_image(&self.ipfs, avatar).await?.into());
        }

        if let Some(ipns) = channel_ipns {
            identity.channel_ipns = Some(ipns);
        }

        let cid = self.ipfs.dag_put(&identity, Codec::default()).await?;

        self.identity = cid.into();

        Ok(cid)
    }

    /// Create a new micro blog post.
    pub async fn create_micro_blog_post(
        &self,
        content: String,
        pin: bool,
    ) -> Result<(Cid, MicroPost), Error> {
        let micro_post = MicroPost {
            identity: self.identity,
            content,
            user_timestamp: Utc::now().timestamp(),
        };

        let cid = self.add_content(&micro_post, pin).await?;

        Ok((cid, micro_post))
    }

    /// Create a new blog post.
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn create_blog_post(
        &self,
        title: String,
        image: &std::path::Path,
        markdown: &std::path::Path,
        pin: bool,
    ) -> Result<(Cid, FullPost), Error> {
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

        let cid = self.add_content(&full_post, pin).await?;

        Ok((cid, full_post))
    }

    /// Create a new blog post.
    #[cfg(target_arch = "wasm32")]
    pub async fn create_blog_post(
        &self,
        title: String,
        image: web_sys::File,
        markdown: web_sys::File,
        pin: bool,
    ) -> Result<(Cid, FullPost), Error> {
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

        let cid = self.add_content(&full_post, pin).await?;

        Ok((cid, full_post))
    }

    /// Create a new video post.
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn create_video_post(
        &self,
        title: String,
        video: Cid,
        thumbnail: &std::path::Path,
        pin: bool,
    ) -> Result<(Cid, Video), Error> {
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

        let cid = self.add_content(&video_post, pin).await?;

        Ok((cid, video_post))
    }

    /// Create a new video post.
    #[cfg(target_arch = "wasm32")]
    pub async fn create_video_post(
        &self,
        title: String,
        video: Cid,
        thumbnail: web_sys::File,
        pin: bool,
    ) -> Result<(Cid, Video), Error> {
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

        let cid = self.add_content(&video_post, pin).await?;

        Ok((cid, video_post))
    }

    /// Create a new comment on the specified media.
    pub async fn create_comment(
        &self,
        origin: Cid,
        text: String,
        pin: bool,
    ) -> Result<(Cid, Comment), Error> {
        let comment = Comment {
            identity: self.identity,
            user_timestamp: Utc::now().timestamp(),
            origin,
            text,
        };

        let cid = self.add_content(&comment, pin).await?;

        Ok((cid, comment))
    }

    /// Returns the CID of the signed block linking to the content
    async fn add_content<V>(&self, metadata: &V, pin: bool) -> Result<Cid, Error>
    where
        V: ?Sized + Serialize,
    {
        let content_cid = self.ipfs.dag_put(metadata, Codec::default()).await?;

        let signed_cid = self.create_signed_link(content_cid).await?;

        if pin {
            self.ipfs.pin_add(signed_cid, true).await?;
        }

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

    /// Returns a DAG-JOSE block CID used to authenticate chat message.
    ///
    /// Message will only be valid when sent by this IPFS node.
    pub async fn chat_signature(&self) -> Result<Cid, Error> {
        let peer = self.ipfs.peer_id().await?;

        let cid = self.create_signed_link(peer).await?;

        Ok(cid)
    }

    async fn create_signed_link(&self, cid: Cid) -> Result<Cid, Error> {
        use k256::elliptic_curve::sec1::ToEncodedPoint;

        let (verif_key, signature, hash_algo) = self.signer.sign(cid.hash().digest()).await?;

        let signed_link = SignedLink {
            link: cid.into(),
            public_key: verif_key.to_encoded_point(false).as_bytes().to_vec(),
            hash_algo,
            signature: signature.to_der().as_bytes().to_vec(),
        };

        let cid = self.ipfs.dag_put(&signed_link, Default::default()).await?;

        Ok(cid)
    }
}
