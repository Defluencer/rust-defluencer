use std::io::ErrorKind;

use ipfs_api::{errors::Error, IpfsService};

use linked_data::comments::{Comment, Commentary};

use cid::Cid;

use structopt::StructOpt;

pub const COMMENTS_KEY: &str = "comments";

#[derive(Debug, StructOpt)]
pub struct Comments {
    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(Debug, StructOpt)]
enum Command {
    /// Add a new comment.
    Add(AddComment),

    /// Remove an old comment.
    Remove(RemoveComment),
}

pub async fn comments_cli(cli: Comments) {
    let res = match cli.cmd {
        Command::Add(add) => add_comment(add).await,
        Command::Remove(remove) => remove_comment(remove).await,
    };

    if let Err(e) = res {
        eprintln!("❗ IPFS: {:#?}", e);
    }
}

#[derive(Debug, StructOpt)]
pub struct AddComment {
    /// CID of content being commented on.
    #[structopt(short, long)]
    origin: Cid,

    /// Content of your comment.
    #[structopt(short, long)]
    comment: String,
}

async fn add_comment(command: AddComment) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let AddComment { origin, comment } = command;

    let comment = Comment::create(origin, comment);
    let comment_cid = ipfs.dag_put(&comment).await?;

    println!("Pinning...");

    if let Err(e) = ipfs.pin_add(&comment_cid, false).await {
        eprintln!("❗ IPFS could not pin {}. Error: {}", comment_cid, e);
    }

    println!("Updating Comment List...");

    let res: Option<(Cid, Commentary)> = ipfs.ipns_get(COMMENTS_KEY).await?;
    let (old_comments_cid, mut list) = res.unwrap();

    match list.comments.get_mut(&origin) {
        Some(vec) => vec.push(comment_cid.into()),
        None => {
            list.comments.insert(origin, vec![comment_cid.into()]);
        }
    }

    ipfs.ipns_put(COMMENTS_KEY, false, &list).await?;

    println!("Unpinning Old List...");

    if let Err(e) = ipfs.pin_rm(&old_comments_cid, false).await {
        eprintln!("❗ IPFS could not unpin {}. Error: {}", old_comments_cid, e);
    }

    println!("✅ Added Comment {}", comment_cid);

    Ok(())
}

#[derive(Debug, StructOpt)]
pub struct RemoveComment {
    /// CID of the content commented on.
    #[structopt(short, long)]
    origin: Cid,

    /// CID of comment to remove.
    #[structopt(short, long)]
    comment: Cid,
}

async fn remove_comment(command: RemoveComment) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let RemoveComment { origin, comment } = command;

    let res: Option<(Cid, Commentary)> = ipfs.ipns_get(COMMENTS_KEY).await?;
    let (old_comments_cid, mut list) = res.unwrap();

    let vec = match list.comments.get_mut(&origin) {
        Some(vec) => vec,
        None => return Err(std::io::Error::from(ErrorKind::NotFound).into()),
    };

    let index = match vec.iter().position(|&ipld| ipld == comment.into()) {
        Some(idx) => idx,
        None => return Err(std::io::Error::from(ErrorKind::NotFound).into()),
    };

    vec.remove(index);

    if vec.is_empty() {
        list.comments.remove(&origin);
    }

    println!("Updating Comment List...");

    ipfs.ipns_put(COMMENTS_KEY, false, &list).await?;

    println!("Unpinning Old List...");

    if let Err(e) = ipfs.pin_rm(&old_comments_cid, false).await {
        eprintln!("❗ IPFS could not unpin {}. Error: {}", old_comments_cid, e);
    }

    println!("✅ Removed Comment {}", comment);

    Ok(())
}
