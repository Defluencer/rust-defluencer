# Defluencer Protocol IPLD Schemas

## Channel

The root of the DAG representing all information related to a channel.

```
advanced ChronologicalMap { ADL "" } #TODO add specifications
advanced ShardedMap { ADL "HAMT/v1" }

type DateTime map [Time:&SignedLink] using ChronologicalMap

type Comments map [Bytes:&SignedLink] using ShardedMap # Keys are hashes of comments Cids
type CommentIndex map [Bytes:&Comments] using ShardedMap # Keys are hashes of content Cids

type ChannelMetadata struct {
    identity &Identity
    content_index optional &DateTime
    comment_index optional &CommentIndex
    live optional &LiveSettings
    folows optional &Follows
    agregation_channel optional String # Name of the pubsub channel used.
}
```

## Signed Links

W.I.P. (EcDSA only)

Links to media content is usualy signed by the creator.

Plan to replace the current system with DAG-JOSE blocks when the cryptography is figured out.

```
type HashAlgorithm union {
  | BitcoinLedgerApp "BitcoinLedgerApp"
  | EthereumLedgerApp "EthereumLedgerApp"
} representation keyed

type SignedLink {
    link Link # The root CID of the DAG being signed.
    public_key Bytes
    hash_algo HashAlgorithm
    signature Bytes
}
```

## Identity

User or Channel identity information. 

```
type Identity struct {
    name String
    bio optional String
    banner optional Link # max size block of raw image data
    avatar optional Link # max size block of raw image data
    ipns_addr optional String # IPNS address
    eth_addr optional String # Ethereum address
    btc_addr optional String # Bitcoin address
}
```

## Media

```
type Media union {
    | &BlogPost link
    | &Video link
    | &Comment link
} representation kinded

type BlogPost struct {
    identity &Identity
    user_timestamp Int # Unix Time
    content Link # Link to markdown file
    title String
    image optional Link # max size block of raw image data
    word_count optional Int # number of words in markdown file
}

type Video struct {
    identity &Identity
    user_timestamp Int # Unix time
    video &TimeCode
    title String
    duration optional Float
    image optional Link # max size block of raw image data 
}

type Comment struct {
    identity &Identity
    user_timestamp Int # Unix Time
    origin optional String # CID as string to prevent recursive pinning.
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

## Chat

W.I.P.

The purpose of signing ChatInfo is to mitigate identity theft.

Since chat sessions have definite start times, the latest block hash could be used, in conjuction with a signature to achive adequate security without requiring the user to sign every message.

This scheme make local IPFS node keys theft less of a bulletproof way to impersonate someone. Rotating Peer Id and signing again would end the attack and the attacker would have to wait for the real user to start chatting before attacking, making it very obvious.

Every chat implementation would have to invalidate the old ChatInfo when the same public key sign a new ChatInfo

```
type ChatInfo {
    name String
    node Bytes # Peer Id of the node used to chat
}

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
    message MessageType

    signature Link # Link to DAG-JOSE block linking to peer id
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

## Streams

A video node contains links to segments of videos of all quality. As video is streamed, new video nodes are created and linked to previous ones.
A special node contains the stream setup data; codecs, qualities, initialization segments, etc...

```
type Track struct {
    name String
    codec String # Mime type
    initseg Link # Link to the initialization segment raw data
    bandwidth Int
}

type Setup struct {
    tracks [Track] # Sorted from lowest to highest bitrate.
}

type Segment struct {
    tracks {String:Link} # Name of the track egg "audio" or "1080p60" & link to video segment raw data
    setup optional &Setup
    previous optional &Segment
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