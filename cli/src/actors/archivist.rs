use tokio::sync::mpsc::UnboundedReceiver;

use ipfs_api::{responses::Codec, IpfsService};

use linked_data::media::video::{Day, Hour, Minute, Second, Timecode};

use cid::Cid;

pub enum Archive {
    //Chat(Cid),
    Video(Cid),
}

pub struct Archivist {
    ipfs: IpfsService,

    archive_rx: UnboundedReceiver<Archive>,

    video_chat_buffer: Option<Second>,

    minute_node: Minute,
    hour_node: Hour,
    day_node: Day,
}

impl Archivist {
    pub fn new(ipfs: IpfsService, archive_rx: UnboundedReceiver<Archive>) -> Self {
        Self {
            ipfs,

            archive_rx,

            video_chat_buffer: None,

            minute_node: Minute {
                links_to_seconds: Vec::with_capacity(60),
            },

            hour_node: Hour {
                links_to_minutes: Vec::with_capacity(60),
            },

            day_node: Day {
                links_to_hours: Vec::with_capacity(24),
            },
        }
    }

    pub async fn start(mut self) {
        println!("✅ Archive System Online");

        while let Some(event) = self.archive_rx.recv().await {
            match event {
                //Archive::Chat(cid) => self.archive_chat_message(cid),
                Archive::Video(cid) => self.archive_video_segment(cid).await,
            }
        }

        self.finalize().await;

        println!("❌ Archive System Offline");
    }

    /* /// Link chat message to Seconds.
    fn archive_chat_message(&mut self, msg_cid: Cid) {
        let node = match self.video_chat_buffer.as_mut() {
            Some(node) => node,
            None => return,
        };

        node.links_to_chat.push(msg_cid.into());
    } */

    /// Buffers Seconds, waiting for chat messages to be linked.
    async fn archive_video_segment(&mut self, cid: Cid) {
        let second_node = Second {
            link_to_video: cid.into(),
            links_to_chat: Vec::with_capacity(5),
        };

        let node = self.video_chat_buffer.take();

        self.video_chat_buffer = Some(second_node);

        let node = match node {
            Some(node) => node,
            None => return,
        };

        self.collect_second(node).await;

        if self.minute_node.links_to_seconds.len() < 60 {
            return;
        }

        self.collect_minute().await;

        if self.hour_node.links_to_minutes.len() < 60 {
            return;
        }

        self.collect_hour().await;
    }

    /// Create DAG node containing a link to video segment and all chat messages.
    /// Minute is then appended with the CID.
    async fn collect_second(&mut self, node: Second) {
        let cid = match self
            .ipfs
            .dag_put(&node, Codec::default(), Codec::default())
            .await
        {
            Ok(cid) => cid,
            Err(e) => {
                eprintln!("❗ IPFS: dag put failed {}", e);
                return;
            }
        };

        self.minute_node.links_to_seconds.push(cid.into());
    }

    /// Create DAG node containing 60 Second links. Hour is then appended with the CID.
    async fn collect_minute(&mut self) {
        let cid = match self
            .ipfs
            .dag_put(&self.minute_node, Codec::default(), Codec::default())
            .await
        {
            Ok(cid) => cid,
            Err(e) => {
                eprintln!("❗ IPFS: dag put failed {}", e);
                return;
            }
        };

        self.minute_node.links_to_seconds.clear();

        self.hour_node.links_to_minutes.push(cid.into());
    }

    /// Create DAG node containing 60 Minute links. Day is then appended with the CID.
    async fn collect_hour(&mut self) {
        let cid = match self
            .ipfs
            .dag_put(&self.hour_node, Codec::default(), Codec::default())
            .await
        {
            Ok(cid) => cid,
            Err(e) => {
                eprintln!("❗ IPFS: dag put failed {}", e);
                return;
            }
        };

        self.hour_node.links_to_minutes.clear();

        self.day_node.links_to_hours.push(cid.into());
    }

    /// Create all remaining DAG nodes then pin and print the final CID.
    async fn finalize(&mut self) {
        self.archive_rx.close();

        println!("Collecting Nodes...");

        if let Some(node) = self.video_chat_buffer.take() {
            self.collect_second(node).await;
        }

        if !self.minute_node.links_to_seconds.is_empty() {
            self.collect_minute().await;
        }

        if !self.hour_node.links_to_minutes.is_empty() {
            self.collect_hour().await;
        }

        if self.day_node.links_to_hours.is_empty() {
            println!("0 Nodes Found");
            return;
        }

        let cid = match self
            .ipfs
            .dag_put(&self.day_node, Codec::default(), Codec::default())
            .await
        {
            Ok(cid) => cid,
            Err(e) => {
                eprintln!("❗ IPFS: dag put failed {}", e);
                return;
            }
        };

        let stream = Timecode {
            timecode: cid.into(),
        };

        let cid = match self
            .ipfs
            .dag_put(&stream, Codec::default(), Codec::default())
            .await
        {
            Ok(cid) => cid,
            Err(e) => {
                eprintln!("❗ IPFS: dag put failed {}", e);
                return;
            }
        };

        println!("Pinning Nodes...");

        match self.ipfs.pin_add(cid, true).await {
            Ok(_) => println!("Final Timecode-addressable Node => {}", cid.to_string()),
            Err(e) => eprintln!("❗ IPFS: pin add failed {}", e),
        }
    }
}
