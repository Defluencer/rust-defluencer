use crate::{anchoring_systems::AnchoringSystem, errors::Error, signature_system::SignatureSystem};

use chrono::{DateTime, Datelike, TimeZone, Timelike, Utc};

use cid::Cid;

use either::Either;

use futures::{stream, Stream, StreamExt};

use ipfs_api::{responses::Codec, IpfsService};

use linked_data::{
    beacon::Beacon,
    comments::{Comment, Comments},
    content::{Content, Media},
    indexes::date_time::*,
    media::{
        blog::{FullPost, MicroPost},
        mime_type::MimeTyped,
        video::{DayNode, HourNode, MinuteNode, VideoMetadata},
    },
    IPLDLink, PeerId,
};

use serde::Serialize;

type MarkdownCid = Cid;
type ImageCid = Cid;
type VideoCid = Cid;

#[derive(Clone)]
pub struct User<T, U>
where
    T: AnchoringSystem,
    U: SignatureSystem,
{
    anchor_sys: T,
    sign_sys: U,
    ipfs: IpfsService,
}

impl<T, U> User<T, U>
where
    T: AnchoringSystem,
    U: SignatureSystem,
{
    pub fn new(ipfs: IpfsService, anchor_sys: T, sign_sys: U) -> Self {
        Self {
            ipfs,
            anchor_sys,
            sign_sys,
        }
    }

    pub async fn get_beacon(&self) -> Result<(Cid, Beacon), Error> {
        let cid = self.anchor_sys.retreive().await?;
        let beacon: Beacon = self.ipfs.dag_get(cid, Option::<&str>::None).await?;

        Ok((cid, beacon))
    }

    async fn update_beacon(&self, old_cid: Cid, beacon: &Beacon) -> Result<(), Error> {
        let new_cid = self.ipfs.dag_put(beacon, Codec::default()).await?;

        self.ipfs.pin_update(old_cid, new_cid).await?;

        self.anchor_sys.anchor(new_cid).await?;

        Ok(())
    }

    /// Update your identity data.
    pub async fn update_identity(
        &self,
        display_name: Option<String>,
        avatar: Option<ImageCid>,
    ) -> Result<(), Error> {
        let (beacon_cid, mut beacon): (Cid, Beacon) = self.get_beacon().await?;

        if let Some(name) = display_name {
            beacon.identity.display_name = name;
        }

        if let Some(avatar) = avatar {
            beacon.identity.avatar = avatar.into();
        }

        self.update_beacon(beacon_cid, &beacon).await
    }

    /// Remove a specific content.
    ///
    /// Note that this content is only remove from your list of content.
    pub async fn remove_content(&self, content_cid: Cid) -> Result<(), Error> {
        let media: Media = self.ipfs.dag_get(content_cid, Option::<&str>::None).await?;
        let date_time = Utc.timestamp(media.timestamp(), 0);

        let (beacon_cid, mut beacon): (Cid, Beacon) = self.get_beacon().await?;

        let index = match beacon.content.date_time {
            Some(index) => index,
            None => return Err(Error::RemoveContent),
        };

        let path = format!(
            "year/{}/month/{}/day/{}/hour/{}/minute/{}/second/{}",
            date_time.year(),
            date_time.month(),
            date_time.day(),
            date_time.hour(),
            date_time.minute(),
            date_time.second()
        );

        let mut contents: Content = self.ipfs.dag_get(index.link, Some(path.clone())).await?;

        if !contents.content.remove(&content_cid.into()) {
            return Err(Error::RemoveContent);
        }

        let contents_cid = self.ipfs.dag_put(&contents, Codec::default()).await?;

        let index_cid = self
            .update_date_time_index(date_time, beacon.content.date_time, contents_cid)
            .await?;

        beacon.content.date_time = Some(index_cid.into());

        self.update_beacon(beacon_cid, &beacon).await
    }

    /// Remove a specific comment.
    ///
    /// Note that this comment is only remove from your list of comments.
    pub async fn remove_comment(&self, comment_cid: Cid) -> Result<(), Error> {
        let comment: Comment = self.ipfs.dag_get(comment_cid, Option::<&str>::None).await?;
        let content_cid = comment.origin.link;

        let media: Media = self.ipfs.dag_get(content_cid, Option::<&str>::None).await?;
        let date_time = Utc.timestamp(media.timestamp(), 0);

        let (beacon_cid, mut beacon): (Cid, Beacon) = self.get_beacon().await?;

        let index = match beacon.comments.date_time {
            Some(index) => index,
            None => return Err(Error::RemoveComment),
        };

        let path = format!(
            "year/{}/month/{}/day/{}/hour/{}/minute/{}/second/{}",
            date_time.year(),
            date_time.month(),
            date_time.day(),
            date_time.hour(),
            date_time.minute(),
            date_time.second()
        );

        let mut comments: Comments = self.ipfs.dag_get(index.link, Some(path.clone())).await?;

        if !comments.comments.remove(&content_cid).is_some() {
            return Err(Error::RemoveComment);
        }

        let comments_cid = self.ipfs.dag_put(&comments, Codec::default()).await?;

        let index_cid = self
            .update_date_time_index(date_time, beacon.comments.date_time, comments_cid)
            .await?;

        beacon.comments.date_time = Some(index_cid.into());

        self.update_beacon(beacon_cid, &beacon).await
    }

    /// Follow a user.
    ///
    /// Theirs content will now be display in your feed.
    pub async fn follow(&self, user: Either<String, Cid>) -> Result<(), Error> {
        let (beacon_cid, mut beacon): (Cid, Beacon) = self.get_beacon().await?;

        let mut follows = beacon.follows.unwrap_or_default();

        let status = match user {
            Either::Left(ens) => follows.ens.insert(ens),
            Either::Right(ipns) => follows.ipns.insert(ipns),
        };

        if !status {
            return Err(Error::Follow);
        }

        beacon.follows = Some(follows);

        self.update_beacon(beacon_cid, &beacon).await
    }

    /// Unfollow a user.
    ///
    /// Theirs content will no longer be display in your feed.
    pub async fn unfollow(&self, user: Either<String, Cid>) -> Result<(), Error> {
        let (beacon_cid, mut beacon): (Cid, Beacon) = self.get_beacon().await?;

        let mut follows = match beacon.follows {
            Some(f) => f,
            None => return Err(Error::UnFollow),
        };

        let status = match user {
            Either::Left(ens) => follows.ens.remove(&ens),
            Either::Right(ipns) => follows.ipns.remove(&ipns),
        };

        if !status {
            return Err(Error::UnFollow);
        }

        beacon.follows = Some(follows);

        self.update_beacon(beacon_cid, &beacon).await
    }

    /// Update live chat & streaming settings.
    pub async fn update_live_settings(
        &self,
        peer_id: Option<PeerId>,
        video_topic: Option<String>,
        chat_topic: Option<String>,
    ) -> Result<(), Error> {
        let (beacon_cid, mut beacon): (Cid, Beacon) = self.get_beacon().await?;

        let mut live = beacon.live.unwrap_or_default();

        if let Some(peer_id) = peer_id {
            live.peer_id = peer_id;
        }

        if let Some(video_topic) = video_topic {
            live.video_topic = video_topic;
        }

        if let Some(chat_topic) = chat_topic {
            live.chat_topic = chat_topic;
        }

        beacon.live = Some(live);

        self.update_beacon(beacon_cid, &beacon).await
    }

    /// Create a new micro blog post.
    pub async fn create_micro_blog_post(&self, content: String) -> Result<Cid, Error> {
        let date_time = Utc::now();
        let timestamp = date_time.timestamp();

        let micro_post = MicroPost { timestamp, content };

        self.add_content(date_time, &micro_post).await
    }

    /// Create a new comment on the specified media.
    pub async fn create_comment(&self, origin: Cid, text: String) -> Result<Cid, Error> {
        let media: Media = self.ipfs.dag_get(origin, Option::<&str>::None).await?;
        let date_time = Utc.timestamp(media.timestamp(), 0);

        let comment = Comment {
            timestamp: Utc::now().timestamp(),
            origin: origin.into(),
            text,
        };

        let comment_cid = self.ipfs.dag_put(&comment, Codec::default()).await?;
        self.ipfs.pin_add(comment_cid, false).await?;

        let (beacon_cid, mut beacon): (Cid, Beacon) = self.get_beacon().await?;

        let mut list = if let Some(index) = beacon.comments.date_time {
            let path = format!(
                "year/{}/month/{}/day/{}/hour/{}/minute/{}/second/{}",
                date_time.year(),
                date_time.month(),
                date_time.day(),
                date_time.hour(),
                date_time.minute(),
                date_time.second()
            );

            self.ipfs
                .dag_get(index.link, Some(path.clone()))
                .await
                .unwrap_or_default()
        } else {
            Comments::default()
        };

        list.comments
            .entry(origin)
            .or_default()
            .push(comment_cid.into());

        let list_cid = self.ipfs.dag_put(&list, Codec::default()).await?;

        let index_cid = self
            .update_date_time_index(date_time, beacon.comments.date_time, list_cid)
            .await?;

        beacon.content.date_time = Some(index_cid.into());

        self.update_beacon(beacon_cid, &beacon).await?;

        Ok(comment_cid)
    }

    /// Create a new blog post.
    pub async fn create_blog_post(
        &self,
        title: String,
        image: ImageCid,
        markdown: MarkdownCid,
    ) -> Result<Cid, Error> {
        let date_time = Utc::now();
        let timestamp = date_time.timestamp();

        let full_post = FullPost {
            timestamp,
            content: markdown.into(),
            image: image.into(),
            title,
        };

        self.add_content(date_time, &full_post).await
    }

    /// Create a new video post.
    pub async fn create_video_post(
        &self,
        title: String,
        image: ImageCid,
        video: VideoCid,
    ) -> Result<Cid, Error> {
        let date_time = Utc::now();
        let timestamp = date_time.timestamp();

        let duration = self.video_duration(video).await?;

        let video_post = VideoMetadata {
            timestamp,
            image: image.into(),
            title,
            duration,
            video: video.into(),
        };

        self.add_content(date_time, &video_post).await
    }

    /// Add an image to IPFS and return the CID
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn add_image(&self, path: &std::path::Path) -> Result<ImageCid, Error> {
        let mime_type = match mime_guess::MimeGuess::from_path(path).first_raw() {
            Some(mime) => mime.to_owned(),
            None => return Err(Error::Image),
        };

        if !(mime_type == "image/png" || mime_type == "image/jpeg") {
            return Err(Error::Image);
        };

        let file = tokio::fs::File::open(path).await?;
        let stream = tokio_util::io::ReaderStream::new(file);
        let cid = self.ipfs.add(stream).await?;

        let mime_typed = MimeTyped {
            mime_type,
            data: either::Either::Left(cid.into()),
        };

        let cid = self.ipfs.dag_put(&mime_typed, Codec::default()).await?;

        Ok(cid)
    }

    /// Add a markdown file to IPFS and return the CID
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn add_markdown(&self, path: &std::path::Path) -> Result<MarkdownCid, Error> {
        let mime_type = match mime_guess::MimeGuess::from_path(path).first_raw() {
            Some(mime) => mime.to_owned(),
            None => return Err(Error::Markdown),
        };

        if mime_type != "text/markdown" {
            return Err(Error::Markdown);
        };

        let file = tokio::fs::File::open(path).await?;
        let stream = tokio_util::io::ReaderStream::new(file);

        let cid = self.ipfs.add(stream).await?;

        Ok(cid)
    }

    /// Add an image to IPFS and return the CID
    #[cfg(target_arch = "wasm32")]
    pub async fn add_image(&self, file: web_sys::File) -> Result<ImageCid, Error> {
        use futures::AsyncReadExt;
        use wasm_bindgen::JsCast;

        let mime_type = file.type_();

        if !(mime_type == "image/png" || mime_type == "image/jpeg") {
            return Err(Error::Image);
        };

        let size = file.size() as usize;

        // TODO disallow image that are too big.

        let readable_stream =
            wasm_streams::ReadableStream::from_raw(file.stream().unchecked_into());

        let mut async_read = readable_stream.into_async_read();

        let mut bytes = Vec::with_capacity(size);
        async_read.read_to_end(&mut bytes).await?;

        let mime_typed = MimeTyped {
            mime_type,
            data: either::Either::Right(bytes),
        };

        let cid = self.ipfs.dag_put(&mime_typed, Codec::default()).await?;

        Ok(cid)
    }

    /// Add a markdown file to IPFS and return the CID
    #[cfg(target_arch = "wasm32")]
    pub async fn add_markdown(&self, file: web_sys::File) -> Result<MarkdownCid, Error> {
        use futures::AsyncReadExt;
        use wasm_bindgen::JsCast;

        if file.type_() != "text/markdown" {
            return Err(Error::Markdown);
        };

        let size = file.size() as usize;

        let readable_stream =
            wasm_streams::ReadableStream::from_raw(file.stream().unchecked_into());

        let mut async_read = readable_stream.into_async_read();

        let mut bytes = Vec::with_capacity(size);
        async_read.read_to_end(&mut bytes).await?;
        let bytes = bytes::Bytes::from(bytes);

        let cid = self.ipfs.add(bytes).await?;

        Ok(cid)
    }

    async fn add_content<V>(&self, date_time: DateTime<Utc>, metadata: &V) -> Result<Cid, Error>
    where
        V: ?Sized + Serialize,
    {
        let content_cid = self.ipfs.dag_put(metadata, Codec::default()).await?;
        let signed_cid = self.sign_sys.sign(content_cid).await?;
        self.ipfs.pin_add(signed_cid, true).await?;

        let (beacon_cid, mut beacon): (Cid, Beacon) = self.get_beacon().await?;

        let mut list = if let Some(index) = beacon.content.date_time {
            let path = format!(
                "year/{}/month/{}/day/{}/hour/{}/minute/{}/second/{}",
                date_time.year(),
                date_time.month(),
                date_time.day(),
                date_time.hour(),
                date_time.minute(),
                date_time.second()
            );

            self.ipfs
                .dag_get(index.link, Some(path.clone()))
                .await
                .unwrap_or_default()
        } else {
            Content::default()
        };

        list.content.insert(signed_cid.into());
        let list_cid = self.ipfs.dag_put(&list, Codec::default()).await?;

        let index_cid = self
            .update_date_time_index(date_time, beacon.content.date_time, list_cid)
            .await?;

        beacon.content.date_time = Some(index_cid.into());

        self.update_beacon(beacon_cid, &beacon).await?;

        Ok(content_cid)
    }

    async fn update_date_time_index(
        &self,
        date_time: DateTime<Utc>,
        index: Option<IPLDLink>,
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
                .dag_get(index.link, Some(path.clone()))
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
                .dag_get(index.link, Some(path.clone()))
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
                .dag_get(index.link, Some(path.clone()))
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
                .dag_get(index.link, Some(path.clone()))
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
                .dag_get(index.link, Some(path.clone()))
                .await
                .unwrap_or_default()
        } else {
            Monthly::default()
        };

        months.month.insert(date_time.month(), days_cid.into());
        let months_cid = self.ipfs.dag_put(&months, Codec::default()).await?;

        let mut years: Yearly = if let Some(index) = index {
            self.ipfs
                .dag_get(index.link, Option::<&str>::None)
                .await
                .unwrap_or_default()
        } else {
            Yearly::default()
        };

        years.year.insert(date_time.year(), months_cid.into());
        let years_cid = self.ipfs.dag_put(&years, Codec::default()).await?;

        Ok(years_cid)
    }

    async fn video_duration(&self, video: Cid) -> Result<f64, Error> {
        let days: DayNode = self.ipfs.dag_get(video, Some("/time")).await?;

        let mut duration = 0.0;

        for (i, ipld) in days.links_to_hours.iter().enumerate().rev().take(1) {
            duration += (i * 3600) as f64; // 3600 second in 1 hour

            let hours: HourNode = self.ipfs.dag_get(ipld.link, Option::<&str>::None).await?;

            for (i, ipld) in hours.links_to_minutes.iter().enumerate().rev().take(1) {
                duration += (i * 60) as f64; // 60 second in 1 minute

                let minutes: MinuteNode =
                    self.ipfs.dag_get(ipld.link, Option::<&str>::None).await?;

                duration += (minutes.links_to_seconds.len() - 1) as f64;
            }
        }

        Ok(duration)
    }

    /// Lazily stream media starting from newest.
    pub fn media_feed(&self, beacon: Beacon) -> impl Stream<Item = Content> + '_ {
        stream::unfold(beacon, move |mut beacon| async move {
            match beacon.content.date_time {
                Some(ipld) => {
                    beacon.content.date_time = None;

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
        //TODO verify JWS
        //.flat_map(|content| self.stream_media(content))
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

    /* fn stream_media(&self, content: Content) -> impl Stream<Item = Media> + '_ {
        stream::unfold(content.content.into_iter(), move |mut iter| async move {
            match iter.next() {
                Some(ipld) => match self.ipfs.dag_get::<&str, Media>(ipld.link, None).await {
                    Ok(media) => Some((media, iter)),
                    Err(_) => None,
                },
                None => None,
            }
        })
    } */
}
