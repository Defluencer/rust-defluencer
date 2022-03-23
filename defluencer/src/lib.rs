pub mod anchors;
pub mod channel;
pub mod content_cache;
pub mod errors;
pub mod moderation_cache;
pub mod signatures;
pub mod user;
pub mod utils;

use std::{borrow::Cow, collections::HashSet};

use anchors::IPNSAnchor;

use bip39::{Language, Mnemonic};

use channel::Channel;

use cid::Cid;
use ed25519::KeypairBytes;

use ed25519_dalek::SecretKey;

use errors::Error;

use futures::{stream, Stream, StreamExt};

use heck::{ToSnakeCase, ToTitleCase};

use linked_data::{
    channel::ChannelMetadata,
    indexes::{date_time::*, log::ChainLink},
    IPLDLink,
};

use ipfs_api::{responses::KeyPair, IpfsService};

use pkcs8::{EncodePrivateKey, LineEnding};

use rand_core::{OsRng, RngCore};

pub struct Defluencer {
    ipfs: IpfsService,
}

impl Defluencer {
    pub fn new() -> Self {
        let ipfs = IpfsService::default();

        Self { ipfs }
    }

    /// Create an new channel on this node.
    ///
    /// Returns channel and a secret passphrase useful to recreate this channel elsewhere.
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
    pub async fn import_channel(
        &self,
        channel_name: impl Into<Cow<'static, str>>,
        passphrase: impl Into<Cow<'static, str>>,
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

        Ok(channel)
    }

    /// Pin a channel to this local node.
    ///
    /// WARNING!
    /// This function pin ALL content from the channel.
    /// The amout of data could be massive.
    pub async fn pin_channel(&self, channel: &ChannelMetadata) -> Result<(), Error> {
        todo!()
    }

    /// Unpin a channel from this local node.
    ///
    /// This function unpin everyting; content, comment, etc...
    pub async fn unpin_channel(&self, channel: &ChannelMetadata) -> Result<(), Error> {
        todo!()
    }

    pub fn stream_content_log(
        &self,
        channel: &ChannelMetadata,
    ) -> impl Stream<Item = Result<Cid, Error>> + '_ {
        stream::try_unfold(channel.content_index.log, move |mut previous| async move {
            let cid = match previous {
                Some(ipld) => ipld.link,
                None => return Ok(None),
            };

            let chainlink = self.ipfs.dag_get::<&str, ChainLink>(cid, None).await?;

            previous = chainlink.previous;

            return Ok(Some((chainlink.media.link, previous)));
        })
    }

    pub fn stream_content_chronologically(
        &self,
        channel: &ChannelMetadata,
    ) -> impl Stream<Item = Cid> + '_ {
        stream::unfold(
            channel.content_index.date_time,
            move |mut datetime| async move {
                match datetime {
                    Some(index) => {
                        datetime = None;

                        Some((Some(index.link), datetime))
                    }
                    None => None,
                }
            },
        )
        .flat_map(|index| self.stream_years(index))
        .flat_map(|year| self.stream_months(year))
        .flat_map(|month| self.stream_days(month))
        .flat_map(|day| self.stream_hours(day))
        .flat_map(|hours| self.stream_minutes(hours))
        .flat_map(|minutes| self.stream_seconds(minutes))
        .flat_map(|seconds| self.stream_content(seconds))
        .flat_map(|sets| self.stream_sets(sets))
    }

    fn stream_years(&self, index: Option<Cid>) -> impl Stream<Item = Yearly> + '_ {
        stream::unfold(index, move |mut index| async move {
            match index {
                Some(cid) => {
                    index = None;

                    match self.ipfs.dag_get::<&str, Yearly>(cid, None).await {
                        Ok(years) => Some((years, index)),
                        Err(_) => None,
                    }
                }
                None => None,
            }
        })
    }

    fn stream_months(&self, years: Yearly) -> impl Stream<Item = Monthly> + '_ {
        stream::unfold(years.year.into_values().rev(), move |mut iter| async move {
            match iter.next() {
                Some(ipld) => match self.ipfs.dag_get::<&str, Monthly>(ipld.link, None).await {
                    Ok(months) => Some((months, iter)),
                    Err(_) => None,
                },
                None => None,
            }
        })
    }

    fn stream_days(&self, months: Monthly) -> impl Stream<Item = Daily> + '_ {
        stream::unfold(
            months.month.into_values().rev(),
            move |mut iter| async move {
                match iter.next() {
                    Some(ipld) => match self.ipfs.dag_get::<&str, Daily>(ipld.link, None).await {
                        Ok(days) => Some((days, iter)),
                        Err(_) => None,
                    },
                    None => None,
                }
            },
        )
    }

    fn stream_hours(&self, days: Daily) -> impl Stream<Item = Hourly> + '_ {
        stream::unfold(days.day.into_values().rev(), move |mut iter| async move {
            match iter.next() {
                Some(ipld) => match self.ipfs.dag_get::<&str, Hourly>(ipld.link, None).await {
                    Ok(hours) => Some((hours, iter)),
                    Err(_) => None,
                },
                None => None,
            }
        })
    }

    fn stream_minutes(&self, hours: Hourly) -> impl Stream<Item = Minutes> + '_ {
        stream::unfold(hours.hour.into_values().rev(), move |mut iter| async move {
            match iter.next() {
                Some(ipld) => match self.ipfs.dag_get::<&str, Minutes>(ipld.link, None).await {
                    Ok(minutes) => Some((minutes, iter)),
                    Err(_) => None,
                },
                None => None,
            }
        })
    }

    fn stream_seconds(&self, minutes: Minutes) -> impl Stream<Item = Seconds> + '_ {
        stream::unfold(
            minutes.minute.into_values().rev(),
            move |mut iter| async move {
                match iter.next() {
                    Some(ipld) => match self.ipfs.dag_get::<&str, Seconds>(ipld.link, None).await {
                        Ok(seconds) => Some((seconds, iter)),
                        Err(_) => None,
                    },
                    None => None,
                }
            },
        )
    }

    fn stream_content(&self, seconds: Seconds) -> impl Stream<Item = HashSet<IPLDLink>> + '_ {
        stream::unfold(
            seconds.second.into_values().rev(),
            move |mut iter| async move {
                match iter.next() {
                    Some(set) => Some((set, iter)),
                    None => None,
                }
            },
        )
    }

    fn stream_sets(&self, sets: HashSet<IPLDLink>) -> impl Stream<Item = Cid> + '_ {
        stream::unfold(sets.into_iter(), move |mut iter| async move {
            match iter.next() {
                Some(ipld) => Some((ipld.link, iter)),
                None => None,
            }
        })
    }

    /* // Lazily stream all the comments for some content on the channel
    pub async fn stream_comments(
        &self,
        channel: ChannelMetadata,
        content_cid: Cid,
    ) -> impl Stream<Item = Comment> + '_ {
        stream::unfold(channel, move |mut beacon| async move {
            if let Some(idx) = beacon.comment_index {
                beacon.comment_index = None;

                if let Ok(media) = self.ipfs.dag_get::<&str, Media>(content_cid, None).await {
                    let date_time = Utc.timestamp(media.timestamp(), 0);

                    let path = get_path(date_time);

                    if let Ok(mut comments) = self
                        .ipfs
                        .dag_get::<String, Comments>(idx.date_time.link, Some(path))
                        .await
                    {
                        if let Some(comments) = comments.comments.remove(&content_cid) {
                            Some((comments, beacon))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        })
        .flat_map(|comments| self.get_comments(comments))
    } */

    /* fn get_comments(&self, comments: Vec<IPLDLink>) -> impl Stream<Item = Comment> + '_ {
        stream::unfold(comments.into_iter(), move |mut iter| async move {
            if let Some(ipld) = iter.next() {
                if let Ok(comment) = self.ipfs.dag_get::<&str, Comment>(ipld.link, None).await {
                    Some((comment, iter))
                } else {
                    None
                }
            } else {
                None
            }
        })
    } */

    /* fn stream_media(&self, content: Content) -> impl Stream<Item = Media> + '_ {
        stream::unfold(content.content.into_iter(), move |mut iter| async move {
            if let Some(ipld) = iter.next() {
                if let Ok(raw_jws) = self.ipfs.dag_get::<&str, RawJWS>(ipld.link, None).await {
                    let jws: Result<JsonWebSignature, Error> = raw_jws.try_into();

                    if let Ok(jws) = jws {
                        if jws.verify().is_ok() {
                            if let Ok(media) =
                                self.ipfs.dag_get::<&str, Media>(jws.link, None).await
                            {
                                Some((media, iter))
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            } else {
                None
            }
        })
    } */
}
