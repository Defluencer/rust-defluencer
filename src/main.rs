mod actors;
mod cli;
mod server;
mod utils;

use crate::cli::{
    beacon::{beacon_cli, BeaconCLI},
    comments::{comments_cli, Comments},
    content::{content_feed_cli, Content},
    daemon::{
        file::{file_cli, File},
        stream::{stream_cli, Stream},
    },
    friends::{friends_cli, Friends},
    identity::{identity_cli, IdentityCLI},
    live::{live_cli, LiveCLI},
    moderation::{moderation_cli, Moderation},
};

use structopt::StructOpt;

#[allow(clippy::large_enum_variant)]
#[derive(Debug, StructOpt)]
#[structopt(name = "defluencer")]
#[structopt(about)]
#[structopt(rename_all = "kebab-case")]
enum CommandLineInterface {
    /// Start the live streaming daemon.
    Stream(Stream),

    /// Start the file streaming daemon.
    File(File),

    /// Create a content beacon.
    Beacon(BeaconCLI),

    /// Appoint moderators & ban or unban users.
    Moderation(Moderation),

    /// Manage your content feed.
    Content(Content),

    /// Manage your comments.
    Comments(Comments),

    /// Manage your friends list.
    Friends(Friends),

    /// Manage your identity.
    Identity(IdentityCLI),

    /// Manage streaming metadata
    Live(LiveCLI),
}

#[tokio::main]
async fn main() {
    match CommandLineInterface::from_args() {
        CommandLineInterface::Stream(stream) => stream_cli(stream).await,
        CommandLineInterface::File(file) => file_cli(file).await,
        CommandLineInterface::Beacon(beacon) => beacon_cli(beacon).await,
        CommandLineInterface::Moderation(mods) => moderation_cli(mods).await,
        CommandLineInterface::Content(feed) => content_feed_cli(feed).await,
        CommandLineInterface::Comments(comments) => comments_cli(comments).await,
        CommandLineInterface::Friends(friends) => friends_cli(friends).await,
        CommandLineInterface::Identity(id) => identity_cli(id).await,
        CommandLineInterface::Live(live) => live_cli(live).await,
    }
}
