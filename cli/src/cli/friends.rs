use std::io::ErrorKind;

use ipfs_api::{errors::Error, IpfsService};

use linked_data::follows::{Friend, Friendlies};

use cid::Cid;

use structopt::StructOpt;

use either::Either;

pub const FRIENDS_KEY: &str = "friends";

#[derive(Debug, StructOpt)]
pub struct Friends {
    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(Debug, StructOpt)]
enum Command {
    /// Add a new friend to your list.
    /// Use either their beacon Cid OR their ethereum name service domain name.
    Add(AddFriend),

    /// Remove a friend from your list.
    /// Use either their beacon Cid OR their ethereum name service domain name.
    Remove(RemoveFriend),
}

pub async fn friends_cli(cli: Friends) {
    let res = match cli.cmd {
        Command::Add(add) => add_friend(add).await,
        Command::Remove(remove) => remove_friend(remove).await,
    };

    if let Err(e) = res {
        eprintln!("❗ IPFS: {:#?}", e);
    }
}

#[derive(Debug, StructOpt)]
pub struct AddFriend {
    /// Beacon CID.
    #[structopt(short, long)]
    beacon: Option<Cid>,

    /// Ethereum name service domain.
    #[structopt(short, long)]
    ens: Option<String>,
}

async fn add_friend(command: AddFriend) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let AddFriend { beacon, ens } = command;

    let new_friend = match (beacon, ens) {
        (Some(cid), None) => Friend {
            friend: Either::Right(cid.into()),
        },
        (None, Some(name)) => Friend {
            friend: Either::Left(name),
        },
        (_, _) => return Err(std::io::Error::from(ErrorKind::InvalidInput).into()),
    };

    println!("Adding Friend {:?}", &new_friend.friend);

    let res = ipfs.ipns_get(FRIENDS_KEY).await?;
    let (old_friends_cid, mut list): (Cid, Friendlies) = res.unwrap();

    list.friends.insert(new_friend);

    println!("Updating Friends List...");

    ipfs.ipns_put(FRIENDS_KEY, false, &list).await?;

    println!("Unpinning Old List...");

    if let Err(e) = ipfs.pin_rm(&old_friends_cid, false).await {
        eprintln!("❗ IPFS could not unpin {}. Error: {}", old_friends_cid, e);
    }

    println!("✅ Friend Added");

    Ok(())
}

#[derive(Debug, StructOpt)]
pub struct RemoveFriend {
    /// Beacon CID
    #[structopt(short, long)]
    beacon: Option<Cid>,

    /// Ethereum name service domain name.
    #[structopt(short, long)]
    ens: Option<String>,
}

async fn remove_friend(command: RemoveFriend) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let RemoveFriend { beacon, ens } = command;

    let old_friend = match (beacon, ens) {
        (Some(cid), None) => Friend {
            friend: Either::Right(cid.into()),
        },
        (None, Some(name)) => Friend {
            friend: Either::Left(name),
        },
        (_, _) => return Err(std::io::Error::from(ErrorKind::InvalidInput).into()),
    };

    println!("Removing Friend {:?}", &old_friend.friend);

    let res = ipfs.ipns_get(FRIENDS_KEY).await?;
    let (old_friends_cid, mut list): (Cid, Friendlies) = res.unwrap();

    list.friends.remove(&old_friend);

    println!("Updating Friends List...");

    ipfs.ipns_put(FRIENDS_KEY, false, &list).await?;

    println!("Unpinning Old List...");

    if let Err(e) = ipfs.pin_rm(&old_friends_cid, false).await {
        eprintln!("❗ IPFS could not unpin {}. Error: {}", old_friends_cid, e);
    }

    println!("✅ Friend Removed");

    Ok(())
}
