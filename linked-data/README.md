# Defluencer Protocol IPLD Schemas

## Channel

The root of the DAG representing all information related to a channel.

```
advanced ChronologicalMap { ADL "" } #TODO add specifications
advanced ShardedMap { ADL "HAMT/v1" }

type DateTime map [Time:&Media] using ChronologicalMap

type Comments map [&Comment:&Comment] using ShardedMap # Keys are hashes of Cids
type CommentIndex map [&Media:&Comments] using ShardedMap # Keys are hashes of Cids

type ChannelMetadata struct {
    seq Int # Growth only counter. Increment every update.
    identity &Identity
    content_index optional &DateTime
    comment_index optional &CommentIndex
    live optional &LiveSettings
    folows optional &Follows
}
```

## Identity

User or Channel identity information.

```
type Identity struct {
    display_name String 
    avatar &MimeTyped
    channel_ipns optional String
}
```

## Media

```
type Media union {
    | &MicroPost link
    | &FullPost link
    | &Video link
    | &Comment link
} representation kinded

type MicroPost struct {
    identity &Identity
    user_timestamp Int # Unix Time
    content String
}

type FullPost struct {
    identity &Identity
    user_timestamp Int # Unix Time
    content Link # Link to markdown file
    image &MimeTyped
    title String
}

type Video struct {
    identity &Identity
    user_timestamp Int # Unix time
    duration Float
    image &MimeTyped # Poster & Thumbnail
    video &TimeCode
    title String
}

type Comment struct {
    identity &Identity
    user_timestamp Int # Unix Time
    origin String # CID as string to prevent recursive pinning.
    text String
}
```

## Live Streaming Settings

```
type LiveSettings struct {
    peer_id String
    video_topic String
    archiving Bool
    chat_topic optional String
    bans optional &Bans
    mods optional &Moderators
}
```

## Follows

List of followees used to crawl the social web.

```
type Follows struct {
  followees [String] # IPNS address of channels
}
```

## Chat Moderation

```
type ETHAddress bytes # Ethereum address are 20 bytes.

type Bans struct {
    banned_addrs [ETHAddress]
}

type Moderators struct {
    moderator_addrs [ETHAddress]
}
```

## Chat

Display Name and GossipSub Peer ID are signed using Ethereum Keys then the address, name, id, and signature are added to IPFS returning a CID.
When receiving a pubsub message this CID is used to fetch and verify that IDs matches and signature is correct.

```
type Text string

type Ban struct {
    ban_peer String
    ban_addrs ETHAddress
}

type Moderator struct {
    mod_peer String
    mod_addrs ETHAddress
}

type MessageType union {
    | Text string
    | Ban map   # ETH address and peer Id of the person to ban.
    | Moderator map # The ETH address and peer Id of the new moderator.
} representation kinded

type ChatMessage struct {
    name String
    
    message MessageType

    signature Link # Link to DAG-JOSE block linking to peer id
}
```

## Streams

A video node contains links to segments of videos of all quality. As video is streamed, new video nodes are created and linked to previous ones.
A special node contains the stream setup data; codecs, qualities, initialization segments, etc...

```
type Segment struct {
    tracks {String:Link} # Name of the track egg "audio" or "1080p60" & link to video segment data
    setup optional &Setup
    previous optional &Segment
}

type Setup struct {
    tracks [Track] # Sorted from lowest to highest bitrate.
}

type Track struct {
    name String
    codec String # Mime type
    initseg Link # Link to the initialization segment data
    bandwidth Int
}
```

## Videos

Timecode nodes are created at specific intervals and linked together to form a structure around the video allowing it to be addressable by timecode.
Video clips are subgraph of the whole.

```
type TimeCode struct {
    time &Day
}

type Day struct {
    hour [&Hour]
}

type Hour struct {
    minute [&Minute]
}

type Minute struct {
    second [&Second]
}

type Second struct {
    video &Segment
    chat [&ChatMessage]
}
```

## Mime Typed Data

Mime typed data.

If the data fit in a single block inline otherwise link to it.

```
type MimeTyped struct {
    mime_type String
    data InlineOrLink
}

type InlineOrLink union {
  | Inline bytes
  | Linked link
} representation kinded

type Inline Bytes
type Linked Link
```

----

## License
Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution
Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.