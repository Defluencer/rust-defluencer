mod actors;
mod cli;
mod server;

use clap::{Parser, Subcommand};

use crate::cli::{
    channel::{channel_cli, ChannelCLI},
    daemon::{
        file::{file_cli, File},
        stream::{stream_cli, Stream},
    },
    node::{node_cli, NodeCLI},
    user::{user_cli, UserCLI},
};

#[derive(Parser)]
#[command(name = "defluencer", bin_name= "defluencer", author = "SionoiS <defluencer@protonmail.com>", version, about, long_about = None, rename_all = "kebab-case")]
struct Defluencer {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Start the video live streaming daemon.
    Stream(Stream),

    /// Start the video file streaming daemon.
    File(File),

    /// Channel related commands.
    Channel(ChannelCLI),

    /// User related commands.
    User(UserCLI),

    /// Manage your node and other utilities.
    #[command(subcommand)]
    Node(NodeCLI),
}

#[tokio::main]
async fn main() {
    let cli = Defluencer::parse();

    match cli.command {
        Commands::Stream(args) => stream_cli(args).await,
        Commands::File(args) => file_cli(args).await,
        Commands::Channel(args) => channel_cli(args).await,
        Commands::User(args) => user_cli(args).await,
        Commands::Node(args) => node_cli(args).await,
    }
}
