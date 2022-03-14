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

    let mut csprng = OsRng::default();

    let priv_key = k256::SecretKey::random(csprng);
    let pub_key = priv_key.public_key();

    let system = ENSSignature::new();
}
