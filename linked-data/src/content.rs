use crate::{
    media::{
        blog::{FullPost, MicroPost},
        video::VideoMetadata,
    },
    IPLDLink,
};

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Default, Debug, Clone, PartialEq)]
pub struct ContentIndexing {
    pub date_time: Option<IPLDLink>,
}

#[derive(Deserialize, Serialize, Default, Debug, Clone, PartialEq)]
pub struct Content {
    pub content: Vec<IPLDLink>,
}

#[derive(Deserialize, PartialEq, Clone)]
#[serde(untagged)]
pub enum Media {
    Statement(MicroPost),
    Blog(FullPost),
    Video(VideoMetadata),
}

impl Media {
    pub fn timestamp(&self) -> i64 {
        match self {
            Media::Statement(metadata) => metadata.timestamp,
            Media::Blog(metadata) => metadata.timestamp,
            Media::Video(metadata) => metadata.timestamp,
        }
    }
}
