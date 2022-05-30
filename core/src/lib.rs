pub mod channel;
pub mod content_cache;
pub mod errors;
pub mod indexing;
pub mod moderation_cache;
pub mod signatures;
pub mod user;
pub mod utils;

use std::collections::HashMap;

use cid::Cid;

use errors::Error;

use futures::{
    future::AbortRegistration,
    stream::{self, FuturesUnordered},
    Stream, StreamExt, TryStreamExt,
};

use indexing::hamt;

use linked_data::{
    channel::ChannelMetadata,
    follows::Follows,
    identity::Identity,
    indexes::date_time::*,
    types::{CryptoKey, IPLDLink, IPNSAddress, IPNSRecord, ValidityType},
};

use ipfs_api::{responses::PubSubMessage, IpfsService};

use prost::Message;

#[derive(Default, Clone)]
pub struct Defluencer {
    ipfs: IpfsService,
}

impl Defluencer {
    pub fn new(ipfs: IpfsService) -> Self {
        Self { ipfs }
    }

    /// Pin a channel to this local node.
    ///
    /// WARNING!
    /// This function pin ALL content from the channel.
    /// The amout of data downloaded could be massive.
    pub async fn pin_channel(&self, ipns: IPNSAddress) -> Result<(), Error> {
        let cid = self.ipfs.name_resolve(ipns.into()).await?;

        self.ipfs.pin_add(cid, true).await?;

        Ok(())
    }

    /// Unpin a channel from this local node.
    ///
    /// This function unpin everyting; metadata, content, comment, etc...
    pub async fn unpin_channel(&self, ipns: IPNSAddress) -> Result<(), Error> {
        let cid = self.ipfs.name_resolve(ipns.into()).await?;

        self.ipfs.pin_rm(cid, true).await?;

        Ok(())
    }

    /// Subscribe to a channel.
    /// The first value returned is the current metadata CID.
    ///
    /// Each update's crypto-signature is verified.
    pub fn subscribe_channel_updates(
        &self,
        channel_ipns: IPNSAddress,
        regis: AbortRegistration,
    ) -> impl Stream<Item = Result<Cid, Error>> + '_ {
        use signature::Signature;
        use signature::Verifier;

        stream::once(async move {
            let ipns = channel_ipns.into();
            let mut channel_cid = self.ipfs.name_resolve(ipns).await?;

            let metadata = self
                .ipfs
                .dag_get::<&str, ChannelMetadata>(channel_cid, None)
                .await?;

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
                        sequence,
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

                    if sequence <= metadata.seq {
                        return Ok(None);
                    }

                    let mut signing_input = Vec::with_capacity(
                        value.len() + validity.len() + 3, /* b"EOL".len() == 3 */
                    );

                    signing_input.extend(value);
                    signing_input.extend(validity);
                    signing_input.extend(validity_type.to_string().as_bytes());

                    // If the pub key is not in the record use the peer id
                    let crypto_key = if !public_key.is_empty() {
                        CryptoKey::decode(public_key.as_ref())?
                    } else {
                        CryptoKey::decode(ipns.hash().digest())?
                    };

                    match crypto_key.key_type {
                        0/* KeyType::RSA */ => unimplemented!(),
                        1/* KeyType::Ed25519 */ => unimplemented!()/* {
                            let public_key = ed25519_dalek::PublicKey::from_bytes(&crypto_key.data)?;

                            let signature = ed25519_dalek::Signature::from_bytes(&signature)?;

                            if public_key.verify(&signing_input, &signature).is_err() {
                                return Ok(None);
                            }
                        } */,
                        2/* KeyType::Secp256k1 */ => {
                            let public_key = k256::ecdsa::VerifyingKey::from_sec1_bytes(&crypto_key.data)?;

                            let signature = k256::ecdsa::Signature::from_bytes(&signature)?;

                            if public_key.verify(&signing_input, &signature).is_err() {
                                return Ok(None);
                            }
                        },
                        3/* KeyType::ECDSA */ => unimplemented!(),
                        _ => panic!("Enum has only 4 possible values")
                    }

                    channel_cid = cid;

                    Ok(Some(channel_cid))
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
    pub fn streaming_web_crawl(
        &self,
        addr: IPNSAddress,
    ) -> impl Stream<Item = Result<HashMap<Cid, ChannelMetadata>, Error>> + '_ {
        stream::once(async move {
            let channel_cid = self.ipfs.name_resolve(addr.into()).await?;

            let channel_meta = self
                .ipfs
                .dag_get::<&str, ChannelMetadata>(channel_cid, None)
                .await?;

            let id = self
                .ipfs
                .dag_get::<&str, Identity>(channel_meta.identity.link, None)
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

                Ok(Some((diff, (identity, visited, unvisited))))
            },
        )
    }

    /// Return all the cids and channels of all the identities provided.
    pub async fn channels_metadata(
        &self,
        identities: impl Iterator<Item = &Identity>,
    ) -> HashMap<Cid, ChannelMetadata> {
        let stream: FuturesUnordered<_> = identities
            .filter_map(|identity| {
                identity
                    .channel_ipns
                    .map(|ipns| self.ipfs.name_resolve(ipns.into()))
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
            .filter_map(|channel| {
                channel
                    .follows
                    .map(|ipld| self.ipfs.dag_get::<&str, Follows>(ipld.link, None))
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
            .filter_map(|addr| async move {
                match self.ipfs.name_resolve(addr.into()).await {
                    Ok(cid) => Some(cid),
                    Err(_) => None,
                }
            })
            .filter_map(|cid| async move {
                match self.ipfs.dag_get::<&str, ChannelMetadata>(cid, None).await {
                    Ok(channel) => Some(channel),
                    Err(_) => None,
                }
            })
            .filter_map(|channel| async move {
                match self
                    .ipfs
                    .dag_get::<&str, Identity>(channel.identity.link, None)
                    .await
                {
                    Ok(identity) => Some((channel.identity.link, identity)),
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

            Ok(Some((months, iter)))
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

                Ok(Some((days, iter)))
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

            Ok(Some((hours, iter)))
        })
    }

    fn stream_minutes(&self, hours: Hourly) -> impl Stream<Item = Result<Minutes, Error>> + '_ {
        stream::try_unfold(hours.hour.into_values().rev(), move |mut iter| async move {
            let ipld = match iter.next() {
                Some(ipld) => ipld,
                None => return Ok(None),
            };

            let minutes = self.ipfs.dag_get::<&str, Minutes>(ipld.link, None).await?;

            Ok(Some((minutes, iter)))
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
                        .map(Result::<_, Error>::Ok),
                );

                Ok(Some((stream, iter)))
            },
        )
        .try_flatten()
        .map_ok(|set| stream::iter(set.into_iter().map(Ok)))
        .try_flatten()
        .map_ok(|ipld| ipld.link)
    }

    /// Lazily stream all the comments for some content on the channel
    pub fn stream_comments(
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
