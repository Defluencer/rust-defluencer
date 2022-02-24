use crate::{
    actors::archivist::Archive,
    cli::moderation::{BANS_KEY, MODS_KEY},
    config::ChatConfig,
};

use futures_util::future::AbortHandle;
use tokio::sync::mpsc::UnboundedSender;
use tokio_stream::StreamExt;

use ipfs_api::{errors::Error, responses::PubSubMessage, IpfsService};

use linked_data::{
    chat::{ChatId, ChatMessage, MessageType},
    moderation::{Ban, Bans, ChatModerationCache, Moderators},
    signature::SignedMessage,
    PeerId,
};

pub struct ChatAggregator {
    ipfs: IpfsService,

    archive_tx: UnboundedSender<Archive>,

    mod_db: ChatModerationCache,

    topic: String,

    bans: Bans,

    new_ban_count: usize,

    mods: Moderators,
}

impl ChatAggregator {
    pub async fn new(
        ipfs: IpfsService,
        archive_tx: UnboundedSender<Archive>,
        config: ChatConfig,
    ) -> Result<Self, Error> {
        let ChatConfig { topic } = config;

        let (mod_res, ban_res) =
            match tokio::try_join!(ipfs.ipns_get(MODS_KEY), ipfs.ipns_get(BANS_KEY)) {
                Ok(res) => res,
                Err(e) => {
                    return Err(e);
                }
            };

        //TODO error handling
        let (_, bans) = ban_res.unwrap();
        let (_, mods) = mod_res.unwrap();

        Ok(Self {
            ipfs,

            archive_tx,

            mod_db: ChatModerationCache::new(100, 0),

            topic,

            bans,

            new_ban_count: 0,

            mods,
        })
    }

    pub async fn start(mut self) {
        let res = self.ipfs.pubsub_sub(&self.topic).await.unwrap();
        let (_, regis) = AbortHandle::new_pair();
        let mut stream = ipfs_api::pubsub_stream(res, regis);

        println!("✅ Chat System Online");

        while let Some(result) = stream.next().await {
            if self.archive_tx.is_closed() {
                //Hacky way to shutdown
                break;
            }

            match result {
                Ok(response) => self.on_pubsub_message(response).await,
                Err(error) => {
                    eprintln!("{}", error);
                    continue;
                }
            }
        }

        if self.new_ban_count > 0 {
            println!(
                "Updating Banned List with {} New Users 👍",
                self.new_ban_count
            );

            if let Err(e) = self.ipfs.ipns_put(BANS_KEY, false, &self.bans).await {
                eprintln!("❗ IPNS Update Failed. {}", e);
            }
        }

        println!("❌ Chat System Offline");
    }

    async fn on_pubsub_message(&mut self, msg: PubSubMessage) {
        let PubSubMessage { from, data } = msg;
        let peer: PeerId = from;

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
        let sign_msg: SignedMessage<ChatId> = match self
            .ipfs
            .dag_get(&msg.signature.link, Option::<&str>::None)
            .await
        {
            Ok(msg) => msg,
            Err(e) => {
                eprintln!("❗ IPFS: dag get failed {}", e);
                return;
            }
        };

        self.mod_db
            .add_peer(peer, msg.signature.link, sign_msg.address, None);

        if peer != sign_msg.data.peer_id {
            self.mod_db.ban_peer(&peer);
            return;
        }

        if !sign_msg.verify() {
            self.mod_db.ban_peer(&peer);
            return;
        }

        if self.bans.banned_addrs.contains(&sign_msg.address) {
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

    async fn mint_and_archive(&mut self, msg: String) {
        let cid = match self.ipfs.dag_put(&msg).await {
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
