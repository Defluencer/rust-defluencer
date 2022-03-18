use std::collections::HashSet;

use crate::{
    comments::Comment,
    media::{
        blog::{FullPost, MicroPost},
        video::VideoMetadata,
    },
    IPLDLink,
};

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Default, Debug, Clone, PartialEq)]
pub struct Content {
    pub content: HashSet<IPLDLink>,
}

#[derive(Deserialize, PartialEq, Clone, Debug)]
#[serde(untagged)]
pub enum Media {
    MicroBlog(MicroPost),
    Blog(FullPost),
    Video(VideoMetadata),
    Comment(Comment),
}

impl Media {
    pub fn timestamp(&self) -> i64 {
        match self {
            Media::MicroBlog(metadata) => metadata.timestamp,
            Media::Blog(metadata) => metadata.timestamp,
            Media::Video(metadata) => metadata.timestamp,
            Media::Comment(metadata) => metadata.timestamp,
        }
    }
}
