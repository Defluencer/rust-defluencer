use crate::errors::Error;

use chrono::{DateTime, Datelike, TimeZone, Timelike, Utc};

use cid::Cid;

use ipfs_api::IpfsService;

use linked_data::{
    beacon::Beacon,
    comments::{Comment, Comments},
    content::{Content, Media},
    indexes::*,
    media::{
        blog::{FullPost, MicroPost},
        mime_type::MimeTyped,
        video::{DayNode, HourNode, MinuteNode, VideoMetadata},
    },
    IPLDLink,
};

use serde::Serialize;

type MarkdownCid = Cid;
type ImageCid = Cid;
type VideoCid = Cid;

pub struct IPNSUser {
    ipfs: IpfsService,
    key: String,
}

impl IPNSUser {
    pub fn new(ipfs: IpfsService, key: String) -> Self {
        Self { ipfs, key }
    }

    pub fn update_display_name(&self) {
        todo!()
    }

    pub fn update_avatar(&self) {
        todo!()
    }

    pub fn update_micro_blog_post(&self) {
        todo!()
    }

    pub fn update_blog_post(&self) {
        todo!()
    }

    pub fn update_video(&self) {
        todo!()
    }

    pub fn delete_content(&self) {
        todo!()
    }

    pub fn remove_comment(&self) {
        todo!()
    }

    pub fn repair_content(&self) {
        todo!()
    }

    pub fn add_friend(&self) {
        todo!()
    }

    pub fn remove_friend(&self) {
        todo!()
    }

    pub fn update_live_settings(&self) {
        todo!()
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

        let comment_cid = self.ipfs.dag_put(&comment).await?;
        self.ipfs.pin_add(comment_cid, false).await?;

        let (beacon_cid, mut beacon): (Cid, Beacon) = self.ipfs.ipns_get(self.key.clone()).await?;

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

        let list_cid = self.ipfs.dag_put(&list).await?;

        let index_cid = self
            .update_date_time_index(date_time, beacon.comments.date_time, list_cid)
            .await?;

        beacon.content.date_time = Some(index_cid.into());

        self.ipfs
            .ipns_update(self.key.clone(), beacon_cid, &beacon)
            .await?;

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
            None => return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput).into()),
        };

        if !(mime_type == "image/png" || mime_type == "image/jpeg") {
            return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput).into());
        };

        let file = tokio::fs::File::open(path).await?;
        let stream = tokio_util::io::ReaderStream::new(file);
        let cid = self.ipfs.add(stream).await?;

        let mime_typed = MimeTyped {
            mime_type,
            data: either::Either::Left(cid.into()),
        };

        let cid = self.ipfs.dag_put(&mime_typed).await?;

        Ok(cid)
    }

    /// Add a markdown file to IPFS and return the CID
    #[cfg(not(target_arch = "wasm32"))]
    pub async fn add_markdown(&self, path: &std::path::Path) -> Result<MarkdownCid, Error> {
        let mime_type = match mime_guess::MimeGuess::from_path(path).first_raw() {
            Some(mime) => mime.to_owned(),
            None => return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput).into()),
        };

        if mime_type != "text/markdown" {
            return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput).into());
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
            return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput).into());
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

        let cid = self.ipfs.dag_put(&mime_typed).await?;

        Ok(cid)
    }

    /// Add a markdown file to IPFS and return the CID
    #[cfg(target_arch = "wasm32")]
    pub async fn add_markdown(&self, file: web_sys::File) -> Result<MarkdownCid, Error> {
        use futures::AsyncReadExt;
        use wasm_bindgen::JsCast;

        if file.type_() != "text/markdown" {
            return Err(std::io::Error::from(std::io::ErrorKind::InvalidInput).into());
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

    async fn add_content<T>(&self, date_time: DateTime<Utc>, metadata: &T) -> Result<Cid, Error>
    where
        T: ?Sized + Serialize,
    {
        let content_cid = self.ipfs.dag_put(metadata).await?;
        self.ipfs.pin_add(content_cid, true).await?;

        let (beacon_cid, mut beacon): (Cid, Beacon) = self.ipfs.ipns_get(self.key.clone()).await?;

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

        list.content.push(content_cid.into());
        let list_cid = self.ipfs.dag_put(&list).await?;

        let index_cid = self
            .update_date_time_index(date_time, beacon.content.date_time, list_cid)
            .await?;

        beacon.content.date_time = Some(index_cid.into());

        self.ipfs
            .ipns_update(self.key.clone(), beacon_cid, &beacon)
            .await?;

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
        let seconds_cid = self.ipfs.dag_put(&seconds).await?;

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
        let minutes_cid = self.ipfs.dag_put(&minutes).await?;

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
        let hours_cid = self.ipfs.dag_put(&hours).await?;

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
        let days_cid = self.ipfs.dag_put(&days).await?;

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
        let months_cid = self.ipfs.dag_put(&months).await?;

        let mut years: Yearly = if let Some(index) = index {
            self.ipfs
                .dag_get(index.link, Option::<&str>::None)
                .await
                .unwrap_or_default()
        } else {
            Yearly::default()
        };

        years.year.insert(date_time.year(), months_cid.into());
        let years_cid = self.ipfs.dag_put(&years).await?;

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
}
