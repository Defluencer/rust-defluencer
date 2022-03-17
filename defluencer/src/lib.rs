pub mod anchors;
pub mod channel;
pub mod content_cache;
pub mod errors;
pub mod moderation_cache;
pub mod signatures;
pub mod user;
pub mod utils;

use chrono::{TimeZone, Utc};
use cid::Cid;

use errors::Error;

use futures::{stream, Stream, StreamExt};

use linked_data::{
    channel::ChannelMetadata,
    comments::{Comment, Comments},
    content::{Content, Media},
    indexes::date_time::{Daily, Hourly, Minutes, Monthly, Seconds, Yearly},
    signature::RawJWS,
    IPLDLink,
};

use ipfs_api::IpfsService;

use signatures::dag_jose::JsonWebSignature;
use utils::get_path;

pub struct Defluencer {
    ipfs: IpfsService,
}

impl Defluencer {
    pub fn new() -> Self {
        let ipfs = IpfsService::default();

        Self { ipfs }
    }

    pub async fn stream_comments(
        &self,
        content_cid: Cid,
        origin: ChannelMetadata,
    ) -> impl Stream<Item = Comment> + '_ {
        stream::unfold(origin, move |mut beacon| async move {
            if let Some(index) = beacon.comment_index.date_time {
                beacon.comment_index.date_time = None;

                if let Ok(media) = self.ipfs.dag_get::<&str, Media>(content_cid, None).await {
                    let date_time = Utc.timestamp(media.timestamp(), 0);

                    let path = get_path(date_time);

                    if let Ok(mut comments) = self
                        .ipfs
                        .dag_get::<String, Comments>(index.link, Some(path))
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
    }

    fn get_comments(&self, comments: Vec<IPLDLink>) -> impl Stream<Item = Comment> + '_ {
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
    }

    /// Lazily stream media starting from newest.
    pub fn stream_media_feed(&self, origin: ChannelMetadata) -> impl Stream<Item = Media> + '_ {
        stream::unfold(origin, move |mut beacon| async move {
            match beacon.content_index.date_time {
                Some(ipld) => {
                    beacon.content_index.date_time = None;

                    Some((Some(ipld.link), beacon))
                }
                None => None,
            }
        })
        .flat_map(|index| self.stream_years(index))
        .flat_map(|year| self.stream_months(year))
        .flat_map(|month| self.stream_days(month))
        .flat_map(|day| self.stream_hours(day))
        .flat_map(|hours| self.stream_minutes(hours))
        .flat_map(|minutes| self.stream_seconds(minutes))
        .flat_map(|seconds| self.stream_content(seconds))
        .flat_map(|content| self.stream_media(content))

        //TODO verify that JWS match the media content
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

    fn stream_content(&self, seconds: Seconds) -> impl Stream<Item = Content> + '_ {
        stream::unfold(
            seconds.second.into_values().rev(),
            move |mut iter| async move {
                match iter.next() {
                    Some(ipld) => match self.ipfs.dag_get::<&str, Content>(ipld.link, None).await {
                        Ok(content) => Some((content, iter)),
                        Err(_) => None,
                    },
                    None => None,
                }
            },
        )
    }

    fn stream_media(&self, content: Content) -> impl Stream<Item = Media> + '_ {
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
    }

    /* /// Create a new IPNS user on this IPFS node.
    ///
    /// Names are converted to title case.
    pub async fn create_ipns_user(
        &self,
        display_name: impl Into<Cow<'static, str>>,
    ) -> Result<IPNSUser, Error> {
        let name = display_name.into();
        let key_name = name.to_snake_case();
        let display_name = name.to_title_case();

        let avatar = Cid::default().into(); //TODO provide a default avatar Cid

        let beacon = Beacon {
            identity: Identity {
                display_name,
                avatar,
            },
            content: Default::default(),
            comments: Default::default(),
            live: Default::default(),
            follows: Default::default(),
            bans: Default::default(),
            mods: Default::default(),
        };

        //TODO generate ed25519 key pair

        //TODO format key for import into IPFS.

        //TODO use bip-39 to export the key as passphrase.

        let KeyPair { id: _, name } = self.ipfs.key_import(key_name, key_pair).await?;

        let user = IPNSUser::new(
            self.ipfs.clone(),
            IPNSAnchor::new(self.ipfs.clone(), name.clone()),
            EdDSASigner::new(self.ipfs.clone(), key_pair),
        );

        self.ipfs.ipns_put(name, false, &beacon).await?;

        Ok(user)
    } */

    /* /// Search this IPFS node for users.
    ///
    /// IPNS records that resolve to beacons are considered local users.
    pub async fn get_ipns_users(&self) -> Result<Vec<IPNSUser>, Error> {
        let list = self.ipfs.key_list().await?;

        let (names, keys): (Vec<String>, Vec<Cid>) = list.into_iter().unzip();

        let futs: Vec<_> = keys
            .into_iter()
            .map(|key| self.ipfs.name_resolve(key))
            .collect();

        let results: Vec<Result<Cid, Error>> = future::join_all(futs).await;

        let list: Vec<(String, _)> = results
            .into_iter()
            .zip(names.into_iter())
            .filter_map(|(result, name)| match result {
                Ok(cid) => Some((name, self.ipfs.dag_get::<&str, Beacon>(cid, Option::None))),
                _ => None,
            })
            .collect();

        let (names, futs): (Vec<String>, Vec<_>) = list.into_iter().unzip();

        let results: Vec<Result<Beacon, Error>> = future::join_all(futs).await;

        let users: Vec<IPNSUser> = results
            .into_iter()
            .zip(names.into_iter())
            .filter_map(|(result, name)| match result {
                Ok(_) => Some(IPNSUser::new(
                    self.ipfs.clone(),
                    IPNSAnchor::new(self.ipfs.clone(), name),
                    IPNSSignature::new(self.ipfs.clone(), key_pair),
                )),
                _ => None,
            })
            .collect();

        Ok(users)
    } */
}
