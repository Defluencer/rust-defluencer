/* #![cfg(target_arch = "wasm32")]

/*
Install wasm-pack first then ->

- Command: wasm-pack test --headless --chrome
- Port is random so local IPFS node must accept any
- Chrome must match chromedriver version
*/

use std::assert_eq;

use wasm_bindgen_test::*;

wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

use cid::{multibase::Base, multihash::MultihashGeneric, Cid};
use futures_util::{self, future::AbortHandle, future::FutureExt, join, StreamExt};
use ipfs_api::IpfsService;

const PEER_ID: &str = "12D3KooWRsEKtLGLW9FHw7t7dDhHrMDahw3VwssNgh55vksdvfmC";

#[wasm_bindgen_test]
async fn id() {
    let decoded = Base::Base58Btc.decode(PEER_ID).unwrap();
    let multihash = MultihashGeneric::from_bytes(&decoded).unwrap();
    let cid = Cid::new_v1(0x70, multihash);

    let ipfs = IpfsService::default();

    match ipfs.peer_id().await {
        Ok(res) => assert_eq!(res, cid),
        Err(e) => panic!("{}", e),
    }
}

const TOPIC: &str = "test";
const MSG: &str = "Hello World!";

#[wasm_bindgen_test]
async fn pubsub_roundtrip() {
    let decoded = Base::Base58Btc.decode(PEER_ID).unwrap();
    let multihash = MultihashGeneric::from_bytes(&decoded).unwrap();
    let peer_id = Cid::new_v1(0x70, multihash);

    let ipfs = IpfsService::default();

    let publish = ipfs.pubsub_pub(TOPIC, MSG.as_bytes()).fuse();

    let subscribe = async {
        let ipfs = IpfsService::default();

        let res = ipfs.pubsub_sub_response(TOPIC).await.unwrap();

        let (_, regis) = AbortHandle::new_pair();

        let mut stream = ipfs_multi_client::pubsub_sub_stream(res, regis)
            .take(1)
            .fuse();

        stream.next().await.unwrap()
    };

    let (res, _) = join!(subscribe, publish);

    let msg = res.unwrap();

    assert_eq!(msg.from, peer_id);
    assert_eq!(MSG, String::from_utf8(msg.data).unwrap());
}

use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug, PartialEq)]
struct TestBlock {
    data: String,
}

#[wasm_bindgen_test]
async fn dag_roundtrip() {
    let ipfs = IpfsService::default();

    let node = TestBlock {
        data: String::from("This is a test"),
    };

    let cid = ipfs.dag_put(&node).await.unwrap();

    let new_node: TestBlock = ipfs.dag_get(cid, Option::<&str>::None).await.unwrap();

    assert_eq!(node, new_node)
}

const SELF_KEY: &str = "bafzaajaiaejcb3tw3wtri7mxd66jsfeowj627zaktxbssmjykbwyzcqsmm46fbdd";

#[wasm_bindgen_test]
async fn key_listing() {
    let ipfs = IpfsService::default();

    let self_cid = Cid::try_from(SELF_KEY).unwrap();

    let list = ipfs.key_list().await.unwrap();

    assert_eq!(list["self"], self_cid)
}

const TEST_CID: &str = "bafyreiejplp7y57dxnasxk7vjdujclpe5hzudiqlgvnit4vinqvtehh3ci";

/* #[wasm_bindgen_test]
async fn name_publish() {
    let ipfs = IpfsService::default();

    let cid = Cid::try_from(TEST_CID).unwrap();

    match ipfs.name_publish(cid, "self").await {
        Ok(res) => assert_eq!(res.value, format!("/ipfs/{}", TEST_CID)),
        Err(e) => panic!("{:?}", e),
    }
} */

#[wasm_bindgen_test]
async fn pin_roundtrip() {
    let ipfs = IpfsService::default();

    let cid = Cid::try_from(TEST_CID).unwrap();

    match ipfs.pin_add(cid, false).await {
        Ok(res) => assert_eq!(res.pins[0], TEST_CID),
        Err(e) => panic!("{:?}", e),
    }

    match ipfs.pin_rm(cid, false).await {
        Ok(res) => assert_eq!(res.pins[0], TEST_CID),
        Err(e) => panic!("{:?}", e),
    }
}

#[wasm_bindgen_test]
async fn add_cat_roundtrip() {
    //use js_sys::{Array, Uint8Array};
    //use wasm_bindgen::{JsCast, JsValue};
    //use wasm_streams::readable::ReadableStream;
    //use web_sys::Blob;
    use bytes::Bytes;

    let ipfs = IpfsService::default();

    let in_data = b"Hello World!";
    let bytes = Bytes::copy_from_slice(in_data);

    /* let u8_array = Uint8Array::new_with_length(in_data.len() as u32);
    u8_array.copy_from(in_data);

    let array = Array::new();
    array.push(&u8_array);
    let blob = Blob::new_with_u8_array_sequence(&array).expect("Blob Construction");

    let stream = ReadableStream::from_raw(blob.stream().unchecked_into()); */

    let cid = ipfs.add(bytes).await.unwrap();

    let out_data = ipfs.cat(cid, Option::<&str>::None).await.unwrap();

    assert_eq!(in_data, &out_data[0..12])
}
 */
