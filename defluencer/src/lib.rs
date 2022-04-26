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
    future::AbortRegistration,
    stream::{self, FuturesUnordered},
    Stream, StreamExt, TryStreamExt,
};

use heck::{ToSnakeCase, ToTitleCase};

use indexing::hamt;
use linked_data::{
    channel::ChannelMetadata,
    follows::Follows,
    identity::Identity,
    indexes::date_time::*,
    types::{CryptoKey, IPLDLink, IPNSAddress, IPNSRecord, ValidityType},
};

use ipfs_api::{
    responses::{Codec, KeyPair, PubSubMessage},
    IpfsService,
};

use pkcs8::{EncodePrivateKey, LineEnding};

use rand_core::{OsRng, RngCore};

use signatures::Signer;

use user::User;

use prost::Message;

#[derive(Default, Clone)]
pub struct Defluencer {
    ipfs: IpfsService,
}

impl Defluencer {
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
    /// Returns a secret phrase, a channel and IPNS address.
    pub async fn create_local_channel(
        &self,
        channel_name: impl Into<Cow<'static, str>>,
    ) -> Result<(Mnemonic, Channel<IPNSAnchor>, Cid), Error> {
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
        let cid = Cid::try_from(id.as_str())?;

        let anchor = IPNSAnchor::new(self.ipfs.clone(), name);
        let channel = Channel::new(self.ipfs.clone(), anchor);

        let peer_id = self.ipfs.peer_id().await?;
        let video_topic = format!("{}_video", display_name.to_snake_case());

        channel
            .update_live_settings(Some(peer_id), Some(video_topic), None, Some(false))
            .await?;

        channel
            .update_identity(Some(display_name), None, Some(cid), None)
            .await?;

        Ok((mnemonic, channel, cid))
    }

    /// Returns a channel by name previously created or imported on this node.
    pub async fn get_local_channel(
        &self,
        channel_name: impl Into<Cow<'static, str>>,
    ) -> Result<Option<Channel<IPNSAnchor>>, Error> {
        let list = self.ipfs.key_list().await?;

        let name = channel_name.into();
        let key_name = name.to_snake_case();

        if !list.contains_key(&key_name) {
            return Ok(None);
        }

        let anchor = IPNSAnchor::new(self.ipfs.clone(), key_name);
        let channel = Channel::new(self.ipfs.clone(), anchor);

        Ok(Some(channel))
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

    /// Subscribe to an IPNS address.
    /// The first value returned is the current IPNS link.
    ///
    /// Each update's crypto-signature is verified.
    ///
    /// Only works for IPNS address pointing to a CID (for now).
    pub fn subscribe_ipns_updates(
        &self,
        channel_ipns: IPNSAddress,
        regis: AbortRegistration,
    ) -> impl Stream<Item = Result<Cid, Error>> + '_ {
        use signature::Verifier;

        stream::once(async move {
            let ipns = channel_ipns.into();
            let mut channel_cid = self.ipfs.name_resolve(ipns).await?;

            let topic = channel_ipns.to_pubsub_topic();

            let stream = self
                .ipfs
                .pubsub_sub(topic.into_bytes(), regis)
                .err_into()
                .try_filter_map(move |msg| async move {
                    let PubSubMessage { from: _, data } = msg;

                    let IPNSRecord {
                        value,
                        signature,
                        validity_type,
                        validity,
                        sequence: _,
                        ttl: _,
                        public_key,
                    } = IPNSRecord::decode(data.as_ref())?;

                    let validity_type = match validity_type {
                        0 => ValidityType::EOL, // The only possible answer for now
                        _ => panic!("Does ValidityType now has more than one variant?"),
                    };

                    let cid_str = std::str::from_utf8(&value)?;
                    let cid = Cid::try_from(cid_str)?;

                    if cid == channel_cid {
                        return Ok(None);
                    }

                    let public_key = if public_key.len() > 0 {
                        ed25519_dalek::PublicKey::from_bytes(&public_key)?
                    } else {
                        let key = CryptoKey::decode(ipns.hash().digest())?;
                        ed25519_dalek::PublicKey::from_bytes(&key.data)?
                    };

                    let mut signing_input = Vec::with_capacity(
                        value.len() + validity.len() + 3, /* b"EOL".len() == 3 */
                    );

                    signing_input.extend(value);
                    signing_input.extend(validity);
                    signing_input.extend(validity_type.to_string().as_bytes());

                    let signature = ed25519_dalek::Signature::from_bytes(&signature)?;

                    if public_key.verify(&signing_input, &signature).is_err() {
                        return Ok(None);
                    }

                    channel_cid = cid;

                    return Ok(Some(channel_cid));
                });

            let stream = stream::once(async move { Ok(channel_cid) }).chain(stream);

            Result::<_, Error>::Ok(stream)
        })
        .try_flatten()
        /* .and_then(move |cid| async move {
            let channel = self
                .ipfs
                .dag_get::<&str, ChannelMetadata>(cid, None)
                .await?;

            return Ok((cid, channel));
        }) */
    }

    /// Returns all followees channels on the social web,
    /// one more degree of separation each iteration without duplicates.
    pub async fn streaming_web_crawl(
        &self,
        identity: IPLDLink,
    ) -> impl Stream<Item = Result<HashMap<Cid, ChannelMetadata>, Error>> + '_ {
        stream::once(async move {
            let id = self
                .ipfs
                .dag_get::<&str, Identity>(identity.link, None)
                .await?;

            Result::<_, Error>::Ok(id)
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
                    (Some(id), _) => self.channels_metadata(std::iter::once(&id)).await,
                    (None, x) if x != 0 => {
                        let identities = self.followees_identity(unvisited.values()).await;

                        self.channels_metadata(identities.values()).await
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

    /// Return all the cids and channels of all the identities provided.
    pub async fn channels_metadata(
        &self,
        identities: impl Iterator<Item = &Identity>,
    ) -> HashMap<Cid, ChannelMetadata> {
        let stream: FuturesUnordered<_> = identities
            .filter_map(|identity| match identity.channel_ipns {
                Some(ipns) => Some(self.ipfs.name_resolve(ipns.into())),
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
    pub async fn followees_identity(
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

    /// Lazily stream a channel's content.
    pub fn stream_content_chronologically(
        &self,
        content_index: IPLDLink,
    ) -> impl Stream<Item = Result<Cid, Error>> + '_ {
        stream::once(async move {
            let yearly = self
                .ipfs
                .dag_get::<&str, Yearly>(content_index.link, None)
                .await?;

            Result::<_, Error>::Ok(yearly)
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
        comment_index: IPLDLink,
        content_cid: Cid,
    ) -> impl Stream<Item = Result<Cid, Error>> + '_ {
        stream::once(async move {
            let comments = hamt::get(&self.ipfs, comment_index, content_cid).await?;

            Result::<_, Error>::Ok(comments)
        })
        .try_filter_map(move |option| async move {
            match option {
                Some(comments) => Ok(Some(hamt::values(&self.ipfs, comments.into()))),
                None => Ok(None),
            }
        })
        .try_flatten()
    }
}
