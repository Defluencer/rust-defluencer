use std::convert::TryFrom;

use crate::{
    cli::{
        content::{COMMENTS_KEY, FEED_KEY},
        friends::FRIENDS_KEY,
        identity::IDENTITY_KEY,
        live::LIVE_KEY,
        moderation::{BANS_KEY, MODS_KEY},
    },
    config::Configuration,
};

use tokio::task::JoinHandle;

use serde::Serialize;

use ipfs_api::{
    errors::Error,
    responses::{PinAddResponse, PinRmResponse},
    IpfsService,
};

use linked_data::{
    beacon::Beacon,
    comments::Commentary,
    content::FeedAnchor,
    follows::Friendlies,
    identity::Identity,
    keccak256,
    live::Live,
    moderation::{Bans, Moderators},
};

use structopt::StructOpt;

use cid::{
    multibase::{encode, Base},
    Cid,
};

#[derive(Debug, StructOpt)]
pub struct BeaconCLI {
    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(Debug, StructOpt)]
enum Command {
    /// Create a new beacon.
    Create(Create),

    /// Pin a beacon.
    /// Will recursively pin all associated data.
    /// The amount of data to be pinned could be MASSIVE use carefully.
    Pin(Pin),

    /// Unpin a beacon.
    /// Recursively unpin all associated data.
    Unpin(Unpin),
}

pub async fn beacon_cli(cli: BeaconCLI) {
    let res = match cli.cmd {
        Command::Create(create) => create_beacon(create).await,
        Command::Pin(pin) => pin_beacon(pin).await,
        Command::Unpin(unpin) => unpin_beacon(unpin).await,
    };

    if let Err(e) = res {
        eprintln!("❗ IPFS: {:#?}", e);
    }
}

#[derive(Debug, StructOpt)]
pub struct Create {
    /// Your choosen display name.
    #[structopt(short, long)]
    display_name: String,

    /// Link to an image avatar.
    #[structopt(short, long)]
    avatar: Cid,
}

async fn create_beacon(args: Create) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let Create {
        display_name,
        avatar,
    } = args;

    println!("Creating Beacon...");

    let mut config = match Configuration::from_file().await {
        Ok(conf) => conf,
        Err(e) => {
            eprintln!("❗ Cannot get configuration file. Error: {:#?}", e);
            eprintln!("Using Default...");
            Configuration::default()
        }
    };

    config.chat.topic = encode(
        Base::Base32Lower,
        &keccak256(&format!("{}_video", &display_name).into_bytes()),
    );
    config.video.pubsub_topic = encode(
        Base::Base32Lower,
        &keccak256(&format!("{}_chat", &display_name).into_bytes()),
    );

    config.save_to_file().await?;

    let peer_id = ipfs.peer_id().await?;

    #[cfg(debug_assertions)]
    println!("IPFS: peer id => {}", &peer_id);

    let live = Live {
        video_topic: config.video.pubsub_topic,
        chat_topic: config.chat.topic,
        peer_id,
    };

    let identity = Identity {
        display_name,
        avatar: avatar.into(),
    };

    let key_list = ipfs.key_list().await?;

    let (identity, content_feed, comments, live, friends, bans, mods) = tokio::try_join!(
        create_ipns_link::<Identity>(&ipfs, "Identity", IDENTITY_KEY, &key_list, Some(identity)),
        create_ipns_link::<FeedAnchor>(&ipfs, "Content Feed", FEED_KEY, &key_list, None),
        create_ipns_link::<Commentary>(&ipfs, "Comments", COMMENTS_KEY, &key_list, None),
        create_ipns_link::<Live>(&ipfs, "Live", LIVE_KEY, &key_list, Some(live)),
        create_ipns_link::<Friendlies>(&ipfs, "Friends", FRIENDS_KEY, &key_list, None),
        create_ipns_link::<Bans>(&ipfs, "Bans", BANS_KEY, &key_list, None),
        create_ipns_link::<Moderators>(&ipfs, "Mods", MODS_KEY, &key_list, None),
    )?;

    let beacon = linked_data::beacon::Beacon {
        identity,
        content_feed: Some(content_feed),
        comments: Some(comments),
        friends: Some(friends),
        live: Some(live),
        bans: Some(bans),
        mods: Some(mods),
    };

    let cid = ipfs.dag_put(&beacon).await?;

    if let Err(e) = ipfs.pin_add(&cid, false).await {
        eprintln!("❗ IPFS could not pin {}. Error: {}", cid.to_string(), e);
    }

    println!("✅ Created Beacon {}", &cid);

    Ok(())
}

#[derive(Debug, StructOpt)]
pub struct Pin {
    /// Beacon CID.
    #[structopt(short, long)]
    cid: Cid,
}

async fn pin_beacon(args: Pin) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let Pin { cid } = args;

    println!("Getting Beacon...");

    let beacon = ipfs.dag_get(&cid, Option::<&str>::None).await?;

    let Beacon {
        identity,
        content_feed,
        comments,
        friends,
        live,
        bans,
        mods,
    } = beacon;

    let mut handles = Vec::with_capacity(100);

    let handle = tokio::spawn({
        let ipfs = ipfs.clone();

        async move { ipfs.pin_add(&cid, false).await }
    });
    handles.push(handle);

    if let Some(content_feed) = content_feed {
        if let Ok(cid) = ipfs.name_resolve(&content_feed).await {
            println!("Getting Content Feed...");

            let handle = tokio::spawn({
                let ipfs = ipfs.clone();

                async move { ipfs.pin_add(&cid, false).await }
            });
            handles.push(handle);

            if let Ok(feed) = ipfs.dag_get::<&str, FeedAnchor>(&cid, None).await {
                for ipld in feed.content.into_iter() {
                    let ipfs = ipfs.clone();

                    let handle = tokio::spawn(async move { ipfs.pin_add(&ipld.link, true).await });

                    handles.push(handle);
                }
            }
        } else {
            println!("Cannot Resolve Content Feed");
        }
    }

    if let Some(comments) = comments {
        println!("Resolving Comments...");

        if let Ok(cid) = ipfs.name_resolve(&comments).await {
            let handle = tokio::spawn({
                let ipfs = ipfs.clone();

                async move { ipfs.pin_add(&cid, false).await }
            });
            handles.push(handle);

            println!("Getting Comments...");

            if let Ok(comments) = ipfs.dag_get::<&str, Commentary>(&cid, None).await {
                for ipld in comments.comments.into_values().flatten() {
                    let ipfs = ipfs.clone();

                    let handle = tokio::spawn(async move { ipfs.pin_add(&ipld.link, false).await });

                    handles.push(handle);
                }
            }
        } else {
            println!("Cannot Resolve Comments");
        }
    }

    pin(&ipfs, Some(identity), &mut handles);
    pin(&ipfs, friends, &mut handles);
    pin(&ipfs, live, &mut handles);
    pin(&ipfs, bans, &mut handles);
    pin(&ipfs, mods, &mut handles);

    println!("Pinning...");

    for handle in handles {
        match handle.await {
            Ok(result) => match result {
                Ok(_) => continue,
                Err(ipfs_err) => {
                    eprintln!("❗ IPFS: {}", ipfs_err);
                    continue;
                }
            },
            Err(e) => {
                eprintln!("❗ Tokio: {}", e);
                continue;
            }
        }
    }

    println!("✅ Pinned Beacon {}", &cid);

    Ok(())
}

#[derive(Debug, StructOpt)]
pub struct Unpin {
    /// Beacon CID.
    #[structopt(short, long)]
    cid: Cid,
}

async fn unpin_beacon(args: Unpin) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let Unpin { cid } = args;

    println!("Getting Beacon...");

    let beacon = ipfs.dag_get(&cid, Option::<&str>::None).await?;

    let Beacon {
        identity,
        content_feed,
        comments,
        friends,
        live,
        bans,
        mods,
    } = beacon;

    let mut handles = Vec::with_capacity(100);

    let handle = tokio::spawn({
        let ipfs = ipfs.clone();

        async move { ipfs.pin_rm(&cid, false).await }
    });
    handles.push(handle);

    println!("Resolving Content Feed...");

    if let Some(content_feed) = content_feed {
        if let Ok(cid) = ipfs.name_resolve(&content_feed).await {
            let handle = tokio::spawn({
                let ipfs = ipfs.clone();

                async move { ipfs.pin_rm(&cid, false).await }
            });
            handles.push(handle);

            println!("Getting Content Feed...");

            if let Ok(feed) = ipfs.dag_get::<&str, FeedAnchor>(&cid, None).await {
                for ipld in feed.content.into_iter() {
                    let ipfs = ipfs.clone();

                    let handle = tokio::spawn(async move { ipfs.pin_rm(&ipld.link, true).await });

                    handles.push(handle);
                }
            }
        } else {
            println!("Cannot Resolve Content Feed");
        }
    }

    if let Some(comments) = comments {
        println!("Resolving Comments...");

        if let Ok(cid) = ipfs.name_resolve(&comments).await {
            let handle = tokio::spawn({
                let ipfs = ipfs.clone();

                async move { ipfs.pin_rm(&cid, false).await }
            });
            handles.push(handle);

            println!("Getting Comments...");

            if let Ok(comments) = ipfs.dag_get::<&str, Commentary>(&cid, None).await {
                for ipld in comments.comments.into_values().flatten() {
                    let ipfs = ipfs.clone();

                    let handle = tokio::spawn(async move { ipfs.pin_rm(&ipld.link, false).await });

                    handles.push(handle);
                }
            }
        } else {
            println!("Cannot Resolve Comments");
        }
    }

    unpin(&ipfs, Some(identity), &mut handles);
    unpin(&ipfs, friends, &mut handles);
    unpin(&ipfs, live, &mut handles);
    unpin(&ipfs, bans, &mut handles);
    unpin(&ipfs, mods, &mut handles);

    println!("Unpinning...");

    for handle in handles {
        match handle.await {
            Ok(result) => match result {
                Ok(_) => continue,
                Err(ipfs_err) => {
                    eprintln!("❗ IPFS: {}", ipfs_err);
                    continue;
                }
            },
            Err(e) => {
                eprintln!("❗ Tokio: {}", e);
                continue;
            }
        }
    }

    println!("✅ Unpinned Beacon {}", &cid);

    Ok(())
}

fn pin(
    ipfs: &IpfsService,
    ipns: Option<Cid>,
    handles: &mut Vec<JoinHandle<Result<PinAddResponse, Error>>>,
) {
    if let Some(ipns) = ipns {
        let handle = tokio::spawn({
            let ipfs = ipfs.clone();

            async move {
                let cid = ipfs.name_resolve(&ipns).await?;

                ipfs.pin_add(&cid, false).await
            }
        });

        handles.push(handle);
    }
}

fn unpin(
    ipfs: &IpfsService,
    ipns: Option<Cid>,
    handles: &mut Vec<JoinHandle<Result<PinRmResponse, Error>>>,
) {
    if let Some(ipns) = ipns {
        let handle = tokio::spawn({
            let ipfs = ipfs.clone();

            async move {
                let cid = ipfs.name_resolve(&ipns).await?;

                ipfs.pin_rm(&cid, false).await
            }
        });

        handles.push(handle);
    }
}

async fn create_ipns_link<T>(
    ipfs: &IpfsService,
    name: &str,
    key: &'static str,
    key_list: &ipfs_api::responses::KeyList,
    data: Option<T>,
) -> Result<Cid, Error>
where
    T: Default + Serialize,
{
    let cid = match key_list.get(key) {
        Some(kp) => *kp,
        None => {
            println!("Generating {} IPNS Key...", name);

            let key_pair = ipfs.key_gen(key).await?;

            println!("Updating {} IPNS Link...", name);

            let cid = Cid::try_from(key_pair.id)?;

            cid
        }
    };

    if let Some(data) = data {
        ipfs.ipns_put(key, false, &data).await?;
    } else {
        ipfs.ipns_put(key, false, &T::default()).await?;
    }

    println!("✅ {} IPNS Link => {}", name, &cid);

    Ok(cid)
}
