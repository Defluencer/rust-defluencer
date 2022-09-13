pub mod channel;
pub mod crypto;
pub mod errors;
pub mod indexing;
pub mod moderation_cache;
pub mod user;
pub mod utils;

use std::collections::{HashMap, HashSet};

use cid::Cid;

use errors::Error;

use futures::{
    future::AbortRegistration,
    stream::{self, FuturesUnordered},
    Stream, StreamExt, TryStreamExt,
};

use indexing::hamt;

use ipns_records::IPNSRecord;
use linked_data::{
    channel::ChannelMetadata,
    follows::Follows,
    identity::Identity,
    indexes::date_time::*,
    media::Media,
    types::{IPLDLink, IPNSAddress},
};

use ipfs_api::{responses::PubSubMessage, IpfsService};

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

    /// Receive updates from the agregation channel.
    ///
    /// Each update is the CID of some content.
    pub fn subscribe_agregation_updates(
        &self,
        channel: String,
        regis: AbortRegistration,
    ) -> impl Stream<Item = Result<Cid, Error>> + '_ {
        self.ipfs
            .pubsub_sub(channel.into_bytes(), regis)
            .err_into()
            .try_filter_map(move |msg| async move {
                let PubSubMessage { from: _, data } = msg;

                let cid = Cid::try_from(data)?;

                let _media = self.ipfs.dag_get::<String, Media>(cid, None).await?;

                Ok(Some(cid))
            })
    }

    /// Subscribe to a channel.
    ///
    /// Return CID of the latest channel metadata.
    pub fn subscribe_channel_updates(
        &self,
        channel_addr: IPNSAddress,
        regis: AbortRegistration,
    ) -> impl Stream<Item = Result<Cid, Error>> + '_ {
        let topic = channel_addr.to_pubsub_topic();

        let stream = self
            .ipfs
            .pubsub_sub(topic.into_bytes(), regis)
            .boxed_local();

        let latest_channel_cid = Cid::default();
        let sequence = 0;

        stream::try_unfold(
            (sequence, latest_channel_cid, stream),
            move |(mut sequence, mut latest_channel_cid, mut stream)| async move {
                let msg = match stream.try_next().await? {
                    Some(msg) => msg,
                    None => return Result::<_, Error>::Ok(None),
                };

                let PubSubMessage { from: _, data } = msg;

                let record = IPNSRecord::from_bytes(&data)?;

                let seq = record.get_sequence();

                if sequence >= seq {
                    return Ok(None);
                }

                let cid = record.get_value();

                if latest_channel_cid == cid {
                    return Ok(None);
                }

                if record.verify(channel_addr.into()).is_err() {
                    return Ok(None);
                }

                sequence = seq;
                latest_channel_cid = cid;

                Ok(Some((
                    latest_channel_cid,
                    (sequence, latest_channel_cid, stream),
                )))
            },
        )
    }

    /// Returns all followees channels on the social web without duplicates.
    ///
    /// WARNING! This search will crawl the entire web. Limiting the number of result is best.
    pub fn streaming_web_crawl(
        &self,
        addresses: impl Iterator<Item = IPNSAddress>,
    ) -> impl Stream<Item = Result<(Cid, ChannelMetadata), Error>> + '_ {
        let set = HashSet::new();

        let resolve_pool = FuturesUnordered::<_>::new();
        let metadata_pool = FuturesUnordered::<_>::new();
        let follows_pool = FuturesUnordered::<_>::new();

        for addr in addresses {
            resolve_pool.push(self.ipfs.name_resolve(addr.into()));
        }

        stream::try_unfold(
            (set, resolve_pool, metadata_pool, follows_pool),
            move |(mut set, mut resolve_pool, mut metadata_pool, mut follows_pool)| async move {
                futures::select! {
                    result = resolve_pool.try_next() => {
                        let cid = match result? {
                            Some(cid) => cid,
                            None => return Result::<_, Error>::Ok(None),
                        };

                        if !set.insert(cid) {
                            return Ok(None);
                        }

                        metadata_pool.push(async move { (cid, self.ipfs.dag_get::<&str, ChannelMetadata>(cid, None).await) });
                    },
                    option = metadata_pool.next() => {
                         let (cid, metadata) = match option {
                            Some(mt) => mt,
                            None => return Ok(None),
                        };

                        let metadata = metadata?;

                        if let Some(ipld) = metadata.follows {
                            follows_pool.push(self.ipfs.dag_get::<&str, Follows>(ipld.link, None));
                        }

                        let next_item = (cid, metadata.clone());

                        return Ok(Some((next_item,
                            (set, resolve_pool, metadata_pool, follows_pool),
                        )));
                    },
                    result = follows_pool.try_next() => {
                         let follows = match result? {
                            Some(fl) => fl,
                            None => return Ok(None),
                        };

                        for addr in follows.followees {
                            resolve_pool.push(self.ipfs.name_resolve(addr.into()));
                        }
                    },
                }

                Ok(None)
            },
        )

        /* stream::once(async move {
            let channel_cid = self.ipfs.name_resolve(addr.into()).await?;

            let id = self
                .ipfs
                .dag_get::<&str, Identity>(channel_cid, Some("/link/identity"))
                .await?;

            Result::<_, Error>::Ok(id)
        })
        .map_ok(|identity| self.web_crawl_step(identity))
        .try_flatten() */
    }

    /* fn web_crawl_step(
        &self,
        channels: HashMap<Cid, ChannelMetadata>,
    ) -> impl Stream<Item = Result<HashMap<Cid, ChannelMetadata>, Error>> + '_ {
        stream::try_unfold(
            (channels.clone(), channels),
            move |(mut visited, mut unvisited)| async move {
                let map = if unvisited.len() > 0 {
                    let identities = self.followees_identity(unvisited.values()).await;

                    self.channels_metadata(identities.values()).await
                } else {
                    return Result::<_, Error>::Ok(None);
                };

                let diff = map
                    .into_iter()
                    .filter_map(|(key, channel)| match visited.contains_key(&key) {
                        true => None,
                        false => {
                            visited.insert(key, channel.clone());

                            Some((key, channel))
                        }
                    })
                    .collect::<HashMap<Cid, ChannelMetadata>>();

                unvisited = diff.clone();

                Ok(Some((diff, (visited, unvisited))))
            },
        )
    } */

    /* fn web_crawl_step(
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
                    .filter_map(
                        |(key, channel)| match visited.insert(key, channel.clone()) {
                            Some(_) => None,
                            None => Some((key, channel)),
                        },
                    )
                    .collect::<HashMap<Cid, ChannelMetadata>>();

                unvisited = diff.clone();

                Ok(Some((diff, (identity, visited, unvisited))))
            },
        )
    } */

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
    pub fn stream_content_rev_chrono(
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

    /// Stream all the comments for some content on the channel.
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
