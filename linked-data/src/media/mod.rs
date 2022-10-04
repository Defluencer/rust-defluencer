pub mod blog;
pub mod chat;
pub mod video;

use serde::Deserialize;

use crate::{comments::Comment, types::IPLDLink};

use self::{blog::BlogPost, video::Video};

#[derive(Deserialize, PartialEq, Clone, Debug)]
#[serde(untagged)]
pub enum Media {
    Blog(BlogPost),
    Video(Video),
    Comment(Comment),
}

impl Media {
    pub fn user_timestamp(&self) -> i64 {
        match self {
            Media::Blog(metadata) => metadata.user_timestamp,
            Media::Video(metadata) => metadata.user_timestamp,
            Media::Comment(metadata) => metadata.user_timestamp,
        }
    }

    pub fn identity(&self) -> IPLDLink {
        match self {
            Media::Blog(metadata) => metadata.identity,
            Media::Video(metadata) => metadata.identity,
            Media::Comment(metadata) => metadata.identity,
        }
    }
}
