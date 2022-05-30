mod actors;
mod cli;
mod server;

use clap::Parser;

use crate::cli::{
    channel::{channel_cli, ChannelCLI},
    daemon::{
        file::{file_cli, File},
        stream::{stream_cli, Stream},
    },
    node::{node_cli, NodeCLI},
    user::{user_cli, UserCLI},
};

#[derive(Debug, Parser)]
#[clap(name = "defluencer", author = "SionoiS <defluencer@protonmail.com>", version, about, long_about = None, rename_all = "kebab-case")]
enum CommandLineInterface {
    /// Start the video live streaming daemon.
    Stream(Stream),

    /// Start the video file streaming daemon.
    File(File),

    /// Channel related commands.
    Channel(ChannelCLI),

    /// User related commands.
    User(UserCLI),

    /// Manage your node and other utilities.
    Node(NodeCLI),
}

#[tokio::main]
async fn main() {
    match CommandLineInterface::parse() {
        CommandLineInterface::Stream(args) => stream_cli(args).await,
        CommandLineInterface::File(args) => file_cli(args).await,
        CommandLineInterface::Channel(args) => channel_cli(args).await,
        CommandLineInterface::User(args) => user_cli(args).await,
        CommandLineInterface::Node(args) => node_cli(args).await,
    }
}
