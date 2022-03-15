#![cfg(target_arch = "wasm32")]

/*
Install wasm-pack first then ->

- Command: wasm-pack test --headless --chrome
- Port is random so local IPFS node must accept any
- Chrome must match chromedriver version
*/

use std::assert_eq;

use ipfs_api::IpfsService;

use rand_core::OsRng;

use wasm_bindgen_test::*;

wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[wasm_bindgen_test]
async fn k256_roundtrip() {
    let ipfs = IpfsService::default();

    let provider = Provider::default().unwrap();
    let transport = Eip1193::new(provider);
    let web3 = Web3::new(transport);

    //TODO get address from metamask

    let system = EthereumSigner::new(ipfs.clone(), addr, web3);

    let cid = Cid::try_from("bafybeig6xv5nwphfmvcnektpnojts33jqcuam7bmye2pb54adnrtccjlsu").unwrap();

    let result = system.sign(cid).await.unwrap();

    //TODO print

    let raw: RawJWS = ipfs.dag_get(result, Option::<&str>::None).await.unwrap();

    let jws: JsonWebSignature = raw.try_into().unwrap();

    let result = jws.verify();

    assert!(result.is_ok());
}
