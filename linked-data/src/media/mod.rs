pub mod blog;
pub mod chat;
pub mod mime_type;
pub mod video;

use serde::Deserialize;

use crate::comments::Comment;

use self::{
    blog::{FullPost, MicroPost},
    video::Video,
};

#[derive(Deserialize, PartialEq, Clone, Debug)]
#[serde(untagged)]
pub enum Media {
    MicroBlog(MicroPost),
    Blog(FullPost),
    Video(Video),
    Comment(Comment),
}

impl Media {
    pub fn user_timestamp(&self) -> i64 {
        match self {
            Media::MicroBlog(metadata) => metadata.user_timestamp,
            Media::Blog(metadata) => metadata.user_timestamp,
            Media::Video(metadata) => metadata.user_timestamp,
            Media::Comment(metadata) => metadata.user_timestamp,
        }
    }
}
