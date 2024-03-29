use crate::types::IPLDLink;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Metadata of a video post.
#[derive(Deserialize, Serialize, PartialEq, Clone, Debug, Default)]
pub struct Video {
    /// Creator identity link
    pub identity: IPLDLink,

    /// Timestamp at the time of publication in Unix time.
    pub user_timestamp: i64,

    /// Link to video.
    pub video: IPLDLink,

    /// Title of this video.
    pub title: String,

    /// Duration in seconds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<f64>,

    /// Link to thumbnail image.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<IPLDLink>,
}

/// Timecode structure root CID.
#[derive(Serialize, Deserialize, Debug)]
pub struct Timecode {
    /// Path ../time/..
    #[serde(rename = "time")]
    pub timecode: IPLDLink,
}

/// Links all hour nodes for multiple hours of video.
#[derive(Serialize, Deserialize, Debug)]
pub struct Day {
    /// Path ../time/hour/1/..
    #[serde(rename = "hour")]
    pub links_to_hours: Vec<IPLDLink>,
}

/// Links all minute nodes for 1 hour of video.
#[derive(Serialize, Deserialize, Debug)]
pub struct Hour {
    /// Path ../time/hour/0/minute/15/..
    #[serde(rename = "minute")]
    pub links_to_minutes: Vec<IPLDLink>,
}

/// Links all variants nodes for 1 minute of video.
#[derive(Serialize, Deserialize, Debug)]
pub struct Minute {
    /// Path ..time/hour/2/minute/36/second/30/..
    #[serde(rename = "second")]
    pub links_to_seconds: Vec<IPLDLink>,
}

/// Links video and chat nodes.
#[derive(Serialize, Deserialize, Debug)]
pub struct Second {
    /// Path ../time/hour/3/minute/59/second/48/video/..
    #[serde(rename = "video")]
    pub link_to_video: IPLDLink,

    /// Path ../time/hour/4/minute/27/second/14/chat/0/..
    #[serde(rename = "chat")]
    pub links_to_chat: Vec<IPLDLink>,
}

/// Links all stream variants, allowing selection of video quality.
///
/// Also link to the previous video node.
#[derive(Serialize, Deserialize, Debug)]
pub struct Segment {
    /// Path ../time/hour/0/minute/36/second/12/video/track/1080p60/..
    #[serde(rename = "track")]
    pub tracks: HashMap<String, IPLDLink>,

    /// Path ../time/hour/0/minute/36/second/12/video/setup/..
    #[serde(rename = "setup")]
    pub setup: Option<IPLDLink>,

    /// Path ../time/hour/0/minute/36/second/12/video/previous/..
    #[serde(rename = "previous")]
    pub previous: Option<IPLDLink>,
}

/// Contains initialization data for video stream.
#[derive(Serialize, Deserialize, Debug)]
pub struct Setup {
    /// Tracks sorted from lowest to highest bitrate.
    #[serde(rename = "track")]
    pub tracks: Vec<Track>, // ../time/hour/0/minute/36/second/12/video/setup/track/0/..
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Track {
    pub name: String,  // ../time/hour/0/minute/36/second/12/video/setup/track/2/name
    pub codec: String, // ../time/hour/0/minute/36/second/12/video/setup/track/3/codec

    #[serde(rename = "initseg")]
    pub initialization_segment: IPLDLink, // ../time/hour/0/minute/36/second/12/video/setup/track/1/initseg

    pub bandwidth: u64, // ../time/hour/0/minute/36/second/12/video/setup/track/4/bandwidth
}
