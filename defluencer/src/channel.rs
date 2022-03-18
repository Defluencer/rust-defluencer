use crate::{
    anchors::Anchor,
    errors::Error,
    utils::{add_image, get_path},
};

use chrono::{DateTime, Datelike, TimeZone, Timelike, Utc};

use cid::Cid;

use either::Either;

use ipfs_api::{responses::Codec, IpfsService};

use linked_data::{
    channel::{ChannelMetadata, Indexing},
    comments::{Comment, Comments},
    content::{Content, Media},
    indexes::date_time::*,
    moderation::{Bans, Moderators},
    Address, PeerId,
};

#[derive(Clone)]
pub struct Channel<T>
where
    T: Anchor + Clone,
{
    anchor: T,
    ipfs: IpfsService,
}

impl<T> Channel<T>
where
    T: Anchor + Clone,
{
    pub fn new(ipfs: IpfsService, anchor: T) -> Self {
        Self { ipfs, anchor }
    }

    /// Update your identity data.
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn update_identity(
        &self,
        display_name: Option<String>,
        avatar: Option<&std::path::Path>,
    ) -> Result<(), Error> {
        let (channel_cid, mut channel) = self.get_channel().await?;

        let mut identity = channel.identity.unwrap_or_default();

        if let Some(name) = display_name {
            identity.display_name = name;
        }

        if let Some(avatar) = avatar {
            identity.avatar = add_image(&self.ipfs, avatar).await?.into();
        }

        channel.identity = Some(identity);

        self.update_channel(channel_cid, &channel).await
    }

    /// Update your identity data.
    #[cfg(target_arch = "wasm32")]
    pub async fn update_identity(
        &self,
        display_name: Option<String>,
        avatar: Option<web_sys::File>,
    ) -> Result<(), Error> {
        let (channel_cid, mut channel) = self.get_channel().await?;

        let mut identity = channel.identity.unwrap_or_default();

        if let Some(name) = display_name {
            identity.display_name = name;
        }

        if let Some(avatar) = avatar {
            identity.avatar = add_image(&self.ipfs, avatar).await?.into();
        }

        channel.identity = Some(identity);

        self.update_channel(channel_cid, &channel).await
    }

    /// Follow a channel.
    pub async fn follow(&self, user: Either<String, Cid>) -> Result<(), Error> {
        let (channel_cid, mut channel) = self.get_channel().await?;

        let mut follows = channel.follows.unwrap_or_default();

        let status = match user {
            Either::Left(ens) => follows.ens.insert(ens),
            Either::Right(ipns) => follows.ipns.insert(ipns),
        };

        if !status {
            return Err(Error::AlreadyAdded);
        }

        channel.follows = Some(follows);

        self.update_channel(channel_cid, &channel).await
    }

    /// Unfollow a channel.
    pub async fn unfollow(&self, user: Either<String, Cid>) -> Result<(), Error> {
        let (channel_cid, mut channel) = self.get_channel().await?;

        let mut follows = match channel.follows {
            Some(f) => f,
            None => return Err(Error::NotFound),
        };

        let status = match user {
            Either::Left(ens) => follows.ens.remove(&ens),
            Either::Right(ipns) => follows.ipns.remove(&ipns),
        };

        if !status {
            return Err(Error::NotFound);
        }

        channel.follows = Some(follows);

        self.update_channel(channel_cid, &channel).await
    }

    /// Update live chat & streaming settings.
    pub async fn update_live_settings(
        &self,
        peer_id: Option<PeerId>,
        video_topic: Option<String>,
        chat_topic: Option<String>,
    ) -> Result<(), Error> {
        let (channel_cid, mut channel) = self.get_channel().await?;

        let mut live = channel.live.unwrap_or_default();

        if let Some(peer_id) = peer_id {
            live.peer_id = peer_id;
        }

        if let Some(video_topic) = video_topic {
            live.video_topic = video_topic;
        }

        if let Some(chat_topic) = chat_topic {
            live.chat_topic = chat_topic;
        }

        channel.live = Some(live);

        self.update_channel(channel_cid, &channel).await
    }

    pub async fn ban_user(&self, user: Address) -> Result<(), Error> {
        let (channel_cid, mut channel) = self.get_channel().await?;

        let mut bans: Bans = match channel.bans {
            Some(link) => self.ipfs.dag_get(link.link, Option::<&str>::None).await?,
            None => Bans::default(),
        };

        if !bans.banned_addrs.insert(user) {
            return Err(Error::AlreadyAdded);
        }

        let bans_cid = self.ipfs.dag_put(&bans, Codec::default()).await?;

        channel.bans = Some(bans_cid.into());

        self.update_channel(channel_cid, &channel).await
    }

    pub async fn unban_user(&self, user: &Address) -> Result<(), Error> {
        let (channel_cid, mut channel) = self.get_channel().await?;

        let mut bans: Bans = match channel.bans {
            Some(link) => self.ipfs.dag_get(link.link, Option::<&str>::None).await?,
            None => return Err(Error::NotFound),
        };

        if !bans.banned_addrs.remove(user) {
            return Err(Error::NotFound);
        }

        let bans_cid = self.ipfs.dag_put(&bans, Codec::default()).await?;

        channel.bans = Some(bans_cid.into());

        self.update_channel(channel_cid, &channel).await
    }

    pub async fn replace_ban_list(&self, bans_cid: Cid) -> Result<(), Error> {
        let (channel_cid, mut channel) = self.get_channel().await?;

        channel.bans = Some(bans_cid.into());

        self.update_channel(channel_cid, &channel).await
    }

    pub async fn add_moderator(&self, user: Address) -> Result<(), Error> {
        let (channel_cid, mut channel) = self.get_channel().await?;

        let mut mods: Moderators = match channel.mods {
            Some(link) => self.ipfs.dag_get(link.link, Option::<&str>::None).await?,
            None => Moderators::default(),
        };

        if !mods.moderator_addrs.insert(user) {
            return Err(Error::AlreadyAdded);
        }

        let mods_cid = self.ipfs.dag_put(&mods, Codec::default()).await?;

        channel.mods = Some(mods_cid.into());

        self.update_channel(channel_cid, &channel).await
    }

    pub async fn remove_moderator(&self, user: &Address) -> Result<(), Error> {
        let (channel_cid, mut channel) = self.get_channel().await?;

        let mut mods: Moderators = match channel.mods {
            Some(link) => self.ipfs.dag_get(link.link, Option::<&str>::None).await?,
            None => return Err(Error::NotFound),
        };

        if !mods.moderator_addrs.remove(user) {
            return Err(Error::NotFound);
        }

        let mods_cid = self.ipfs.dag_put(&mods, Codec::default()).await?;

        channel.mods = Some(mods_cid.into());

        self.update_channel(channel_cid, &channel).await
    }

    pub async fn replace_moderator_list(&self, moderators_cid: Cid) -> Result<(), Error> {
        let (channel_cid, mut channel) = self.get_channel().await?;

        channel.mods = Some(moderators_cid.into());

        self.update_channel(channel_cid, &channel).await
    }

    /// Add new content.
    pub async fn add_content(&self, content_cid: Cid) -> Result<(), Error> {
        let media: Media = self.ipfs.dag_get(content_cid, Some("/link")).await?;
        let date_time = Utc.timestamp(media.timestamp(), 0);

        let (channel_cid, mut channel) = self.get_channel().await?;

        let path = get_path(date_time);

        let (mut contents, index) = match channel.content_index {
            Some(index) => (
                self.ipfs.dag_get(index.date_time.link, Some(path)).await?,
                Some(index.date_time.link),
            ),
            None => (Content::default(), None),
        };

        if !contents.content.insert(content_cid.into()) {
            return Err(Error::AlreadyAdded);
        }

        let contents_cid = self.ipfs.dag_put(&contents, Codec::default()).await?;

        let date_time_cid = self
            .update_date_time_index(date_time, index, contents_cid)
            .await?;

        channel.content_index = Some(Indexing {
            date_time: date_time_cid.into(),
        });

        self.update_channel(channel_cid, &channel).await
    }

    /// Add a new comment on the specified media.
    pub async fn add_comment(&self, comment_cid: Cid) -> Result<(), Error> {
        let media: Media = self.ipfs.dag_get(comment_cid, Some("/link/origin")).await?;
        let date_time = Utc.timestamp(media.timestamp(), 0);

        let (channel_cid, mut channel) = self.get_channel().await?;

        let path = get_path(date_time);

        let (mut comments, index) = match channel.comment_index {
            Some(index) => (
                self.ipfs.dag_get(index.date_time.link, Some(path)).await?,
                Some(index.date_time.link),
            ),
            None => (Comments::default(), None),
        };

        comments
            .comments
            .entry(comment_cid)
            .and_modify(|vec| vec.push(comment_cid.into()))
            .or_insert(vec![comment_cid.into()]);

        let comments_cid = self.ipfs.dag_put(&comments, Codec::default()).await?;

        let date_time_cid = self
            .update_date_time_index(date_time, index, comments_cid)
            .await?;

        channel.comment_index = Some(Indexing {
            date_time: date_time_cid.into(),
        });

        self.update_channel(channel_cid, &channel).await
    }

    /// Remove a specific content.
    pub async fn remove_content(&self, content_cid: Cid) -> Result<(), Error> {
        let media: Media = self.ipfs.dag_get(content_cid, Option::<&str>::None).await?;
        let date_time = Utc.timestamp(media.timestamp(), 0);

        let (channel_cid, mut channel) = self.get_channel().await?;

        let path = get_path(date_time);

        let (mut contents, index): (Content, _) = match channel.content_index {
            Some(index) => (
                self.ipfs.dag_get(index.date_time.link, Some(path)).await?,
                Some(index.date_time.link),
            ),
            None => return Err(Error::NotFound),
        };

        if !contents.content.remove(&content_cid.into()) {
            return Err(Error::NotFound);
        }

        let contents_cid = self.ipfs.dag_put(&contents, Codec::default()).await?;

        let date_time_cid = self
            .update_date_time_index(date_time, index, contents_cid)
            .await?;

        channel.content_index = Some(Indexing {
            date_time: date_time_cid.into(),
        });

        self.update_channel(channel_cid, &channel).await
    }

    /// Remove a specific comment.
    pub async fn remove_comment(&self, comment_cid: Cid) -> Result<(), Error> {
        let comment: Comment = self.ipfs.dag_get(comment_cid, Option::<&str>::None).await?;
        let content_cid = comment.origin.link;

        let media: Media = self.ipfs.dag_get(content_cid, Option::<&str>::None).await?;
        let date_time = Utc.timestamp(media.timestamp(), 0);

        let (channel_cid, mut channel) = self.get_channel().await?;

        let path = get_path(date_time);

        let (mut comments, index): (Comments, _) = match channel.comment_index {
            Some(index) => (
                self.ipfs
                    .dag_get(index.date_time.link, Some(path.clone()))
                    .await?,
                Some(index.date_time.link),
            ),
            None => return Err(Error::NotFound),
        };

        if !comments.comments.remove(&content_cid).is_some() {
            return Err(Error::NotFound);
        }

        let comments_cid = self.ipfs.dag_put(&comments, Codec::default()).await?;

        let date_time_cid = self
            .update_date_time_index(date_time, index, comments_cid)
            .await?;

        channel.comment_index = Some(Indexing {
            date_time: date_time_cid.into(),
        });

        self.update_channel(channel_cid, &channel).await
    }

    async fn get_channel(&self) -> Result<(Cid, ChannelMetadata), Error> {
        let cid = self.anchor.retreive().await?;
        let channel: ChannelMetadata = self.ipfs.dag_get(cid, Option::<&str>::None).await?;

        Ok((cid, channel))
    }

    async fn update_channel(&self, old_cid: Cid, channel: &ChannelMetadata) -> Result<(), Error> {
        let new_cid = self.ipfs.dag_put(channel, Codec::default()).await?;

        self.ipfs.pin_update(old_cid, new_cid).await?;

        self.anchor.anchor(new_cid).await?;

        Ok(())
    }

    async fn update_date_time_index(
        &self,
        date_time: DateTime<Utc>,
        index: Option<Cid>,
        content_cid: Cid,
    ) -> Result<Cid, Error> {
        let mut seconds: Seconds = if let Some(index) = index {
            let path = format!(
                "year/{}/month/{}/day/{}/hour/{}/minute/{}",
                date_time.year(),
                date_time.month(),
                date_time.day(),
                date_time.hour(),
                date_time.minute()
            );

            self.ipfs
                .dag_get(index, Some(path))
                .await
                .unwrap_or_default()
        } else {
            Seconds::default()
        };

        seconds
            .second
            .insert(date_time.second(), content_cid.into());
        let seconds_cid = self.ipfs.dag_put(&seconds, Codec::default()).await?;

        let mut minutes: Minutes = if let Some(index) = index {
            let path = format!(
                "year/{}/month/{}/day/{}/hour/{}",
                date_time.year(),
                date_time.month(),
                date_time.day(),
                date_time.hour()
            );

            self.ipfs
                .dag_get(index, Some(path))
                .await
                .unwrap_or_default()
        } else {
            Minutes::default()
        };

        minutes
            .minute
            .insert(date_time.minute(), seconds_cid.into());
        let minutes_cid = self.ipfs.dag_put(&minutes, Codec::default()).await?;

        let mut hours: Hourly = if let Some(index) = index {
            let path = format!(
                "year/{}/month/{}/day/{}",
                date_time.year(),
                date_time.month(),
                date_time.day()
            );

            self.ipfs
                .dag_get(index, Some(path))
                .await
                .unwrap_or_default()
        } else {
            Hourly::default()
        };

        hours.hour.insert(date_time.hour(), minutes_cid.into());
        let hours_cid = self.ipfs.dag_put(&hours, Codec::default()).await?;

        let mut days: Daily = if let Some(index) = index {
            let path = format!("year/{}/month/{}", date_time.year(), date_time.month());

            self.ipfs
                .dag_get(index, Some(path))
                .await
                .unwrap_or_default()
        } else {
            Daily::default()
        };

        days.day.insert(date_time.day(), hours_cid.into());
        let days_cid = self.ipfs.dag_put(&days, Codec::default()).await?;

        let mut months: Monthly = if let Some(index) = index {
            let path = format!("year/{}", date_time.year());

            self.ipfs
                .dag_get(index, Some(path))
                .await
                .unwrap_or_default()
        } else {
            Monthly::default()
        };

        months.month.insert(date_time.month(), days_cid.into());
        let months_cid = self.ipfs.dag_put(&months, Codec::default()).await?;

        let mut years: Yearly = if let Some(index) = index {
            self.ipfs
                .dag_get(index, Option::<&str>::None)
                .await
                .unwrap_or_default()
        } else {
            Yearly::default()
        };

        years.year.insert(date_time.year(), months_cid.into());
        let years_cid = self.ipfs.dag_put(&years, Codec::default()).await?;

        Ok(years_cid)
    }
}
