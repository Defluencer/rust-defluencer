use std::ops::Add;

use crate::{
    errors::Error,
    indexing::{datetime, hamt},
    signatures::Signer,
    utils::add_image,
};

use chrono::{Duration, SecondsFormat, TimeZone, Utc};

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
    types::{Address, CryptoKey, IPLDLink, IPNSAddress, IPNSRecord, KeyType, ValidityType},
};

use cid::multihash::Multihash;

use prost::Message;

use sha2::Digest;

#[derive(Clone)]
pub struct Channel<T>
where
    T: Signer + Clone,
{
    ipfs: IpfsService,
    ipns: IPNSAddress,
    signer: T,
}

impl<T> Channel<T>
where
    T: Signer + Clone,
{
    pub fn new(ipfs: IpfsService, ipns: IPNSAddress, signer: T) -> Self {
        Self { ipfs, signer, ipns }
    }

    /// Create a new channel.
    pub async fn create(ipfs: IpfsService, signer: T, identity: IPLDLink) -> Result<Self, Error> {
        let metadata = ChannelMetadata {
            identity,
            ..Default::default()
        };

        let cid = ipfs.dag_put(&metadata, Codec::default()).await?;

        let ipns = create_ipns_record(cid, &ipfs, &signer, metadata.seq).await?;

        let channel = Self { ipfs, signer, ipns };

        Ok(channel)
    }

    /// Update your identity data.
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn update_identity(
        &self,
        display_name: Option<String>,
        avatar: Option<std::path::PathBuf>,
        channel_ipns: Option<Cid>,
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
            identity.avatar = Some(add_image(&self.ipfs, &avatar).await?.into());
        }

        if let Some(ipns) = channel_ipns {
            identity.channel_ipns = Some(ipns.into());
        }

        let cid = self.ipfs.dag_put(&identity, Codec::default()).await?;

        channel.identity = cid.into();

        self.update_metadata(channel_cid, &channel).await?;

        Ok(cid)
    }

    /// Update your identity data.
    #[cfg(target_arch = "wasm32")]
    pub async fn update_identity(
        &self,
        display_name: Option<String>,
        avatar: Option<web_sys::File>,
        channel_ipns: Option<Cid>,
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
            identity.avatar = Some(add_image(&self.ipfs, avatar).await?.into());
        }

        if let Some(ipns) = channel_ipns {
            identity.channel_ipns = Some(ipns.into());
        }

        let cid = self.ipfs.dag_put(&identity, Codec::default()).await?;

        channel.identity = cid.into();

        self.update_metadata(channel_cid, &channel).await?;

        Ok(cid)
    }

    /// Replace your current Identity.
    pub async fn replace_identity(&self, identity: IPLDLink) -> Result<Cid, Error> {
        let (channel_cid, mut channel) = self.get_metadata().await?;

        channel.identity = identity;

        self.update_metadata(channel_cid, &channel).await?;

        Ok(identity.link)
    }

    /// Follow a channel.
    pub async fn follow(&self, addr: IPNSAddress) -> Result<Cid, Error> {
        let (channel_cid, mut channel) = self.get_metadata().await?;

        let mut follows = match channel.follows {
            Some(ipld) => self.ipfs.dag_get::<&str, Follows>(ipld.link, None).await?,
            None => Follows::default(),
        };

        if !follows.followees.insert(addr) {
            return Err(Error::AlreadyAdded);
        }

        let cid = self.ipfs.dag_put(&follows, Codec::default()).await?;

        channel.follows = Some(cid.into());

        self.update_metadata(channel_cid, &channel).await?;

        Ok(cid)
    }

    /// Unfollow a channel.
    pub async fn unfollow(&self, addr: IPNSAddress) -> Result<Cid, Error> {
        let (channel_cid, mut channel) = self.get_metadata().await?;

        let mut follows = match channel.follows {
            Some(ipld) => self.ipfs.dag_get::<&str, Follows>(ipld.link, None).await?,
            None => return Err(Error::NotFound),
        };

        if !follows.followees.remove(&addr) {
            return Err(Error::NotFound);
        }

        let cid = self.ipfs.dag_put(&follows, Codec::default()).await?;

        channel.follows = Some(cid.into());

        self.update_metadata(channel_cid, &channel).await?;

        Ok(cid)
    }

    /// Replace your follow list.
    pub async fn replace_follow_list(&self, follows: IPLDLink) -> Result<Cid, Error> {
        let (channel_cid, mut channel) = self.get_metadata().await?;

        channel.follows = Some(follows);

        self.update_metadata(channel_cid, &channel).await?;

        Ok(follows.link)
    }

    /// Update live chat & streaming settings.
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

    /// Replace your live chat & streaming settings.
    pub async fn replace_live_settings(&self, settings: IPLDLink) -> Result<Cid, Error> {
        let (channel_cid, mut channel) = self.get_metadata().await?;

        channel.live = Some(settings);

        self.update_metadata(channel_cid, &channel).await?;

        Ok(settings.link)
    }

    /// Add a user to your ban list.
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

    /// Remove a user from your ban list.
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

    /// Replace your ban list.
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

        self.update_metadata(channel_cid, &channel).await?;

        Ok(bans.link)
    }

    /// Add a moderator to your list.
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

        self.update_metadata(channel_cid, &channel).await?;

        Ok(Some(mods_cid))
    }

    /// Remove a moderator from your list.
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

        self.update_metadata(channel_cid, &channel).await?;

        Ok(Some(mods_cid))
    }

    /// Replace your moderator list.
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

        self.update_metadata(channel_cid, &channel).await?;

        Ok(moderators.link)
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

        channel.comment_index = Some(index);

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

        channel.comment_index = Some(index);

        self.update_metadata(channel_cid, &channel).await?;

        Ok(())
    }

    pub async fn get_metadata(&self) -> Result<(Cid, ChannelMetadata), Error> {
        let cid = self.ipfs.name_resolve(self.ipns.into()).await?;
        let channel: ChannelMetadata = self.ipfs.dag_get(cid, Option::<&str>::None).await?;

        Ok((cid, channel))
    }

    async fn update_metadata(&self, old_cid: Cid, channel: &ChannelMetadata) -> Result<Cid, Error> {
        let new_cid = self.ipfs.dag_put(channel, Codec::default()).await?;

        self.ipfs.pin_update(old_cid, new_cid).await?;

        let ipns = create_ipns_record(new_cid, &self.ipfs, &self.signer, channel.seq + 1).await?;

        if ipns != self.ipns {
            return Err(Error::IPNSMismatch);
        }

        Ok(new_cid)
    }

    pub fn get_address(&self) -> IPNSAddress {
        self.ipns
    }
}

async fn create_ipns_record(
    cid: Cid,
    ipfs: &IpfsService,
    signer: &impl Signer,
    sequence: u64,
) -> Result<IPNSAddress, Error> {
    let value = format!("/ipfs/{}", cid.to_string()).into_bytes();

    let validity = Utc::now()
        .add(Duration::weeks(52))
        .to_rfc3339_opts(SecondsFormat::Nanos, false)
        .into_bytes();

    let validity_type = ValidityType::EOL;

    let signing_input = {
        let mut data = Vec::with_capacity(
            value.len() + validity.len() + 3, /* b"EOL".len() == 3 */
        );

        data.extend(value.iter());
        data.extend(validity.iter());
        data.extend(validity_type.to_string().as_bytes());

        data
    };

    let (public_key, signature) = signer.sign(signing_input).await?;

    let verifying_key = k256::ecdsa::VerifyingKey::from(public_key);
    let signature = signature.to_der().to_bytes().into_vec();

    let public_key = CryptoKey {
        key_type: KeyType::Secp256k1 as i32,
        data: verifying_key.to_bytes().to_vec(),
    }
    .encode_to_vec(); // Protobuf encoding

    let ipns = {
        let multihash = if public_key.len() <= 42 {
            Multihash::wrap(0x00, &public_key).unwrap()
        } else {
            let hash = sha2::Sha256::new_with_prefix(&public_key).finalize();

            Multihash::wrap(0x12, &hash).unwrap()
        };

        Cid::new_v1(0x72, multihash)
    };

    let record = IPNSRecord {
        value,
        signature,
        validity_type: validity_type as i32,
        validity,
        sequence,
        ttl: 0, //TODO figure this out!
        public_key,
    };

    let record_data = record.encode_to_vec(); // Protobuf encoding

    ipfs.dht_put(ipns, record_data).await?;

    Ok(ipns.into())
}
