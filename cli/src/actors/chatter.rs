use crate::actors::archivist::Archive;

use defluencer::{moderation_cache::ChatModerationCache, signatures::dag_jose::JsonWebSignature};

use futures_util::{future::AbortHandle, StreamExt, TryStreamExt};

use tokio::sync::{mpsc::UnboundedSender, watch::Receiver};

use ipfs_api::{
    responses::{Codec, PubSubMessage},
    IpfsService,
};

use linked_data::{
    media::chat::{ChatMessage, MessageType},
    moderation::{Ban, Bans, Moderators},
    signature::RawJWS,
    types::PeerId,
};

pub struct Chatter {
    ipfs: IpfsService,

    archive_tx: UnboundedSender<Archive>,

    shutdown: Receiver<()>,

    mod_db: ChatModerationCache,

    topic: String,

    bans: Bans,

    new_ban_count: usize,

    mods: Moderators,
}

impl Chatter {
    pub fn new(
        ipfs: IpfsService,
        archive_tx: UnboundedSender<Archive>,
        shutdown: Receiver<()>,
        topic: String,
        bans: Bans,
        mods: Moderators,
    ) -> Self {
        Self {
            ipfs,

            archive_tx,

            shutdown,

            mod_db: ChatModerationCache::new(100, 0),

            topic,

            bans,

            new_ban_count: 0,

            mods,
        }
    }

    pub async fn start(mut self) {
        let ipfs = self.ipfs.clone();

        let (_, regis) = AbortHandle::new_pair();
        let mut stream = ipfs
            .pubsub_sub(self.topic.as_bytes().to_owned(), regis)
            .boxed();

        println!("✅ Chat System Online");

        loop {
            tokio::select! {
                biased;

                _ = self.shutdown.changed() => break,

                res = stream.try_next() => match res {
                    Ok(option) => match option {
                        Some(msg) => self.on_pubsub_message(msg).await,
                        None => {},
                    },
                    Err(e) => eprintln!("{}", e),
                },
            }
        }

        if self.new_ban_count > 0 {
            match self.ipfs.dag_put(&self.bans, Codec::default()).await {
                Ok(cid) => println!(
                    "Updating Banned List with {} New Users 👍\nNew List CID: {}",
                    self.new_ban_count, cid
                ),
                Err(e) => eprintln!("❗ IPFS DAG Put Failed. {}", e),
            }
        }

        println!("❌ Chat System Offline");
    }

    async fn on_pubsub_message(&mut self, msg: PubSubMessage) {
        let PubSubMessage { from, data } = msg;
        let peer: PeerId = from.into();

        if self.mod_db.is_banned(&peer) {
            return;
        }

        let msg: ChatMessage = match serde_json::from_slice(&data) {
            Ok(data) => data,
            Err(e) => {
                eprintln!("❗ PubSub Message Deserialization Failed. {}", e);
                return;
            }
        };

        if !self.mod_db.is_verified(&peer, &msg.signature.link) {
            return self.get_origin(peer, msg).await;
        }

        self.process_msg(&peer, msg).await
    }

    async fn get_origin(&mut self, peer: PeerId, msg: ChatMessage) {
        let jws: JsonWebSignature = match self
            .ipfs
            .dag_get::<&str, RawJWS>(msg.signature.link, Option::<&str>::None)
            .await
        {
            Ok(raw_jws) => match raw_jws.try_into() {
                Ok(jws) => jws,
                Err(e) => {
                    eprintln!("❗ {}", e);
                    return;
                }
            },
            Err(e) => {
                eprintln!("❗ IPFS: dag get failed {}", e);
                return;
            }
        };

        let address = match jws.get_eth_address() {
            Some(addr) => addr,
            None => {
                self.mod_db
                    .add_peer(peer, msg.signature.link, [0u8; 20], None);

                self.mod_db.ban_peer(&peer);

                return;
            }
        };

        self.mod_db
            .add_peer(peer, msg.signature.link, address, None);

        if peer != jws.link.into() {
            self.mod_db.ban_peer(&peer);
            return;
        }

        if !jws.verify().is_ok() {
            self.mod_db.ban_peer(&peer);
            return;
        }

        if self.bans.banned_addrs.contains(&address) {
            self.mod_db.ban_peer(&peer);
            return;
        }

        self.process_msg(&peer, msg).await
    }

    async fn process_msg(&mut self, peer: &PeerId, chat: ChatMessage) {
        match chat.message {
            MessageType::Text(text) => self.mint_and_archive(text).await,
            MessageType::Ban(ban) => self.update_bans(peer, ban),
            MessageType::Mod(_) => {}
        }
    }

    async fn mint_and_archive(&self, msg: String) {
        let cid = match self.ipfs.dag_put(&msg, Codec::default()).await {
            Ok(cid) => cid,
            Err(e) => {
                eprintln!("❗ IPFS: dag put failed {}", e);
                return;
            }
        };

        let msg = Archive::Chat(cid);

        if let Err(error) = self.archive_tx.send(msg) {
            eprintln!("❗ Archive receiver hung up. {}", error);
        }
    }

    fn update_bans(&mut self, peer: &PeerId, ban: Ban) {
        let address = self.mod_db.get_address(peer).unwrap();

        if !self.mods.moderator_addrs.contains(address) {
            return;
        }

        self.mod_db.ban_peer(&ban.ban_peer);
        self.bans.banned_addrs.insert(ban.ban_addrs);

        self.new_ban_count += 1;
    }
}
