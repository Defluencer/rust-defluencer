use defluencer::{errors::Error, Defluencer};

use cid::Cid;

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
    /// Add a new comment.
    Add(Comment),

    /// Remove an old comment.
    Remove(Comment),
}

pub async fn comments_cli(cli: Comments) {
    let res = match cli.cmd {
        Command::Add(args) => add(cli.key_name, args).await,
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

    if let Some(channel) = defluencer.get_local_channel(key).await? {
        channel.add_comment(args.comment).await?;
    }

    println!("✅ Added Comment {}", args.comment);

    Ok(())
}

async fn remove(key: String, args: Comment) -> Result<(), Error> {
    let defluencer = Defluencer::default();

    if let Some(channel) = defluencer.get_local_channel(key).await? {
        channel.remove_comment(args.comment).await?;
    }

    println!("✅ Removed Comment {}", args.comment);

    Ok(())
}
