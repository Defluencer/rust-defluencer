# Defluencer
Defluencer protocol rust implementation.

## Crates
- [linked-data](https://github.com/Defluencer/rust-defluencer/tree/develop/linked-data)
- [ipfs-api](https://github.com/Defluencer/rust-defluencer/tree/develop/ipfs-api)
- [cli](https://github.com/Defluencer/rust-defluencer/tree/develop/cli)
- [core](https://github.com/Defluencer/rust-defluencer/tree/develop/defluencer)

## How does it works?

Thanks to IPFS, every user who consume media content also redistribute it, resulting in social media without central servers. Data on IPFS is immutable which means your content can be shared but never modified. As content get viral, it is replicated by anyone who reads, whatches or interacts with it in any way.

Users following users create a social web that can be crawled for more content.

If instead of creating, a user only share, they become de facto a content aggregator. Users can be individuals or organisations. Organisation who filer, aggregate and moderate content are called platforms and works similarily to curent social media giants.

### Storage

Defluencer communicates to a local IPFS node via the HTTP API. Metadata and content are added to IPFS and are cryptographically signed. The end result is a DAG representing an entire social media presence (videos, photos, comments, blog posts, etc...). The root of this DAG is called a "Beacon" and changes with every update.

### Anchoring

Having a constantly changing Beacon is not very useful, we need a permanent but mutable link.

IPNS is used for this purpose. An IPNS address is the hash of a public key, does not change and points to an IPNS record. In this record a link and a signature, allow anyone to verify and fetch the most up to date beacon of the owner of the secret key.

The protocol is agnostic to this anchoring system. Blockchains, DIDs or even web server could be used.

### Addressing

TODO
