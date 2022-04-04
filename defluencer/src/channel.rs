use std::collections::{HashSet, VecDeque};

use crate::{
    anchors::{Anchor, IPNSAnchor},
    errors::Error,
    utils::add_image,
};

use async_recursion::async_recursion;
use chrono::{DateTime, Datelike, TimeZone, Timelike, Utc};

use cid::Cid;

use ipfs_api::{responses::Codec, IpfsService};

use linked_data::{
    channel::ChannelMetadata,
    comments::Comment,
    follows::Follows,
    identity::Identity,
    indexes::{
        date_time::*,
        hamt::{
            BitField, BucketEntry, Element, HAMTNode, HAMTRoot, BUCKET_SIZE, DIGEST_LENGTH_BYTES,
        },
        log::ChainLink,
    },
    live::LiveSettings,
    media::Media,
    moderation::{Bans, Moderators},
    Address, IPLDLink, IPNSAddress, PeerId,
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

    /// Update your identity data.
    #[cfg(target_arch = "wasm32")]
    pub async fn update_identity(
        &self,
        display_name: Option<String>,
        avatar: Option<web_sys::File>,
        channel_ipns: Option<Cid>,
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

    pub async fn replace_identity(&self, identity_cid: Cid) -> Result<Cid, Error> {
        let (channel_cid, mut channel) = self.get_channel().await?;

        channel.identity = identity_cid.into();

        self.update_channel(channel_cid, &channel).await
    }

    /// Follow a channel.
    pub async fn follow(&self, user_identity: Cid) -> Result<Cid, Error> {
        let (channel_cid, mut channel) = self.get_channel().await?;

        let mut follows = match channel.follows {
            Some(ipld) => self.ipfs.dag_get::<&str, Follows>(ipld.link, None).await?,
            None => Follows::default(),
        };

        if !follows.followees.insert(user_identity.into()) {
            return Err(Error::AlreadyAdded);
        }

        let cid = self.ipfs.dag_put(&follows, Codec::default()).await?;

        channel.follows = Some(cid.into());

        self.update_channel(channel_cid, &channel).await
    }

    /// Unfollow a channel.
    pub async fn unfollow(&self, user_identity: Cid) -> Result<Cid, Error> {
        let (channel_cid, mut channel) = self.get_channel().await?;

        let mut follows = match channel.follows {
            Some(ipld) => self.ipfs.dag_get::<&str, Follows>(ipld.link, None).await?,
            None => return Err(Error::NotFound),
        };

        if !follows.followees.remove(&user_identity.into()) {
            return Err(Error::NotFound);
        }

        let cid = self.ipfs.dag_put(&follows, Codec::default()).await?;

        channel.follows = Some(cid.into());

        self.update_channel(channel_cid, &channel).await
    }

    pub async fn replace_follow_list(&self, follows_cid: Cid) -> Result<Cid, Error> {
        let (channel_cid, mut channel) = self.get_channel().await?;

        channel.follows = Some(follows_cid.into());

        self.update_channel(channel_cid, &channel).await
    }

    /// Update live chat & streaming settings.
    pub async fn update_live_settings(
        &self,
        peer_id: Option<PeerId>,
        video_topic: Option<String>,
        chat_topic: Option<String>,
    ) -> Result<Cid, Error> {
        let (channel_cid, mut channel) = self.get_channel().await?;

        let mut live = match channel.live {
            Some(ipld) => {
                self.ipfs
                    .dag_get::<&str, LiveSettings>(ipld.link, None)
                    .await?
            }
            None => LiveSettings::default(),
        };

        if let Some(peer_id) = peer_id {
            live.peer_id = peer_id;
        }

        if let Some(video_topic) = video_topic {
            live.video_topic = video_topic;
        }

        if let Some(chat_topic) = chat_topic {
            live.chat_topic = chat_topic;
        }

        let cid = self.ipfs.dag_put(&live, Codec::default()).await?;

        channel.live = Some(cid.into());

        self.update_channel(channel_cid, &channel).await
    }

    pub async fn replace_live_settings(&self, settings_cid: Cid) -> Result<Cid, Error> {
        let (channel_cid, mut channel) = self.get_channel().await?;

        channel.live = Some(settings_cid.into());

        self.update_channel(channel_cid, &channel).await
    }

    pub async fn ban_user(&self, user: Address) -> Result<Cid, Error> {
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

    pub async fn unban_user(&self, user: &Address) -> Result<Cid, Error> {
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

    pub async fn replace_ban_list(&self, bans_cid: Cid) -> Result<Cid, Error> {
        let (channel_cid, mut channel) = self.get_channel().await?;

        channel.bans = Some(bans_cid.into());

        self.update_channel(channel_cid, &channel).await
    }

    pub async fn add_moderator(&self, user: Address) -> Result<Cid, Error> {
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

    pub async fn remove_moderator(&self, user: &Address) -> Result<Cid, Error> {
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

    pub async fn replace_moderator_list(&self, moderators_cid: Cid) -> Result<Cid, Error> {
        let (channel_cid, mut channel) = self.get_channel().await?;

        channel.mods = Some(moderators_cid.into());

        self.update_channel(channel_cid, &channel).await
    }

    /// Add new content.
    pub async fn add_content(&self, content_cid: Cid) -> Result<Cid, Error> {
        // path "/link" to skip dag-jose block
        let media: Media = self.ipfs.dag_get(content_cid, Some("/link")).await?;
        let datetime = Utc.timestamp(media.user_timestamp(), 0);

        let (channel_cid, mut channel) = self.get_channel().await?;

        let new_index = self
            .log_index_add(channel.content_index.log, content_cid)
            .await?;

        channel.content_index.log = Some(new_index.into());

        let new_index = self
            .datetime_index_insert(datetime, channel.content_index.date_time, content_cid)
            .await?;

        channel.content_index.date_time = Some(new_index.into());

        self.update_channel(channel_cid, &channel).await
    }

    /// Remove a specific content.
    pub async fn remove_content(&self, content_cid: Cid) -> Result<Cid, Error> {
        let media: Media = self.ipfs.dag_get(content_cid, Option::<&str>::None).await?;
        let datetime = Utc.timestamp(media.user_timestamp(), 0);

        let (channel_cid, mut channel) = self.get_channel().await?;

        if let Some(index) = channel.content_index.log {
            let new_index = self.log_index_remove(index, content_cid).await?;

            channel.content_index.log = Some(new_index.into());
        }

        if let Some(index) = channel.content_index.date_time {
            let new_index = self
                .datetime_index_remove(datetime, index, content_cid)
                .await?;

            channel.content_index.date_time = Some(new_index.into());
        }

        self.update_channel(channel_cid, &channel).await
    }

    /// Add a new comment on the specified media.
    pub async fn add_comment(&self, comment_cid: Cid) -> Result<Cid, Error> {
        let comment: Comment = self.ipfs.dag_get(comment_cid, Option::<&str>::None).await?;
        let media_cid = comment.origin;

        let (channel_cid, mut channel) = self.get_channel().await?;

        let index = match channel.comment_index.hamt {
            Some(it) => it,
            _ => return Err(Error::NotFound),
        };

        let comments = self.hamt_get(index, &media_cid).await?;
        let comments = self
            .hamt_insert(comments.into(), &comment_cid, &Cid::default())
            .await?;

        let new_index = self.hamt_insert(index, &media_cid, &comments).await?;

        channel.comment_index.hamt = Some(new_index.into());

        self.update_channel(channel_cid, &channel).await
    }

    /// Remove a specific comment.
    pub async fn remove_comment(&self, comment_cid: Cid) -> Result<Cid, Error> {
        let comment: Comment = self.ipfs.dag_get(comment_cid, Option::<&str>::None).await?;
        let media_cid = comment.origin;

        let (channel_cid, mut channel) = self.get_channel().await?;

        let index = match channel.comment_index.hamt {
            Some(it) => it,
            _ => return Err(Error::NotFound),
        };

        let comments = self.hamt_get(index, &media_cid).await?;
        let comments = self.hamt_remove(comments.into(), &comment_cid).await?;

        let index = self.hamt_insert(index, &media_cid, &comments).await?;

        channel.comment_index.hamt = Some(index.into());

        self.update_channel(channel_cid, &channel).await
    }

    /// Pin a channel to this local node.
    ///
    /// WARNING!
    /// This function pin ALL content from the channel.
    /// The amout of data could be massive.
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

    async fn get_channel(&self) -> Result<(Cid, ChannelMetadata), Error> {
        let cid = self.anchor.retreive().await?;
        let channel: ChannelMetadata = self.ipfs.dag_get(cid, Option::<&str>::None).await?;

        Ok((cid, channel))
    }

    async fn update_channel(&self, old_cid: Cid, channel: &ChannelMetadata) -> Result<Cid, Error> {
        let new_cid = self.ipfs.dag_put(channel, Codec::default()).await?;

        self.ipfs.pin_update(old_cid, new_cid).await?;

        self.anchor.anchor(new_cid).await?;

        Ok(new_cid)
    }

    async fn log_index_add(&self, index: Option<IPLDLink>, add_cid: Cid) -> Result<Cid, Error> {
        let mut chainlink = match index {
            Some(index) => {
                self.ipfs
                    .dag_get::<&str, ChainLink>(index.link, None)
                    .await?
            }
            None => ChainLink::default(),
        };

        chainlink.media = add_cid.into();
        chainlink.previous = index;

        let cid = self.ipfs.dag_put(&chainlink, Codec::default()).await?;

        Ok(cid)
    }

    async fn log_index_remove(&self, index: IPLDLink, remove_cid: Cid) -> Result<Cid, Error> {
        let mut chainlinks = Vec::default();
        let mut previous: Option<IPLDLink> = Some(index);

        loop {
            let cid = match previous {
                Some(ipld) => ipld.link,
                None => break,
            };

            let chainlink = self.ipfs.dag_get::<&str, ChainLink>(cid, None).await?;

            if chainlink.media.link == remove_cid {
                previous = chainlink.previous;

                break;
            } else {
                previous = chainlink.previous;

                chainlinks.push(chainlink);
            }
        }

        for mut chainlink in chainlinks.into_iter().rev() {
            chainlink.previous = previous;

            let cid = self.ipfs.dag_put(&chainlink, Codec::default()).await?;

            previous = Some(cid.into());
        }

        Ok(previous.unwrap().link)
    }

    async fn datetime_index_insert(
        &self,
        date_time: DateTime<Utc>,
        index: Option<IPLDLink>,
        add_cid: Cid,
    ) -> Result<Cid, Error> {
        let mut yearly = Yearly::default();
        let mut monthly = Monthly::default();
        let mut daily = Daily::default();
        let mut hourly = Hourly::default();
        let mut minutes = Minutes::default();
        let mut seconds = Seconds::default();

        if let Some(index) = index {
            yearly = self.ipfs.dag_get::<&str, Yearly>(index.link, None).await?;
        }

        if let Some(ipld) = yearly.year.get(&date_time.year()) {
            monthly = self.ipfs.dag_get::<&str, Monthly>(ipld.link, None).await?;
        }

        if let Some(ipld) = monthly.month.get(&date_time.month()) {
            daily = self.ipfs.dag_get::<&str, Daily>(ipld.link, None).await?;
        }

        if let Some(ipld) = daily.day.get(&date_time.day()) {
            hourly = self.ipfs.dag_get::<&str, Hourly>(ipld.link, None).await?;
        }

        if let Some(ipld) = hourly.hour.get(&date_time.hour()) {
            minutes = self.ipfs.dag_get::<&str, Minutes>(ipld.link, None).await?;
        }

        if let Some(ipld) = minutes.minute.get(&date_time.minute()) {
            seconds = self.ipfs.dag_get::<&str, Seconds>(ipld.link, None).await?;
        }

        seconds
            .second
            .entry(date_time.second())
            .and_modify(|set| {
                set.insert(add_cid.into());
            })
            .or_insert({
                let mut set = HashSet::with_capacity(1);
                set.insert(add_cid.into());
                set
            });
        let cid = self.ipfs.dag_put(&seconds, Codec::default()).await?;

        minutes.minute.insert(date_time.minute(), cid.into());
        let cid = self.ipfs.dag_put(&minutes, Codec::default()).await?;

        hourly.hour.insert(date_time.hour(), cid.into());
        let cid = self.ipfs.dag_put(&hourly, Codec::default()).await?;

        daily.day.insert(date_time.day(), cid.into());
        let cid = self.ipfs.dag_put(&daily, Codec::default()).await?;

        monthly.month.insert(date_time.month(), cid.into());
        let cid = self.ipfs.dag_put(&monthly, Codec::default()).await?;

        yearly.year.insert(date_time.year(), cid.into());
        let cid = self.ipfs.dag_put(&yearly, Codec::default()).await?;

        Ok(cid)
    }

    async fn datetime_index_remove(
        &self,
        date_time: DateTime<Utc>,
        index: IPLDLink,
        remove_cid: Cid,
    ) -> Result<Cid, Error> {
        let mut yearly = self.ipfs.dag_get::<&str, Yearly>(index.link, None).await?;

        let mut monthly = match yearly.year.get(&date_time.year()) {
            Some(ipld) => self.ipfs.dag_get::<&str, Monthly>(ipld.link, None).await?,
            None => return Err(Error::NotFound),
        };

        let mut daily = match monthly.month.get(&date_time.month()) {
            Some(ipld) => self.ipfs.dag_get::<&str, Daily>(ipld.link, None).await?,
            None => return Err(Error::NotFound),
        };

        let mut hourly = match daily.day.get(&date_time.day()) {
            Some(ipld) => self.ipfs.dag_get::<&str, Hourly>(ipld.link, None).await?,
            None => return Err(Error::NotFound),
        };

        let mut minutes = match hourly.hour.get(&date_time.hour()) {
            Some(ipld) => self.ipfs.dag_get::<&str, Minutes>(ipld.link, None).await?,
            None => return Err(Error::NotFound),
        };

        let mut seconds = match minutes.minute.get(&date_time.minute()) {
            Some(ipld) => self.ipfs.dag_get::<&str, Seconds>(ipld.link, None).await?,
            None => return Err(Error::NotFound),
        };

        let set = match seconds.second.get_mut(&date_time.second()) {
            Some(set) => set,
            None => return Err(Error::NotFound),
        };

        set.remove(&remove_cid.into());

        if set.is_empty() {
            seconds.second.remove(&date_time.second());
        }

        if seconds.second.is_empty() {
            minutes.minute.remove(&date_time.minute());
        } else {
            let cid = self.ipfs.dag_put(&seconds, Codec::default()).await?;

            minutes.minute.insert(date_time.minute(), cid.into());
        }

        if minutes.minute.is_empty() {
            hourly.hour.remove(&date_time.hour());
        } else {
            let cid = self.ipfs.dag_put(&minutes, Codec::default()).await?;

            hourly.hour.insert(date_time.hour(), cid.into());
        }

        if hourly.hour.is_empty() {
            daily.day.remove(&date_time.day());
        } else {
            let cid = self.ipfs.dag_put(&hourly, Codec::default()).await?;

            daily.day.insert(date_time.day(), cid.into());
        }

        if daily.day.is_empty() {
            monthly.month.remove(&date_time.month());
        } else {
            let cid = self.ipfs.dag_put(&daily, Codec::default()).await?;

            monthly.month.insert(date_time.month(), cid.into());
        }

        if monthly.month.is_empty() {
            yearly.year.remove(&date_time.year());
        } else {
            let cid = self.ipfs.dag_put(&monthly, Codec::default()).await?;

            yearly.year.insert(date_time.year(), cid.into());
        }

        let cid = self.ipfs.dag_put(&yearly, Codec::default()).await?;

        Ok(cid)
    }

    async fn hamt_get(&self, root: IPLDLink, key: &Cid) -> Result<Cid, Error> {
        let root = self.ipfs.dag_get::<&str, HAMTRoot>(root.link, None).await?;

        let mut depth = 0;
        let mut node = root.hamt;

        loop {
            let index = key.hash().digest()[depth] as usize;
            let map = BitField::from(node.map);
            let data_index = map[0..index].count_ones();

            if !map[index] {
                // CASE: index bit is not set
                return Err(Error::NotFound);
            }

            // CASE: index bit is set
            match &node.data[data_index] {
                Element::Link(ipld) => {
                    if (depth + 1) > DIGEST_LENGTH_BYTES {
                        // MAX Collisions Error
                        return Err(Error::NotFound); // TODO add new error
                    }

                    node = self.ipfs.dag_get::<&str, HAMTNode>(ipld.link, None).await?;
                    depth += 1;

                    continue;
                }
                Element::Bucket(vec) => match vec.binary_search(&(*key).into()) {
                    Ok(idx) => return Ok(vec[idx].value.link),
                    Err(_) => return Err(Error::NotFound),
                },
            }
        }
    }

    async fn hamt_insert(
        &self,
        index_cid: IPLDLink,
        add_key: &Cid,
        add_value: &Cid,
    ) -> Result<Cid, Error> {
        let mut root = self
            .ipfs
            .dag_get::<&str, HAMTRoot>(index_cid.link, None)
            .await?;

        self.set(add_key, add_value, 0, &mut root.hamt).await?;

        let cid = self.ipfs.dag_put(&root, Codec::default()).await?;

        Ok(cid)
    }

    #[async_recursion(?Send)]
    async fn set(
        &self,
        key: &Cid,
        value: &Cid,
        depth: usize,
        node: &mut HAMTNode,
    ) -> Result<Cid, Error> {
        let index = key.hash().digest()[depth] as usize;
        let mut map = BitField::from(node.map);
        let data_index = map[0..index].count_ones();

        if map[index] {
            // CASE: index bit is set
            match &mut node.data[data_index] {
                Element::Link(ipld) => {
                    if (depth + 1) > DIGEST_LENGTH_BYTES {
                        // MAX Collisions Error
                        return Err(Error::NotFound); // TODO add new error
                    }

                    let mut new_node = self.ipfs.dag_get::<&str, HAMTNode>(ipld.link, None).await?;

                    let cid = self.set(key, value, depth + 1, &mut new_node).await?;

                    *ipld = cid.into();

                    let cid = self.ipfs.dag_put(&node, Codec::default()).await?;

                    return Ok(cid);
                }
                Element::Bucket(vec) => {
                    if vec.len() < BUCKET_SIZE {
                        let entry = BucketEntry {
                            key: (*key).into(),
                            value: (*value).into(),
                        };

                        let idx = vec.binary_search(&entry).unwrap_or_else(|x| x);
                        vec.insert(idx, entry);

                        let cid = self.ipfs.dag_put(&node, Codec::default()).await?;

                        return Ok(cid);
                    }

                    let mut new_node = HAMTNode::default();

                    for item in vec.iter() {
                        self.set(&item.key.link, &item.value.link, depth + 1, &mut new_node)
                            .await?;
                    }

                    let cid = self.set(key, value, depth + 1, &mut new_node).await?;

                    node.data[data_index] = Element::Link(cid.into());

                    let cid = self.ipfs.dag_put(&node, Codec::default()).await?;

                    return Ok(cid);
                }
            }
        }

        // CASE: index bit is not set

        let bucket = Element::Bucket(VecDeque::from([BucketEntry {
            key: (*key).into(),
            value: (*value).into(),
        }]));

        node.data.insert(data_index, bucket);

        map.set(index, true);
        node.map = map.into_inner();

        let cid = self.ipfs.dag_put(&node, Codec::default()).await?;

        Ok(cid)
    }

    async fn hamt_remove(&self, index: IPLDLink, key: &Cid) -> Result<Cid, Error> {
        let mut root = self
            .ipfs
            .dag_get::<&str, HAMTRoot>(index.link, None)
            .await?;

        self.delete(key, 0, &mut root.hamt).await?;

        let cid = self.ipfs.dag_put(&root, Codec::default()).await?;

        Ok(cid)
    }

    #[async_recursion(?Send)]
    async fn delete(&self, key: &Cid, depth: usize, node: &mut HAMTNode) -> Result<Element, Error> {
        let index = key.hash().digest()[depth] as usize;
        let mut map = BitField::from(node.map);
        let data_index = map[0..index].count_ones();

        if !map[index] {
            // CASE: index bit is not set
            return Err(Error::NotFound);
        }

        // CASE: index bit is set

        if let Element::Link(ipld) = node.data[data_index] {
            if (depth + 1) > DIGEST_LENGTH_BYTES {
                // MAX Collisions Error
                return Err(Error::NotFound); // TODO add new error
            }

            let mut new_node = self.ipfs.dag_get::<&str, HAMTNode>(ipld.link, None).await?;

            let element = self.delete(key, depth + 1, &mut new_node).await?;

            node.data[data_index] = element;

            if let Element::Link(_) = node.data[data_index] {
                let cid = self.ipfs.dag_put(&node, Codec::default()).await?;

                return Ok(Element::Link(cid.into()));
            }
        }

        let mut links = 0;
        let mut entrees = 0;

        for element in node.data.iter() {
            match element {
                Element::Link(_) => {
                    links += 1;
                }
                Element::Bucket(vec) => {
                    entrees += vec.len();
                }
            }
        }

        if depth == 0 || links > 0 || entrees > (BUCKET_SIZE + 1) {
            if let Element::Bucket(vec) = &mut node.data[data_index] {
                if vec.len() > 1 {
                    match vec.binary_search(&(*key).into()) {
                        Ok(idx) => {
                            vec.remove(idx);
                        }
                        Err(_) => return Err(Error::NotFound),
                    }
                } else {
                    node.data.remove(data_index);

                    map.set(index, false);
                    node.map = map.into_inner();
                }

                let cid = self.ipfs.dag_put(&node, Codec::default()).await?;

                return Ok(Element::Link(cid.into()));
            }
        }

        if depth != 0 && links == 0 && entrees == (BUCKET_SIZE + 1) {
            // Collapse node into parent
            if let Element::Bucket(vec) = &mut node.data[data_index] {
                match vec.binary_search(&(*key).into()) {
                    Ok(idx) => {
                        vec.remove(idx);
                    }
                    Err(_) => return Err(Error::NotFound),
                }

                return Ok(Element::Bucket(vec.clone()));
            }
        }

        Err(Error::NotFound)
    }

    //TODO HAMT iterator
}

impl Channel<IPNSAnchor> {
    pub async fn get_ipns_address(&self) -> Result<IPNSAddress, Error> {
        let key_list = self.ipfs.key_list().await?;

        let cid = match key_list.get(self.anchor.get_name()) {
            Some(keypair) => *keypair,
            None => return Err(ipfs_api::errors::Error::Ipns.into()),
        };

        Ok(cid)
    }
}
