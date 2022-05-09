use std::borrow::Cow;

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
    signature::{AlgorithmType, Header, JsonWebKey, RawJWS, RawSignature},
    types::{IPLDLink, IPNSAddress},
};

use multibase::Base;

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

    pub async fn create(
        &self,
        user_name: impl Into<Cow<'static, str>>,
        ipfs: IpfsService,
        signer: T,
    ) -> Result<Self, Error> {
        let identity = Identity {
            display_name: user_name.into().into_owned(),
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

        let signed_cid = self.create_dag_jose(content_cid).await?;

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

    /// Returns a DAG-JOSE block CID used to authenticate chat message.
    ///
    /// Message will only be valid when sent by this IPFS node.
    pub async fn chat_signature(&self) -> Result<Cid, Error> {
        let peer = self.ipfs.peer_id().await?;

        let cid = self.create_dag_jose(peer).await?;

        Ok(cid)
    }

    async fn create_dag_jose(&self, cid: Cid) -> Result<Cid, Error> {
        let payload = cid.to_bytes();
        let payload = Base::Base64Url.encode(payload);

        let protected = Header {
            algorithm: Some(AlgorithmType::ES256K),
            json_web_key: None,
        };

        let protected = serde_json::to_vec(&protected)?;
        let protected = Base::Base64Url.encode(protected);

        let message = format!("{}.{}", payload, protected);

        let (public_key, signature) = self.signer.sign(message.into_bytes()).await?;

        // Lazy Hack: Deserialize then serialize as the other type
        let jwk_string = public_key.to_jwk_string();
        let jwk: JsonWebKey = serde_json::from_str(&jwk_string)?;

        let header = Some(Header {
            algorithm: None,
            json_web_key: Some(jwk),
        });

        let signature = Base::Base64Url.encode(signature);

        let json = RawJWS {
            payload,
            signatures: vec![RawSignature {
                header,
                protected,
                signature,
            }],
            link: cid.into(), // ignored when serializing
        };

        let cid = self.ipfs.dag_put(&json, Codec::DagJose).await?;

        Ok(cid)
    }
}
