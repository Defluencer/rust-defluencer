use crate::{
    anchors::{Anchor, IPNSAnchor},
    errors::Error,
    indexing::{datetime, hamt},
    utils::add_image,
};

use chrono::{TimeZone, Utc};

use cid::Cid;

use ipfs_api::{responses::Codec, IpfsService};

use linked_data::{
    channel::ChannelMetadata,
    comments::Comment,
    follows::Follows,
    identity::Identity,
    indexes::hamt::HAMTRoot,
    live::LiveSettings,
    media::Media,
    moderation::{Bans, Moderators},
    types::{Address, IPLDLink, IPNSAddress},
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
        avatar: Option<std::path::PathBuf>,
        channel_ipns: Option<Cid>,
        channel_ens: Option<String>,
    ) -> Result<Cid, Error> {
        let (channel_cid, mut channel) = self.get_metadata().await?;

        let mut identity = self
            .ipfs
            .dag_get::<&str, Identity>(channel.identity.link, None)
            .await?;

        if let Some(name) = display_name {
            identity.display_name = name;
        }

        if let Some(avatar) = avatar {
            identity.avatar = add_image(&self.ipfs, &avatar).await?.into();
        }

        if let Some(ipns) = channel_ipns {
            identity.channel_ipns = Some(ipns.into());
        }

        if let Some(ens) = channel_ens {
            identity.channel_ens = Some(ens);
        }

        let cid = self.ipfs.dag_put(&identity, Codec::default()).await?;

        channel.identity = cid.into();

        self.update_metadata(channel_cid, &channel).await
    }

    /// Update your identity data.
    #[cfg(target_arch = "wasm32")]
    pub async fn update_identity(
        &self,
        display_name: Option<String>,
        avatar: Option<web_sys::File>,
        channel_ipns: Option<IPNSAddress>,
        channel_ens: Option<String>,
    ) -> Result<Cid, Error> {
        let (channel_cid, mut channel) = self.get_channel().await?;

        let mut identity = self
            .ipfs
            .dag_get::<&str, Identity>(channel.identity.link, None)
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

        channel.identity = cid.into();

        self.update_channel(channel_cid, &channel).await
    }

    pub async fn replace_identity(&self, identity: IPLDLink) -> Result<Cid, Error> {
        let (channel_cid, mut channel) = self.get_metadata().await?;

        channel.identity = identity;

        self.update_metadata(channel_cid, &channel).await
    }

    /// Follow a channel.
    pub async fn follow(&self, identity: IPLDLink) -> Result<Cid, Error> {
        let (channel_cid, mut channel) = self.get_metadata().await?;

        let mut follows = match channel.follows {
            Some(ipld) => self.ipfs.dag_get::<&str, Follows>(ipld.link, None).await?,
            None => Follows::default(),
        };

        if !follows.followees.insert(identity) {
            return Err(Error::AlreadyAdded);
        }

        let cid = self.ipfs.dag_put(&follows, Codec::default()).await?;

        channel.follows = Some(cid.into());

        self.update_metadata(channel_cid, &channel).await
    }

    /// Unfollow a channel.
    pub async fn unfollow(&self, identity: IPLDLink) -> Result<Cid, Error> {
        let (channel_cid, mut channel) = self.get_metadata().await?;

        let mut follows = match channel.follows {
            Some(ipld) => self.ipfs.dag_get::<&str, Follows>(ipld.link, None).await?,
            None => return Err(Error::NotFound),
        };

        if !follows.followees.remove(&identity) {
            return Err(Error::NotFound);
        }

        let cid = self.ipfs.dag_put(&follows, Codec::default()).await?;

        channel.follows = Some(cid.into());

        self.update_metadata(channel_cid, &channel).await
    }

    pub async fn replace_follow_list(&self, follows: IPLDLink) -> Result<Cid, Error> {
        let (channel_cid, mut channel) = self.get_metadata().await?;

        channel.follows = Some(follows);

        self.update_metadata(channel_cid, &channel).await
    }

    /// Update live chat & streaming settings.
    ///
    /// Returns CID of new settings.
    pub async fn update_live_settings(
        &self,
        peer_id: Option<Cid>,
        video_topic: Option<String>,
        chat_topic: Option<String>,
        archiving: Option<bool>,
    ) -> Result<Cid, Error> {
        let (channel_cid, mut channel) = self.get_metadata().await?;

        let mut live = match channel.live {
            Some(ipld) => {
                self.ipfs
                    .dag_get::<&str, LiveSettings>(ipld.link, None)
                    .await?
            }
            None => LiveSettings::default(),
        };

        if let Some(peer_id) = peer_id {
            live.peer_id = peer_id.into();
        }

        if let Some(video_topic) = video_topic {
            live.video_topic = video_topic;
        }

        if let Some(chat_topic) = chat_topic {
            live.chat_topic = Some(chat_topic);
        }

        if let Some(archive) = archiving {
            live.archiving = archive;
        }

        let cid = self.ipfs.dag_put(&live, Codec::default()).await?;

        channel.live = Some(cid.into());

        self.update_metadata(channel_cid, &channel).await?;

        Ok(cid)
    }

    pub async fn replace_live_settings(&self, settings: IPLDLink) -> Result<Cid, Error> {
        let (channel_cid, mut channel) = self.get_metadata().await?;

        channel.live = Some(settings);

        self.update_metadata(channel_cid, &channel).await
    }

    /// Returns new list if a user was banned.
    pub async fn ban_user(&self, user: Address) -> Result<Option<Cid>, Error> {
        let (channel_cid, mut channel) = self.get_metadata().await?;

        let mut live = match channel.live {
            Some(ipld) => {
                self.ipfs
                    .dag_get::<&str, LiveSettings>(ipld.link, None)
                    .await?
            }
            None => LiveSettings::default(),
        };

        let mut bans: Bans = match live.bans {
            Some(link) => self.ipfs.dag_get(link.link, Option::<&str>::None).await?,
            None => Bans::default(),
        };

        if !bans.banned_addrs.insert(user) {
            return Ok(None);
        }

        let bans_cid = self.ipfs.dag_put(&bans, Codec::default()).await?;
        live.bans = Some(bans_cid.into());

        let live_cid = self.ipfs.dag_put(&live, Codec::default()).await?;
        channel.live = Some(live_cid.into());

        self.update_metadata(channel_cid, &channel).await?;

        Ok(Some(bans_cid))
    }

    /// Returns new list if a user was unbanned.
    pub async fn unban_user(&self, user: &Address) -> Result<Option<Cid>, Error> {
        let (channel_cid, mut channel) = self.get_metadata().await?;

        let mut live = match channel.live {
            Some(ipld) => {
                self.ipfs
                    .dag_get::<&str, LiveSettings>(ipld.link, None)
                    .await?
            }
            None => LiveSettings::default(),
        };

        let mut bans: Bans = match live.bans {
            Some(link) => self.ipfs.dag_get(link.link, Option::<&str>::None).await?,
            None => return Ok(None),
        };

        if !bans.banned_addrs.remove(user) {
            return Ok(None);
        }

        let bans_cid = self.ipfs.dag_put(&bans, Codec::default()).await?;
        live.bans = Some(bans_cid.into());

        let live_cid = self.ipfs.dag_put(&live, Codec::default()).await?;
        channel.live = Some(live_cid.into());

        self.update_metadata(channel_cid, &channel).await?;

        Ok(Some(bans_cid))
    }

    pub async fn replace_ban_list(&self, bans: IPLDLink) -> Result<Cid, Error> {
        let (channel_cid, mut channel) = self.get_metadata().await?;

        let mut live = match channel.live {
            Some(ipld) => {
                self.ipfs
                    .dag_get::<&str, LiveSettings>(ipld.link, None)
                    .await?
            }
            None => LiveSettings::default(),
        };

        live.bans = Some(bans);

        let live_cid = self.ipfs.dag_put(&live, Codec::default()).await?;
        channel.live = Some(live_cid.into());

        self.update_metadata(channel_cid, &channel).await
    }

    /// Returns new channel Cid if a moderator was added.
    pub async fn add_moderator(&self, user: Address) -> Result<Option<Cid>, Error> {
        let (channel_cid, mut channel) = self.get_metadata().await?;

        let mut live = match channel.live {
            Some(ipld) => {
                self.ipfs
                    .dag_get::<&str, LiveSettings>(ipld.link, None)
                    .await?
            }
            None => LiveSettings::default(),
        };

        let mut mods: Moderators = match live.mods {
            Some(link) => self.ipfs.dag_get(link.link, Option::<&str>::None).await?,
            None => Moderators::default(),
        };

        if !mods.moderator_addrs.insert(user) {
            return Ok(None);
        }

        let mods_cid = self.ipfs.dag_put(&mods, Codec::default()).await?;
        live.mods = Some(mods_cid.into());

        let live_cid = self.ipfs.dag_put(&live, Codec::default()).await?;
        channel.live = Some(live_cid.into());

        let new_channel = self.update_metadata(channel_cid, &channel).await?;

        Ok(Some(new_channel))
    }

    /// Returns new channel Cid if a moderator was removed.
    pub async fn remove_moderator(&self, user: &Address) -> Result<Option<Cid>, Error> {
        let (channel_cid, mut channel) = self.get_metadata().await?;

        let mut live = match channel.live {
            Some(ipld) => {
                self.ipfs
                    .dag_get::<&str, LiveSettings>(ipld.link, None)
                    .await?
            }
            None => LiveSettings::default(),
        };

        let mut mods: Moderators = match live.mods {
            Some(link) => self.ipfs.dag_get(link.link, Option::<&str>::None).await?,
            None => return Ok(None),
        };

        if !mods.moderator_addrs.remove(user) {
            return Ok(None);
        }

        let mods_cid = self.ipfs.dag_put(&mods, Codec::default()).await?;
        live.mods = Some(mods_cid.into());

        let live_cid = self.ipfs.dag_put(&live, Codec::default()).await?;
        channel.live = Some(live_cid.into());

        let new_channel = self.update_metadata(channel_cid, &channel).await?;

        Ok(Some(new_channel))
    }

    pub async fn replace_moderator_list(&self, moderators: IPLDLink) -> Result<Cid, Error> {
        let (channel_cid, mut channel) = self.get_metadata().await?;

        let mut live = match channel.live {
            Some(ipld) => {
                self.ipfs
                    .dag_get::<&str, LiveSettings>(ipld.link, None)
                    .await?
            }
            None => LiveSettings::default(),
        };

        live.mods = Some(moderators);

        let live_cid = self.ipfs.dag_put(&live, Codec::default()).await?;
        channel.live = Some(live_cid.into());

        self.update_metadata(channel_cid, &channel).await
    }

    /// Add new content.
    pub async fn add_content(&self, content_cid: Cid) -> Result<Cid, Error> {
        // path "/link" to skip dag-jose block
        let media: Media = self.ipfs.dag_get(content_cid, Some("/link")).await?;
        let datetime = Utc.timestamp(media.user_timestamp(), 0);

        let (channel_cid, mut channel) = self.get_metadata().await?;

        datetime::insert(
            &self.ipfs,
            datetime,
            &mut channel.content_index,
            content_cid,
        )
        .await?;

        self.update_metadata(channel_cid, &channel).await
    }

    /// Remove a specific media.
    /// Also remove associated comments.
    pub async fn remove_content(&self, content_cid: Cid) -> Result<Cid, Error> {
        let media: Media = self.ipfs.dag_get(content_cid, Option::<&str>::None).await?;
        let datetime = Utc.timestamp(media.user_timestamp(), 0);

        let (channel_cid, mut channel) = self.get_metadata().await?;

        if channel.content_index.is_none() {
            return Ok(channel_cid);
        };

        datetime::remove(
            &self.ipfs,
            datetime,
            &mut channel.content_index,
            content_cid,
        )
        .await?;

        // Remove comments too!
        if let Some(index) = channel.comment_index.as_mut() {
            hamt::remove(&self.ipfs, index, content_cid).await?;
        }

        self.update_metadata(channel_cid, &channel).await
    }

    /// Add a new comment on the specified media.
    pub async fn add_comment(&self, comment_cid: Cid) -> Result<Cid, Error> {
        let comment: Comment = self.ipfs.dag_get(comment_cid, Option::<&str>::None).await?;
        let media_cid = comment.origin;

        let (channel_cid, mut channel) = self.get_metadata().await?;

        let mut index = match channel.comment_index {
            Some(index) => index,
            None => self
                .ipfs
                .dag_put(&HAMTRoot::default(), Codec::default())
                .await?
                .into(),
        };

        let mut comments = match hamt::get(&self.ipfs, index, media_cid).await? {
            Some(comments) => comments.into(),
            None => self
                .ipfs
                .dag_put(&HAMTRoot::default(), Codec::default())
                .await?
                .into(),
        };

        hamt::insert(&self.ipfs, &mut comments, comment_cid, comment_cid).await?;

        hamt::insert(&self.ipfs, &mut index, media_cid, comments.link).await?;

        channel.comment_index = Some(index.into());

        self.update_metadata(channel_cid, &channel).await
    }

    /// Remove a specific comment.
    pub async fn remove_comment(&self, comment_cid: Cid) -> Result<(), Error> {
        let comment: Comment = self.ipfs.dag_get(comment_cid, Option::<&str>::None).await?;
        let media_cid = comment.origin;

        let (channel_cid, mut channel) = self.get_metadata().await?;

        let mut index = match channel.comment_index {
            Some(it) => it,
            _ => return Ok(()),
        };

        let mut comments = match hamt::get(&self.ipfs, index, media_cid).await? {
            Some(comments) => comments.into(),
            None => return Ok(()),
        };

        hamt::remove(&self.ipfs, &mut comments, comment_cid).await?;

        hamt::insert(&self.ipfs, &mut index, media_cid, comments.link).await?;

        channel.comment_index = Some(index.into());

        self.update_metadata(channel_cid, &channel).await?;

        Ok(())
    }

    /// Pin a channel to this local node.
    ///
    /// WARNING!
    /// This function pin ALL content from the channel.
    /// The amout of data downloaded could be massive.
    pub async fn pin_channel(&self) -> Result<(), Error> {
        let cid = self.anchor.retreive().await?;

        self.ipfs.pin_add(cid, true).await?;

        Ok(())
    }

    /// Unpin a channel from this local node.
    ///
    /// This function unpin everyting; metadata, content, comment, etc...
    pub async fn unpin_channel(&self) -> Result<(), Error> {
        let cid = self.anchor.retreive().await?;

        self.ipfs.pin_rm(cid, true).await?;

        Ok(())
    }

    pub async fn get_metadata(&self) -> Result<(Cid, ChannelMetadata), Error> {
        let cid = self.anchor.retreive().await?;
        let channel: ChannelMetadata = self.ipfs.dag_get(cid, Option::<&str>::None).await?;

        Ok((cid, channel))
    }

    async fn update_metadata(&self, old_cid: Cid, channel: &ChannelMetadata) -> Result<Cid, Error> {
        let new_cid = self.ipfs.dag_put(channel, Codec::default()).await?;

        self.ipfs.pin_update(old_cid, new_cid).await?;

        self.anchor.anchor(new_cid).await?;

        Ok(new_cid)
    }
}

impl Channel<IPNSAnchor> {
    pub async fn get_ipns_address(&self) -> Result<Option<IPNSAddress>, Error> {
        let key_list = self.ipfs.key_list().await?;

        let ipns = match key_list.get(self.anchor.get_name()) {
            Some(cid) => (*cid).into(),
            None => return Ok(None),
        };

        Ok(Some(ipns))
    }

    pub fn get_name(&self) -> &str {
        self.anchor.get_name()
    }
}
