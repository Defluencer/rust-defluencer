use std::collections::HashMap;

use linked_data::identity::Identity;

use cid::Cid;

/// Identity, Media & Comments Cache
#[derive(Debug, PartialEq, Clone)]
pub struct ContentCache {
    /// Comments CIDs
    comments: Vec<Cid>,

    /// Comment index mapped to channel index.
    comment_to_channel: HashMap<usize, usize>,

    /// Channel CIDs.
    channels: Vec<Cid>,

    /// Channel index mapped to identity index.
    channel_to_identity: HashMap<usize, usize>,

    /// Identity CIDs
    identities: Vec<Cid>,

    /// Comment index mapped to media index.
    comment_to_media: HashMap<usize, usize>,

    /// Media CIDs.
    media_content: Vec<Cid>,

    /// Media index mapped to channel index.
    media_to_channel: HashMap<usize, usize>,
}

impl ContentCache {
    pub fn create() -> Self {
        Self {
            comments: Vec::with_capacity(100),
            comment_to_channel: HashMap::with_capacity(100),
            channels: Vec::with_capacity(100),
            channel_to_identity: HashMap::with_capacity(100),
            identities: Vec::with_capacity(100),
            comment_to_media: HashMap::with_capacity(100),
            media_content: Vec::with_capacity(100),
            media_to_channel: HashMap::with_capacity(100),
        }
    }

    /* /// Idempotent way to add a user's identity.
    pub fn insert_identity(&mut self, channel: Cid, identity: Identity) {
        let channel_idx = match self.channels.iter().position(|item| *item == channel) {
            Some(idx) => idx,
            None => {
                let idx = self.channels.len();
                self.channels.push(channel);

                idx
            }
        };

        match self.channel_to_identity.get(&channel_idx) {
            Some(name_idx) => {
                self.names[*name_idx] = identity.display_name;
                self.avatars[*name_idx] = identity.avatar.link;
            }
            None => {
                let name_idx = self.names.len();

                self.names.push(identity.display_name);
                self.avatars.push(identity.avatar.link);

                self.channel_to_identity.insert(channel_idx, name_idx);
            }
        }
    } */

    /* /// Idempotent way to add user media content.
    pub fn insert_media_content(&mut self, channel: Cid, content: Content) {
        let channel_idx = match self.channels.iter().position(|item| *item == channel) {
            Some(idx) => idx,
            None => {
                let idx = self.channels.len();
                self.channels.push(channel);

                idx
            }
        };

        for ipld in content.content.into_iter() {
            if !self.media_content.contains(&ipld.link) {
                let idx = self.media_content.len();

                self.media_content.push(ipld.link);

                self.media_to_channel.insert(idx, channel_idx);
            }
        }
    } */

    /* pub fn iter_media_content(&self) -> impl Iterator<Item = &Cid> {
        self.media_content.iter()
    } */

    /* pub fn media_content_author(&self, media: &Cid) -> Option<&str> {
        let media_idx = self.media_content.iter().position(|item| *item == *media)?;

        let channel_idx = self.media_to_channel.get(&media_idx)?;

        let name_idx = self.channel_to_identity.get(channel_idx)?;

        let name = self.names.get(*name_idx)?;

        Some(name)
    } */

    /* /// Idempotent way to add user comments.
    pub fn insert_comments(&mut self, channel: Cid, comments: Comments) {
        let channel_idx = match self.channels.iter().position(|item| *item == channel) {
            Some(idx) => idx,
            None => {
                let idx = self.channels.len();
                self.channels.push(channel);

                idx
            }
        };

        for (media_cid, comments) in comments.comments.into_iter() {
            let media_idx = match self
                .media_content
                .iter()
                .position(|item| *item == media_cid)
            {
                Some(idx) => idx,
                None => {
                    let idx = self.media_content.len();

                    self.media_content.push(media_cid);

                    idx
                }
            };

            for comment in comments.into_iter() {
                if !self.comments.contains(&comment.link) {
                    let comment_idx = self.comments.len();

                    self.comments.push(comment.link);

                    self.comment_to_channel.insert(comment_idx, channel_idx);

                    self.comment_to_media.insert(comment_idx, media_idx);
                }
            }
        }
    } */

    /* pub fn iter_comments(&self, media: &Cid) -> Option<impl Iterator<Item = &Cid>> {
        let media_idx = self.media_content.iter().position(|item| *item == *media)?;

        let iterator = self
            .comment_to_media
            .iter()
            .filter_map(move |(comment_idx, idx)| {
                if *idx == media_idx {
                    self.comments.get(*comment_idx)
                } else {
                    None
                }
            });

        Some(iterator)
    } */

    /* pub fn comment_author(&self, comment: &Cid) -> Option<&str> {
        let comment_idx = self.comments.iter().position(|item| *item == *comment)?;

        let channel_idx = self.comment_to_channel.get(&comment_idx)?;

        let name_idx = self.channel_to_identity.get(channel_idx)?;

        let name = self.names.get(*name_idx)?;

        Some(name)
    } */

    /* pub fn comments_count(&self, media: &Cid) -> usize {
        let media_idx = match self.media_content.iter().position(|item| *item == *media) {
            Some(idx) => idx,
            None => return 0,
        };

        self.comment_to_media.values().fold(
            0,
            |count, idx| {
                if *idx == media_idx {
                    count + 1
                } else {
                    count
                }
            },
        )
    } */
}
