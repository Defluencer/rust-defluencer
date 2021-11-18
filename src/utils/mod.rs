pub mod config;
pub mod dag_nodes;

pub const OPTIONS: ipfs_api::request::Add = ipfs_api::request::Add {
    trickle: None,
    only_hash: None,
    wrap_with_directory: None,
    chunker: None,
    pin: Some(false),
    raw_leaves: None,
    cid_version: Some(1),
    hash: None,
    inline: None,
    inline_limit: None,
};
