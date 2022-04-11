# Defluencer Protocol
Rust implementation.

## Crates
- [linked-data](https://github.com/Defluencer/rust-defluencer/tree/develop/linked-data)
- [ipfs-api](https://github.com/Defluencer/rust-defluencer/tree/develop/ipfs-api)
- [cli](https://github.com/Defluencer/rust-defluencer/tree/develop/cli)
- [core](https://github.com/Defluencer/rust-defluencer/tree/develop/defluencer)

# How does it works?

Users can create branded channels or rely on existing channels. If a channel only share user created content, they become de facto a content aggregator. Channels can be individuals or organisations. Organisation who choose to filter, aggregate and moderate content are called platforms. Platforms are free to build their own website or app.

# Storage Network

Defluencer is a protocol built on top of the inter-planetary file system (IPFS). On IPFS data is **content addressed** which means your content can be shared but never modified. As content go viral, it is **replicated** by anyone who reads, whatches or interacts with it in any way, resulting in social media without central servers.

Social media content added to IPFS is **cryptographically signed**. By doing so, each piece of content becomes **verifiable**.

Websites or applications folowing the protocol become **interoperable** with each other for the same reasons.

# Channel Anchoring

Content addressed data is great when sharing but having a constantly changing channel identifier is not very useful, we need something permanent.

IPNS is used for this purpose. An IPNS address is the hash of a public key, does not change and points to a record. In this record a link and a signature allows anyone to verify and fetch the most up to date channel identifier.

The protocol is agnostic to this anchoring system. Blockchains, DIDs or even web server could be used.

# Channel Addressing

Having permanent identifier for channels is good but who want to remember a number? Channels should have names like websites.

Ethereum name service can be use to associate a name to an IPNS address. Again, the protocol is agnostic to this, you can use what you want.