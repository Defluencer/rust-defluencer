# Linked Data

Defluencer IPLD schemas.

## Beacon

Metadata, content and comment indexes.

### IPLD Schemas
```
type Beacon struct {
    identity Identity
    content_feed ContentIndexing
    comments CommentIndexing
    live optional LiveSettings
    friends optional Follows
    bans optional &Bans
    mods optional &Moderators
}
```
## Identity

User name and avatar.

### IPLD Schemas
```
type Identity struct {
    display_name String 
    avatar &MimeTyped
}
```

## Content

List of indexes for content search.

### IPLD Schemas
```
type ContentIndexing struct {
    date_time optional &Yearly
}

type Content struct {
    content [&Media]
}

type Media union {
    | &MicroPost link
    | &FullPost link
    | &VideoMetadata link
} representation kinded
```
## Comments

List of indexes for comment search.

The date & time of the content being commented on is used.

### IPLD Schemas
```
type CommentIndexing {
    date_time optional &Yearly
}

type Comments struct {
    "comments": {String:[&Comment]} # Keys are CIDs of the content being commented on.
}

type Comment struct {
    timestamp Int # Unix Time
    origin Link # CID of content being commented on.
    comment String
}
```
## Indexes

Only date & time ATM. Other indexing methods could be added.

### IPLD Schemas
```
type Yearly struct {
  year [Int:&Monthly] 
}

type Monthly struct {
  month [Int:&Daily] 
}

type Daily struct {
  day [Int:&Hourly] 
}

type Hourly struct {
  hour [Int:&Minutes] 
}

type Minutes struct {
  minute [Int:&Seconds] 
}

type Seconds struct {
  second [Int:Link] # Can link to content or comments 
}

```
## Live Settings

Metadata needed for live streaming.

```
type LiveSettings struct {
    peer_id String
    video_topic String
    chat_topic String
}
```
## Follows

A list of user you follow.

### IPLD Schemas
```
type Follows struct {
  ens [String] # ENS domain names
  ipns [String] # IPNS addresses
}
```
## Chat Moderation

Moderator can send ban/mod messages via PubSub.
The message should be signed.
The schemas are list of banned users and moderators.

### IPLD Schemas
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

### IPLD Schemas
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

type ChatID struct {
    peer_id String
    name String
}

type SignedMessage {
    address ETHAddress

    data ChatID

    signature Bytes # 65 bytes
}

type ChatMessage struct {
    message MessageType

    signature &SignedMessage
}
```

## Streams

A video node contains links to segments of videos of all quality. As video is streamed, new video nodes are created and linked to previous ones.
A special node contains the stream setup data; codecs, qualities, initialization segments, etc...

### IPLD Schemas
```
type VideoNode struct {
    tracks {String:Link} # Name of the track egg "audio" or "1080p60" & link to video segment data
    setup optional &SetupNode
    previous optional &VideoNode
}

type SetupNode struct {
    tracks [Track] # Sorted from lowest to highest bitrate.
}

type Track struct {
    name String
    codec String #Mime type
    init_seg Link # Link to the initialization segment data
    bandwidth Int
}
```

## Videos

Timecode nodes are created at specific intervals and linked together to form a structure around the video allowing it to be addressable by timecode.
Video clips are subgraph of the whole.

### IPLD Schemas
```
type VideoMetadata struct {
    timestamp Int # Unix time
    duration Float
    image Link # Poster & Thumbnail
    video &TimeCodeNode
    title String
}

type TimeCodeNode struct {
    time &DayNode
}

type DayNode struct {
    hour [&HourNode]
}

type HourNode struct {
    minute [&MinuteNode]
}

type MinuteNode struct {
    second [&SecondNode]
}

type SecondNode struct {
    video &VideoNode
    chat [&ChatMessage]
}
```
## Blog

Micro-blogging & long form via markdown files.

### IPLD Schemas
```
type MicroPost struct {
    timestamp Int # Unix Time
    content String
}

type FullPost struct {
    timestamp Int # Unix Time
    content Link # Link to markdown file
    image Link
    title String
}
```

## Mime Typed Data

Mime typed data.

If the data fit in a single block inline otherwise link to it.

### IPLD Schemas
```
type MimeTyped struct {
    mime_type String
    data EitherInlineOrLink
}

type EitherInlineOrLink union {
  | Inline bytes
  | Linked link
} representation kinded

type Inline Bytes
type Linked Link
```

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