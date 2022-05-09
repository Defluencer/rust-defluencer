use defluencer::{channel::Channel, errors::Error, signatures::TestSigner, Defluencer};

use cid::Cid;

use futures_util::pin_mut;

use ipfs_api::IpfsService;
use linked_data::channel::ChannelMetadata;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct Comments {
    /// Channel IPNS address.
    #[structopt(short, long)]
    address: Cid,

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
        Command::Add(args) => add(cli.address, args).await,
        Command::Stream(args) => stream(cli.address, args).await,
        Command::Remove(args) => remove(cli.address, args).await,
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

async fn add(ipns: Cid, args: Comment) -> Result<(), Error> {
    let ipfs = IpfsService::default();
    let signer = TestSigner::default(); //TODO

    let channel = Channel::new(ipfs, ipns.into(), signer);

    channel.add_comment(args.comment).await?;

    println!("✅ Added Comment {}", args.comment);

    Ok(())
}

async fn remove(ipns: Cid, args: Comment) -> Result<(), Error> {
    let ipfs = IpfsService::default();
    let signer = TestSigner::default(); //TODO

    let channel = Channel::new(ipfs, ipns.into(), signer);

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

async fn stream(ipns: Cid, args: Stream) -> Result<(), Error> {
    use futures_util::TryStreamExt;

    let ipfs = IpfsService::default();
    let defluencer = Defluencer::new(ipfs.clone());

    let cid = ipfs.name_resolve(ipns).await?;
    let metadata = ipfs.dag_get::<&str, ChannelMetadata>(cid, None).await?;

    let index = match metadata.comment_index {
        Some(ipns) => ipns,
        None => {
            eprintln!("❗ This channel has no comments.");
            return Ok(());
        }
    };

    let stream = defluencer.stream_comments(index, args.content);
    pin_mut!(stream);

    while let Some(cid) = stream.try_next().await? {
        println!("{}", cid);
    }

    println!("✅ Comments Stream End");

    Ok(())
}
