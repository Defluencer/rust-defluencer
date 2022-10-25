#![cfg(not(target_arch = "wasm32"))]

#[cfg(test)]
mod tests {
    use bytes::Bytes;
    use cid::Cid;
    use futures_util::{future::FutureExt, stream, StreamExt};
    use ipfs_api::{
        responses::{Codec, PinMode},
        IpfsService,
    };

    const PEER_ID: &str = "12D3KooWRsEKtLGLW9FHw7t7dDhHrMDahw3VwssNgh55vksdvfmC";

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn id() {
        let peer_id = PeerId::try_from(PEER_ID).unwrap();

        let ipfs = IpfsService::default();

        match ipfs.peer_id().await {
            Ok(res) => assert_eq!(res, peer_id),
            Err(e) => panic!("{}", e),
        }
    }

    const TOPIC: &str = "test";
    const MSG: &str = "Hello World!";

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn pubsub_roundtrip() {
        let peer_id = PeerId::try_from(PEER_ID).unwrap();

        let ipfs = IpfsService::default();

        let publish = ipfs.pubsub_pub(TOPIC, MSG.as_bytes()).fuse();

        let subscribe = async {
            let ipfs = IpfsService::default();

            let mut stream = ipfs
                .pubsub_sub(TOPIC.as_bytes().to_owned())
                .take(1)
                .boxed_local();

            stream.next().await.unwrap()
        };

        let (res, _) = tokio::join!(subscribe, publish);

        let msg = res.unwrap();

        assert_eq!(peer_id, msg.from);
        assert_eq!(MSG, String::from_utf8(msg.data).unwrap());
    }

    use linked_data::types::{IPNSAddress, PeerId};
    use serde::{Deserialize, Serialize};

    #[derive(Deserialize, Serialize, Debug, PartialEq)]
    struct TestBlock {
        data: String,
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn dag_roundtrip() {
        let ipfs = IpfsService::default();

        let node = TestBlock {
            data: String::from("This is a test"),
        };

        let cid = ipfs.dag_put(&node, Codec::default()).await.unwrap();

        let new_node: TestBlock = ipfs.dag_get(cid, Option::<&str>::None).await.unwrap();

        assert_eq!(node, new_node)
    }

    const SELF_KEY: &str = "bafzaajaiaejcb3tw3wtri7mxd66jsfeowj627zaktxbssmjykbwyzcqsmm46fbdd";

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn key_listing() {
        let self_cid = IPNSAddress::try_from(SELF_KEY).unwrap();

        let ipfs = IpfsService::default();

        let list = ipfs.key_list().await.unwrap();

        assert_eq!(self_cid, list["self"])
    }

    const TEST_CID: &str = "bafyreiejplp7y57dxnasxk7vjdujclpe5hzudiqlgvnit4vinqvtehh3ci";

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore]
    async fn name_publish() {
        let cid = Cid::try_from(TEST_CID).unwrap();

        let ipfs = IpfsService::default();

        match ipfs.name_publish(cid, "self").await {
            Ok(res) => assert_eq!(res.value, format!("/ipfs/{}", TEST_CID)),
            Err(e) => panic!("{:?}", e),
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn pin_roundtrip() {
        let cid = Cid::try_from(TEST_CID).unwrap();

        let ipfs = IpfsService::default();

        match ipfs.pin_add(cid, false).await {
            Ok(res) => assert_eq!(res.pins[0], TEST_CID),
            Err(e) => panic!("{:?}", e),
        }

        match ipfs.pin_rm(cid, false).await {
            Ok(res) => assert_eq!(res.pins[0], TEST_CID),
            Err(e) => panic!("{:?}", e),
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn add_cat_roundtrip() {
        let ipfs = IpfsService::default();

        let data: Vec<Result<Bytes, reqwest::Error>> = vec![
            Ok(Bytes::from_static(b"Hello ")),
            Ok(Bytes::from_static(b"World!")),
        ];

        let stream = stream::iter(data);

        let cid = ipfs.add(stream).await.unwrap();

        let data = ipfs.cat(cid, Option::<&str>::None).await.unwrap();

        assert_eq!(b"Hello World!", &data[0..12])
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn pin_ls() {
        let ipfs = IpfsService::default();

        let res = ipfs.pin_ls(PinMode::Recursive).await.unwrap();

        println!("{:?}", res);
    }
}
