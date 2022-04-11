pub mod anchors;
pub mod channel;
pub mod content_cache;
pub mod errors;
pub mod indexing;
pub mod moderation_cache;
pub mod signatures;
pub mod user;
pub mod utils;

use std::{borrow::Cow, collections::HashMap};

use anchors::IPNSAnchor;

use bip39::{Language, Mnemonic};

use channel::Channel;

use cid::Cid;

use ed25519::KeypairBytes;
use ed25519_dalek::SecretKey;

use errors::Error;

use futures::{
    stream::{self, FuturesUnordered},
    Stream, StreamExt, TryStreamExt,
};

use heck::{ToSnakeCase, ToTitleCase};

use indexing::hamt;
use linked_data::{
    channel::ChannelMetadata, follows::Follows, identity::Identity, indexes::date_time::*,
};

use ipfs_api::{
    responses::{Codec, KeyPair},
    IpfsService,
};

use pkcs8::{EncodePrivateKey, LineEnding};

use rand_core::{OsRng, RngCore};

use signatures::Signer;

use user::User;

pub struct Defluencer {
    ipfs: IpfsService,
}

impl Defluencer {
    pub fn new() -> Self {
        let ipfs = IpfsService::default();

        Self { ipfs }
    }

    pub async fn create_user<T>(
        &self,
        user_name: impl Into<Cow<'static, str>>,
        signer: T,
    ) -> Result<User<T>, Error>
    where
        T: Signer,
    {
        let identity = Identity {
            display_name: user_name.into().into_owned(),
            avatar: Cid::default().into(), //TODO generic avatar cid
            channel_ipns: None,
            channel_ens: None,
        };

        let identity = self.ipfs.dag_put(&identity, Codec::default()).await?.into();

        let user = User::new(self.ipfs.clone(), signer, identity);

        Ok(user)
    }

    /// Create an new channel on this node.
    ///
    /// Returns channel and a secret passphrase used to recreate this channel elsewhere.
    pub async fn create_channel(
        &self,
        channel_name: impl Into<Cow<'static, str>>,
    ) -> Result<(Mnemonic, Channel<IPNSAnchor>), Error> {
        let name = channel_name.into();
        let key_name = name.to_snake_case();
        let display_name = name.to_title_case();

        let mut bytes = [0u8; 32];
        OsRng.fill_bytes(&mut bytes);

        let secret_key = SecretKey::from_bytes(&bytes)?;

        let key_pair_bytes = KeypairBytes {
            secret_key: secret_key.to_bytes(),
            public_key: None,
        };

        let mnemonic = Mnemonic::from_entropy(&bytes, Language::English)?;

        let data = key_pair_bytes.to_pkcs8_pem(LineEnding::default())?;
        let KeyPair { id, name } = self.ipfs.key_import(key_name, data.to_string()).await?;
        let ipns = Cid::try_from(id)?;

        let anchor = IPNSAnchor::new(self.ipfs.clone(), name);
        let channel = Channel::new(self.ipfs.clone(), anchor);

        channel
            .update_identity(Some(display_name), None, Some(ipns), None)
            .await?;

        Ok((mnemonic, channel))
    }

    /// Returns a channel by name previously created or imported on this node.
    pub async fn get_channel(
        &self,
        channel_name: impl Into<Cow<'static, str>>,
    ) -> Result<Channel<IPNSAnchor>, Error> {
        let list = self.ipfs.key_list().await?;

        let name = channel_name.into();
        let key_name = name.to_snake_case();

        if !list.contains_key(&key_name) {
            return Err(Error::NotFound);
        }

        let anchor = IPNSAnchor::new(self.ipfs.clone(), key_name);
        let channel = Channel::new(self.ipfs.clone(), anchor);

        Ok(channel)
    }

    /// Recreate a channel on this node from a secret passphrase.
    ///
    /// Note that having the same channel on multiple nodes is NOT recommended.
    /// Use this to transfer a channel from one node to another.
    pub async fn import_channel(
        &self,
        channel_name: impl Into<Cow<'static, str>>,
        passphrase: impl Into<Cow<'static, str>>,
        pin_content: bool,
    ) -> Result<Channel<IPNSAnchor>, Error> {
        let name = channel_name.into();
        let key_name = name.to_snake_case();

        let mnemonic = Mnemonic::from_phrase(&passphrase.into(), Language::English)?;

        let secret_key = SecretKey::from_bytes(&mnemonic.entropy())?;

        let key_pair_bytes = KeypairBytes {
            secret_key: secret_key.to_bytes(),
            public_key: None,
        };

        let data = key_pair_bytes.to_pkcs8_pem(LineEnding::default())?;
        let KeyPair { id: _, name } = self.ipfs.key_import(key_name, data.to_string()).await?;

        let anchor = IPNSAnchor::new(self.ipfs.clone(), name);
        let channel = Channel::new(self.ipfs.clone(), anchor);

        if pin_content {
            channel.pin_channel().await?;
        }

        Ok(channel)
    }

    /// Return all the cids and channels of all the identities provided.
    pub async fn get_channels(
        &self,
        identities: impl Iterator<Item = &Identity>,
    ) -> HashMap<Cid, ChannelMetadata> {
        let stream: FuturesUnordered<_> = identities
            .filter_map(|identity| match identity.channel_ipns {
                Some(ipns) => Some(self.ipfs.name_resolve(ipns)),
                None => None,
            })
            .collect();

        stream
            .filter_map(|result| async move {
                match result {
                    Ok(cid) => match self.ipfs.dag_get::<&str, ChannelMetadata>(cid, None).await {
                        Ok(channel) => Some((cid, channel)),
                        Err(_) => None,
                    },
                    Err(_) => None,
                }
            })
            .collect()
            .await
    }

    /// Returns all the cids and identities of all the followees of all the channels provided.
    pub async fn get_followees_identity(
        &self,
        channels: impl Iterator<Item = &ChannelMetadata>,
    ) -> HashMap<Cid, Identity> {
        let stream: FuturesUnordered<_> = channels
            .filter_map(|channel| match channel.follows {
                Some(ipld) => Some(self.ipfs.dag_get::<&str, Follows>(ipld.link, None)),
                None => None,
            })
            .collect();

        stream
            .filter_map(|result| async move {
                match result {
                    Ok(follows) => Some(stream::iter(follows.followees)),
                    Err(_) => None,
                }
            })
            .flatten()
            .filter_map(|ipld| async move {
                match self.ipfs.dag_get::<&str, Identity>(ipld.link, None).await {
                    Ok(identity) => Some((ipld.link, identity)),
                    Err(_) => None,
                }
            })
            .collect()
            .await
    }

    /// Returns all followees channels on the social web,
    /// one more degree of separation each iteration.
    pub async fn streaming_web_crawl(
        &self,
        channel: &ChannelMetadata,
    ) -> impl Stream<Item = Result<HashMap<Cid, ChannelMetadata>, Error>> + '_ {
        stream::try_unfold(Some(channel.identity), move |mut identity| async move {
            let ipld = match identity.take() {
                Some(ipld) => ipld,
                None => return Result::<_, Error>::Ok(None),
            };

            let id = self.ipfs.dag_get::<&str, Identity>(ipld.link, None).await?;

            return Ok(Some((id, identity)));
        })
        .map_ok(|identity| self.web_crawl_step(identity))
        .try_flatten()
    }

    fn web_crawl_step(
        &self,
        identity: Identity,
    ) -> impl Stream<Item = Result<HashMap<Cid, ChannelMetadata>, Error>> + '_ {
        stream::try_unfold(
            (Some(identity), HashMap::new(), HashMap::new()),
            move |(mut identity, mut visited, mut unvisited)| async move {
                let map = match (identity.take(), unvisited.len()) {
                    (Some(id), _) => self.get_channels(std::iter::once(&id)).await,
                    (None, x) if x != 0 => {
                        let identities = self.get_followees_identity(unvisited.values()).await;

                        self.get_channels(identities.values()).await
                    }
                    (_, _) => return Result::<_, Error>::Ok(None),
                };

                let diff = map
                    .into_iter()
                    .filter_map(|(key, channel)| match visited.insert(key, channel) {
                        Some(_) => None,
                        None => Some((key, channel)),
                    })
                    .collect::<HashMap<Cid, ChannelMetadata>>();

                unvisited = diff.clone();

                return Ok(Some((diff, (identity, visited, unvisited))));
            },
        )
    }

    /// Lazily stream a channel's content.
    pub fn stream_content_chronologically(
        &self,
        channel: &ChannelMetadata,
    ) -> impl Stream<Item = Result<Cid, Error>> + '_ {
        stream::try_unfold(channel.content_index, move |mut datetime| async move {
            let ipld = match datetime.take() {
                Some(ipld) => ipld,
                None => return Result::<_, Error>::Ok(None),
            };

            let yearly = self.ipfs.dag_get::<&str, Yearly>(ipld.link, None).await?;

            return Ok(Some((yearly, datetime)));
        })
        .map_ok(|year| self.stream_months(year))
        .try_flatten()
        .map_ok(|month| self.stream_days(month))
        .try_flatten()
        .map_ok(|day| self.stream_hours(day))
        .try_flatten()
        .map_ok(|hours| self.stream_minutes(hours))
        .try_flatten()
        .map_ok(|minutes| self.stream_seconds(minutes))
        .try_flatten()
    }

    fn stream_months(&self, years: Yearly) -> impl Stream<Item = Result<Monthly, Error>> + '_ {
        stream::try_unfold(years.year.into_values().rev(), move |mut iter| async move {
            let ipld = match iter.next() {
                Some(ipld) => ipld,
                None => return Ok(None),
            };

            let months = self.ipfs.dag_get::<&str, Monthly>(ipld.link, None).await?;

            return Ok(Some((months, iter)));
        })
    }

    fn stream_days(&self, months: Monthly) -> impl Stream<Item = Result<Daily, Error>> + '_ {
        stream::try_unfold(
            months.month.into_values().rev(),
            move |mut iter| async move {
                let ipld = match iter.next() {
                    Some(ipld) => ipld,
                    None => return Ok(None),
                };

                let days = self.ipfs.dag_get::<&str, Daily>(ipld.link, None).await?;

                return Ok(Some((days, iter)));
            },
        )
    }

    fn stream_hours(&self, days: Daily) -> impl Stream<Item = Result<Hourly, Error>> + '_ {
        stream::try_unfold(days.day.into_values().rev(), move |mut iter| async move {
            let ipld = match iter.next() {
                Some(ipld) => ipld,
                None => return Ok(None),
            };

            let hours = self.ipfs.dag_get::<&str, Hourly>(ipld.link, None).await?;

            return Ok(Some((hours, iter)));
        })
    }

    fn stream_minutes(&self, hours: Hourly) -> impl Stream<Item = Result<Minutes, Error>> + '_ {
        stream::try_unfold(hours.hour.into_values().rev(), move |mut iter| async move {
            let ipld = match iter.next() {
                Some(ipld) => ipld,
                None => return Ok(None),
            };

            let minutes = self.ipfs.dag_get::<&str, Minutes>(ipld.link, None).await?;

            return Ok(Some((minutes, iter)));
        })
    }

    fn stream_seconds(&self, minutes: Minutes) -> impl Stream<Item = Result<Cid, Error>> + '_ {
        stream::try_unfold(
            minutes.minute.into_values().rev(),
            move |mut iter| async move {
                let ipld = match iter.next() {
                    Some(ipld) => ipld,
                    None => return Result::<_, Error>::Ok(None),
                };

                let seconds = self.ipfs.dag_get::<&str, Seconds>(ipld.link, None).await?;

                let stream = stream::iter(
                    seconds
                        .second
                        .into_values()
                        .rev()
                        .map(|item| Result::<_, Error>::Ok(item)),
                );

                return Ok(Some((stream, iter)));
            },
        )
        .try_flatten()
        .map_ok(|set| stream::iter(set.into_iter().map(|item| Ok(item))))
        .try_flatten()
        .map_ok(|ipld| ipld.link)
    }

    /// Lazily stream all the comments for some content on the channel
    pub async fn stream_comments(
        &self,
        channel: &ChannelMetadata,
        content_cid: Cid,
    ) -> impl Stream<Item = Result<Cid, Error>> + '_ {
        stream::try_unfold(channel.comment_index, move |mut index| async move {
            let ipld = match index.take() {
                Some(ipld) => ipld,
                None => return Result::<_, Error>::Ok(None),
            };

            let comments = match hamt::get(&self.ipfs, ipld, content_cid).await? {
                Some(comments) => comments,
                None => return Result::<_, Error>::Ok(None),
            };

            let stream = hamt::values(&self.ipfs, comments.into());

            Ok(Some((stream, index)))
        })
        .try_flatten()
    }

    /* pub async fn stream_comments(
        &self,
        channel: &ChannelMetadata,
        content_cid: Cid,
    ) -> impl Stream<Item = Cid> + '_ {
        stream::unfold(channel.comment_index.hamt, move |mut index| async move {
            match index {
                Some(ipld) => match hamt::get(&self.ipfs, ipld, content_cid).await {
                    Ok(comments) => {
                        index = None;

                        let stream = hamt::values(&self.ipfs, comments.into());

                        Some((stream, index))
                    }
                    Err(_) => None,
                },
                None => None,
            }
        })
        .flatten()
    } */
}
