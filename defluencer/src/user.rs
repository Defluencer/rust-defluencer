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
        blog::BlogPost,
        chat::ChatInfo,
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
        ipns_addr: Option<IPNSAddress>,
    ) -> Result<Self, Error> {
        let identity = Identity {
            name: user_name.into().into_owned(),
            ipns_addr,
            ..Default::default()
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
        name: Option<String>,
        bio: Option<String>,
        banner: Option<std::path::PathBuf>,
        avatar: Option<std::path::PathBuf>,
        ipns_addr: Option<IPNSAddress>,
        btc_addr: Option<String>,
        eth_addr: Option<String>,
    ) -> Result<(Cid, Identity), Error> {
        let mut identity = self
            .ipfs
            .dag_get::<&str, Identity>(self.identity.link, None)
            .await?;

        if let Some(name) = name {
            identity.name = name;
        }

        if let Some(bio) = bio {
            identity.bio = Some(bio);
        }

        if let Some(banner) = banner {
            identity.banner = Some(add_image(&self.ipfs, &banner).await?.into());
        }

        if let Some(avatar) = avatar {
            identity.avatar = Some(add_image(&self.ipfs, &avatar).await?.into());
        }

        if let Some(ipns) = ipns_addr {
            identity.ipns_addr = Some(ipns);
        }

        if let Some(btc_addr) = btc_addr {
            identity.btc_addr = Some(btc_addr);
        }

        if let Some(eth_addr) = eth_addr {
            identity.eth_addr = Some(eth_addr);
        }

        let cid = self.ipfs.dag_put(&identity, Codec::default()).await?;

        self.identity = cid.into();

        Ok((cid, identity))
    }

    /// Update your identity data.
    #[cfg(target_arch = "wasm32")]
    pub async fn update_identity(
        &mut self,
        name: Option<String>,
        bio: Option<String>,
        banner: Option<web_sys::File>,
        avatar: Option<web_sys::File>,
        ipns_addr: Option<IPNSAddress>,
        btc_addr: Option<String>,
        eth_addr: Option<String>,
    ) -> Result<(Cid, Identity), Error> {
        let mut identity = self
            .ipfs
            .dag_get::<&str, Identity>(self.identity.link, None)
            .await?;

        if let Some(name) = name {
            identity.name = name;
        }

        if let Some(bio) = bio {
            identity.bio = Some(bio);
        }

        if let Some(banner) = banner {
            identity.banner = Some(add_image(&self.ipfs, banner).await?.into());
        }

        if let Some(avatar) = avatar {
            identity.avatar = Some(add_image(&self.ipfs, avatar).await?.into());
        }

        if let Some(ipns) = ipns_addr {
            identity.ipns_addr = Some(ipns);
        }

        if let Some(btc_addr) = btc_addr {
            identity.btc_addr = Some(btc_addr);
        }

        if let Some(eth_addr) = eth_addr {
            identity.eth_addr = Some(eth_addr);
        }

        let cid = self.ipfs.dag_put(&identity, Codec::default()).await?;

        self.identity = cid.into();

        Ok((cid, identity))
    }

    /// Create a new micro blog post.
    pub async fn create_micro_blog_post(
        &self,
        text: String,
        pin: bool,
    ) -> Result<(Cid, Comment), Error> {
        let micro_post = Comment {
            identity: self.identity,
            text,
            user_timestamp: Utc::now().timestamp(),
            origin: None,
        };

        let cid = self.add_content(&micro_post, pin).await?;

        Ok((cid, micro_post))
    }

    /// Create a new blog post.
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn create_blog_post(
        &self,
        title: String,
        image: Option<&std::path::Path>,
        markdown: &std::path::Path,
        word_count: Option<u64>,
        pin: bool,
    ) -> Result<(Cid, BlogPost), Error> {
        let (image, content) = match image {
            Some(image) => {
                let (image, markdown) = tokio::try_join!(
                    add_image(&self.ipfs, image),
                    add_markdown(&self.ipfs, markdown)
                )?;

                (Some(image.into()), markdown.into())
            }
            None => {
                let markdown = add_markdown(&self.ipfs, markdown).await?;

                (None, markdown.into())
            }
        };

        let post = BlogPost {
            identity: self.identity,
            user_timestamp: Utc::now().timestamp(),
            content,
            image,
            title,
            word_count,
        };

        let cid = self.add_content(&post, pin).await?;

        Ok((cid, post))
    }

    /// Create a new blog post.
    #[cfg(target_arch = "wasm32")]
    pub async fn create_blog_post(
        &self,
        title: String,
        image: Option<web_sys::File>,
        markdown: web_sys::File,
        word_count: Option<u64>,
        pin: bool,
    ) -> Result<(Cid, BlogPost), Error> {
        let (image, content) = match image {
            Some(image) => {
                let (image, markdown) = futures::try_join!(
                    add_image(&self.ipfs, image),
                    add_markdown(&self.ipfs, markdown)
                )?;

                (Some(image.into()), markdown.into())
            }
            None => {
                let markdown = add_markdown(&self.ipfs, markdown).await?;

                (None, markdown.into())
            }
        };

        let post = BlogPost {
            identity: self.identity,
            user_timestamp: Utc::now().timestamp(),
            content,
            image,
            title,
            word_count,
        };

        let cid = self.add_content(&post, pin).await?;

        Ok((cid, post))
    }

    /// Create a new video post.
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn create_video_post(
        &self,
        title: String,
        video: Cid,
        thumbnail: Option<&std::path::Path>,
        pin: bool,
    ) -> Result<(Cid, Video), Error> {
        let (image, duration) = match thumbnail {
            Some(img) => {
                let (img, dur) =
                    tokio::try_join!(add_image(&self.ipfs, img), self.video_duration(video))?;

                (Some(img.into()), Some(dur))
            }
            None => {
                let duration = self.video_duration(video).await?;

                (None, Some(duration))
            }
        };

        let video_post = Video {
            identity: self.identity,
            user_timestamp: Utc::now().timestamp(),
            image,
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
        thumbnail: Option<web_sys::File>,
        pin: bool,
    ) -> Result<(Cid, Video), Error> {
        let (image, duration) = match thumbnail {
            Some(img) => {
                let (img, dur) =
                    futures::try_join!(add_image(&self.ipfs, img), self.video_duration(video))?;

                (Some(img.into()), Some(dur))
            }
            None => {
                let duration = self.video_duration(video).await?;

                (None, Some(duration))
            }
        };

        let video_post = Video {
            identity: self.identity,
            user_timestamp: Utc::now().timestamp(),
            image,
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
            origin: Some(origin),
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
    pub async fn chat_signature(&self, chat_info: ChatInfo) -> Result<Cid, Error> {
        let cid = self.ipfs.dag_put(&chat_info, Codec::default()).await?;

        let cid = self.create_signed_link(cid).await?;

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
