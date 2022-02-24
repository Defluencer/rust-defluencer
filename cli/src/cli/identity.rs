use ipfs_api::{errors::Error, IpfsService};

use linked_data::identity::Identity;

use cid::Cid;

use structopt::StructOpt;

pub const IDENTITY_KEY: &str = "identity";

#[derive(Debug, StructOpt)]
pub struct IdentityCLI {
    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(Debug, StructOpt)]
enum Command {
    /// Choose a new display name.
    Name(UpdateName),

    /// Choose a new image avatar.
    Avatar(UpdateAvatar),
}

pub async fn identity_cli(cli: IdentityCLI) {
    let res = match cli.cmd {
        Command::Name(name) => update_name(name).await,
        Command::Avatar(avatar) => update_avatar(avatar).await,
    };

    if let Err(e) = res {
        eprintln!("❗ IPFS: {:#?}", e);
    }
}

#[derive(Debug, StructOpt)]
pub struct UpdateName {
    /// Display name.
    #[structopt(short, long)]
    name: String,
}

async fn update_name(command: UpdateName) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let UpdateName { name } = command;

    let res = ipfs.ipns_get(IDENTITY_KEY).await?;
    let (old_id_cid, mut id): (Cid, Identity) = res.unwrap();

    id.display_name = name;

    ipfs.ipns_put(IDENTITY_KEY, false, &id).await?;

    if let Err(e) = ipfs.pin_rm(&old_id_cid, false).await {
        eprintln!("❗ IPFS could not unpin {}. Error: {}", old_id_cid, e);
    }

    println!("✅ Display Name Updated");

    Ok(())
}

#[derive(Debug, StructOpt)]
pub struct UpdateAvatar {
    /// Link to image avatar.
    #[structopt(short, long)]
    image: Cid,
    // Path to image file.
    //#[structopt(short, long)]
    //path: Option<PathBuf>,
}

async fn update_avatar(command: UpdateAvatar) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let UpdateAvatar { image } = command;

    let res = ipfs.ipns_get(IDENTITY_KEY).await?;
    let (old_id_cid, mut id): (Cid, Identity) = res.unwrap();

    id.avatar = image.into();

    ipfs.ipns_put(IDENTITY_KEY, false, &id).await?;

    if let Err(e) = ipfs.pin_rm(&old_id_cid, false).await {
        eprintln!("❗ IPFS could not unpin {}. Error: {}", old_id_cid, e);
    }

    println!("✅ Avatar Updated");

    Ok(())
}
