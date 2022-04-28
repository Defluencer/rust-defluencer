use defluencer::{errors::Error, Defluencer};

use cid::Cid;

use futures_util::pin_mut;

use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Comments {
    /// Channel local key name.
    #[structopt(short, long)]
    key_name: String,

    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(Debug, StructOpt)]
enum Command {
    /// Add a new comment to your channel.
    Add(Comment),

    /// Stream all comments for some content on your channel.
    Stream(Stream),

    /// Remove an old comment from your channel.
    Remove(Comment),
}

pub async fn comments_cli(cli: Comments) {
    let res = match cli.cmd {
        Command::Add(args) => add(cli.key_name, args).await,
        Command::Stream(args) => stream(cli.key_name, args).await,
        Command::Remove(args) => remove(cli.key_name, args).await,
    };

    if let Err(e) = res {
        eprintln!("❗ IPFS: {:#?}", e);
    }
}

#[derive(Debug, StructOpt)]
pub struct Comment {
    /// The CID of the comment.
    #[structopt(short, long)]
    comment: Cid,
}

async fn add(key: String, args: Comment) -> Result<(), Error> {
    let defluencer = Defluencer::default();

    let channel = match defluencer.get_local_channel(key).await? {
        Some(channel) => channel,
        None => {
            eprintln!("❗ Cannot find channel");
            return Ok(());
        }
    };

    channel.add_comment(args.comment).await?;

    println!("✅ Added Comment {}", args.comment);

    Ok(())
}

async fn remove(key: String, args: Comment) -> Result<(), Error> {
    let defluencer = Defluencer::default();

    let channel = match defluencer.get_local_channel(key).await? {
        Some(channel) => channel,
        None => {
            eprintln!("❗ Cannot find channel");
            return Ok(());
        }
    };

    channel.remove_comment(args.comment).await?;

    println!("✅ Removed Comment {}", args.comment);

    Ok(())
}

#[derive(Debug, StructOpt)]
pub struct Stream {
    /// Content CID.
    #[structopt(short, long)]
    content: Cid,
}

async fn stream(key: String, args: Stream) -> Result<(), Error> {
    use futures_util::TryStreamExt;

    let defluencer = Defluencer::default();

    let Stream { content } = args;

    let channel = match defluencer.get_local_channel(key).await? {
        Some(channel) => channel,
        None => {
            eprintln!("❗ Cannot find channel");
            return Ok(());
        }
    };

    let (_cid, metadata) = channel.get_metadata().await?;

    let index = match metadata.comment_index {
        Some(ipns) => ipns,
        None => {
            eprintln!("❗ This channel has no comments.");
            return Ok(());
        }
    };

    let stream = defluencer.stream_comments(index, content);
    pin_mut!(stream);

    while let Some(cid) = stream.try_next().await? {
        println!("{}", cid);
    }

    Ok(())
}
