use crate::types::IPLDLink;

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Metadata for video thumbnail and playback.
///
/// Recursive pin.
#[derive(Deserialize, Serialize, PartialEq, Clone, Debug)]
pub struct Video {
    pub identity: IPLDLink,

    /// Timestamp at the time of publication in Unix time.
    pub user_timestamp: i64,

    /// Duration in seconds.
    pub duration: f64,

    /// Link to Raw node of thumbnail image.
    pub image: IPLDLink,

    /// Link to TimecodeNode.
    pub video: IPLDLink,

    /// Title of this video.
    pub title: String,
}

/// Timecode structure root CID.
#[derive(Serialize, Deserialize, Debug)]
pub struct Timecode {
    /// ../time/..
    #[serde(rename = "time")]
    pub timecode: IPLDLink,
}

/// Links all hour nodes for multiple hours of video.
#[derive(Serialize, Deserialize, Debug)]
pub struct Day {
    /// ../time/hour/1/..
    #[serde(rename = "hour")]
    pub links_to_hours: Vec<IPLDLink>,
}

/// Links all minute nodes for 1 hour of video.
#[derive(Serialize, Deserialize, Debug)]
pub struct Hour {
    /// ../time/hour/0/minute/15/..
    #[serde(rename = "minute")]
    pub links_to_minutes: Vec<IPLDLink>,
}

/// Links all variants nodes for 1 minute of video.
#[derive(Serialize, Deserialize, Debug)]
pub struct Minute {
    /// ..time/hour/2/minute/36/second/30/..
    #[serde(rename = "second")]
    pub links_to_seconds: Vec<IPLDLink>,
}

/// Links video and chat nodes.
#[derive(Serialize, Deserialize, Debug)]
pub struct Second {
    /// ../time/hour/3/minute/59/second/48/video/..
    #[serde(rename = "video")]
    pub link_to_video: IPLDLink,

    /// ../time/hour/4/minute/27/second/14/chat/0/..
    #[serde(rename = "chat")]
    pub links_to_chat: Vec<IPLDLink>,
}

/// Links all stream variants, allowing selection of video quality.
///
/// Also link to the previous video node.
#[derive(Serialize, Deserialize, Debug)]
pub struct Segment {
    /// ../time/hour/0/minute/36/second/12/video/track/1080p60/..
    #[serde(rename = "track")]
    pub tracks: HashMap<String, IPLDLink>,

    /// ../time/hour/0/minute/36/second/12/video/setup/..
    #[serde(rename = "setup")]
    pub setup: Option<IPLDLink>,

    /// ../time/hour/0/minute/36/second/12/video/previous/..
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

    pub bandwidth: usize, // ../time/hour/0/minute/36/second/12/video/setup/track/4/bandwidth
}
