use cid::Cid;
use defluencer::{channel::Channel, errors::Error, signatures::TestSigner, Defluencer};

use futures_util::{future::AbortHandle, pin_mut};

use ipfs_api::IpfsService;

use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct ChannelCLI {
    #[structopt(subcommand)]
    cmd: Command,
}

#[derive(Debug, StructOpt)]
enum Command {
    /// Create a new channel.
    Create(Create),

    /// Recursively pin all channel associated data.
    ///
    /// CAUTION: The amount of data to download could be MASSIVE.
    Pin(Pinning),

    /// Recursively unpin all channel associated data.
    Unpin(Pinning),

    /// Receive channel updates in real time.
    Subscribe(Subscribe),
}

pub async fn channel_cli(cli: ChannelCLI) {
    let res = match cli.cmd {
        Command::Create(args) => create(args).await,
        Command::Pin(args) => pin(args).await,
        Command::Unpin(args) => unpin(args).await,
        Command::Subscribe(args) => subscribe(args).await,
    };

    if let Err(e) = res {
        eprintln!("❗ IPFS: {:#?}", e);
    }
}

#[derive(Debug, StructOpt)]
pub struct Create {
    /// Your choosen channel name.
    #[structopt(short, long)]
    display_name: String,
}

async fn create(args: Create) -> Result<(), Error> {
    let ipfs = IpfsService::default();

    let signer = TestSigner::default(); // TODO

    let channel = Channel::create(args.display_name, ipfs, signer).await?;

    println!(
        "✅ Channel Created\nIPNS Address: {}",
        channel.get_address()
    );

    Ok(())
}

#[derive(Debug, StructOpt)]
pub struct Pinning {
    /// Channel IPNS address.
    #[structopt(short, long)]
    address: Cid,
}

async fn pin(args: Pinning) -> Result<(), Error> {
    let defluencer = Defluencer::default();

    defluencer.pin_channel(args.address.into()).await?;

    println!("Channel's Content Pinned ✅");

    Ok(())
}

async fn unpin(args: Pinning) -> Result<(), Error> {
    let defluencer = Defluencer::default();

    defluencer.unpin_channel(args.address.into()).await?;

    println!("Channel's Content Unpinned ✅");

    Ok(())
}

#[derive(Debug, StructOpt)]
pub struct Subscribe {
    /// Channel IPNS address.
    #[structopt(short, long)]
    address: Cid,
}

async fn subscribe(args: Subscribe) -> Result<(), Error> {
    use futures_util::TryStreamExt;

    let defluencer = Defluencer::default();

    let (handle, regis) = AbortHandle::new_pair();
    let stream = defluencer.subscribe_ipns_updates(args.address.into(), regis);
    pin_mut!(stream);

    let control = tokio::signal::ctrl_c();
    pin_mut!(control);

    println!("Awaiting Updates\nPress CRTL-C to exit...");

    loop {
        tokio::select! {
            biased;

            _ = &mut control => {
                handle.abort();
                return Ok(());
            }

            result = stream.try_next() => match result {
                Ok(option) => match option {
                    Some(cid) => println!("New Channel Metadata: {}", cid),
                    None => continue,
                },
                Err(e) => return Err(e),
            }
        }
    }
}
