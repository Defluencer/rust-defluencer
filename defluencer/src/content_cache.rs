use std::collections::HashMap;

use cid::Cid;

/// Identity, Media & Comments Cache
#[derive(Debug, PartialEq, Clone)]
pub struct ContentCache {
    identities: Vec<Cid>,
    channels: Vec<Cid>,
    media: Vec<Cid>,
    comments: Vec<Cid>,

    /// One channel to One identity
    channel_to_identity: HashMap<usize, usize>,

    /// One content to One identity
    media_to_identity: HashMap<usize, usize>,

    /// One comment to One content
    comment_to_media: HashMap<usize, usize>,

    /// One comment to One identity
    comment_to_identity: HashMap<usize, usize>,

    /// Channel indices sync with content_indices
    channel_indices: Vec<usize>,
    /// Content indices sync with channel_indices
    media_indices: Vec<usize>,
}

impl ContentCache {
    pub fn create() -> Self {
        Self {
            identities: Default::default(),
            channels: Default::default(),
            media: Default::default(),
            comments: Default::default(),

            channel_to_identity: Default::default(),
            media_to_identity: Default::default(),
            comment_to_media: Default::default(),
            comment_to_identity: Default::default(),

            channel_indices: Default::default(),
            media_indices: Default::default(),
        }
    }

    /// Idempotent way to add channel identity.
    pub fn insert_channel_identity(&mut self, identity: Cid, channel: Cid) {
        let channel_idx = self.channel_index(channel);
        let id_idx = self.identity_index(identity);

        self.channel_to_identity.insert(channel_idx, id_idx);
    }

    /// Idempotent way to add channel content.
    ///
    /// Note that the identity is the media creator's not the channel's
    pub fn insert_channel_media(&mut self, channel: Cid, media: Cid, identity: Cid) {
        let id_idx = self.identity_index(identity);
        let media_idx = self.media_index(media);
        let channel_idx = self.channel_index(channel);

        self.media_to_identity.insert(media_idx, id_idx);

        for (i, idx) in self.channel_indices.iter().enumerate() {
            if *idx != channel_idx {
                // skip if wrong channel
                continue;
            }

            if media_idx == self.media_indices[i] {
                // return if content already added
                return;
            }
        }

        self.channel_indices.push(channel_idx);
        self.media_indices.push(media_idx);
    }

    /// Idempotent way to add comments.
    ///
    /// Note that the identity is the comment creator's not the media's
    pub fn insert_media_comment(&mut self, media: Cid, comment: Cid, identity: Cid) {
        let comment_idx = self.comment_index(comment);
        let media_idx = self.media_index(media);
        let id_idx = self.identity_index(identity);

        self.comment_to_media.insert(comment_idx, media_idx);
        self.comment_to_identity.insert(comment_idx, id_idx);
    }

    pub fn iter_media(&self) -> impl Iterator<Item = Cid> + '_ {
        self.media.iter().copied()
    }

    pub fn iter_media_comments(&self, media: Cid) -> impl Iterator<Item = Cid> + '_ {
        self.comment_to_media
            .iter()
            .filter_map(move |(comment_idx, media_idx)| {
                if media == self.media[*media_idx] {
                    Some(self.comments[*comment_idx])
                } else {
                    None
                }
            })
    }

    pub fn media_author(&self, media: &Cid) -> Option<Cid> {
        let media_idx = self.media.iter().position(|item| item == media)?;

        let id_idx = *self.media_to_identity.get(&media_idx)?;

        let id = *self.identities.get(id_idx)?;

        Some(id)
    }

    pub fn comment_author(&self, comment: &Cid) -> Option<Cid> {
        let comment_idx = self.comments.iter().position(|item| item == comment)?;

        let id_idx = *self.comment_to_identity.get(&comment_idx)?;

        let id = *self.identities.get(id_idx)?;

        Some(id)
    }

    pub fn comments_count(&self, media: &Cid) -> usize {
        self.comment_to_media.values().fold(0, |count, media_idx| {
            if *media == self.media[*media_idx] {
                count + 1
            } else {
                count
            }
        })
    }

    fn identity_index(&mut self, identity: Cid) -> usize {
        match self.identities.iter().position(|item| *item == identity) {
            Some(idx) => idx,
            None => {
                let idx = self.identities.len();
                self.identities.push(identity);

                idx
            }
        }
    }

    fn channel_index(&mut self, channel: Cid) -> usize {
        match self.channels.iter().position(|item| *item == channel) {
            Some(idx) => idx,
            None => {
                let idx = self.channels.len();
                self.channels.push(channel);

                idx
            }
        }
    }

    fn media_index(&mut self, content: Cid) -> usize {
        match self.media.iter().position(|item| *item == content) {
            Some(idx) => idx,
            None => {
                let idx = self.media.len();
                self.media.push(content);

                idx
            }
        }
    }

    fn comment_index(&mut self, comment: Cid) -> usize {
        match self.comments.iter().position(|item| *item == comment) {
            Some(idx) => idx,
            None => {
                let idx = self.comments.len();
                self.comments.push(comment);

                idx
            }
        }
    }
}
