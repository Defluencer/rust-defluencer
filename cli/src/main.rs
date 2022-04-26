mod actors;
mod cli;
mod config;
mod server;

use crate::cli::{
    channel::{channel_cli, ChannelCLI},
    comments::{comments_cli, Comments},
    content::{content_cli, Content},
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

    /// Create a channel.
    Channel(ChannelCLI),

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
        CommandLineInterface::Stream(args) => stream_cli(args).await,
        CommandLineInterface::File(args) => file_cli(args).await,
        CommandLineInterface::Channel(args) => channel_cli(args).await,
        CommandLineInterface::Moderation(args) => moderation_cli(args).await,
        CommandLineInterface::Content(args) => content_cli(args).await,
        CommandLineInterface::Comments(args) => comments_cli(args).await,
        CommandLineInterface::Friends(args) => friends_cli(args).await,
        CommandLineInterface::Identity(args) => identity_cli(args).await,
        CommandLineInterface::Live(args) => live_cli(args).await,
    }
}
