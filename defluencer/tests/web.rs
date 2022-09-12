#![cfg(target_arch = "wasm32")]

/*
Install wasm-pack first then ->

- Command: wasm-pack test --headless --chrome
- Port is random so local IPFS node must accept any
- Chrome must match chromedriver version

OR

- Command: wasm-pack test --chrome -- --test web
- Open browser to address specified in terminal
- Accept metamask prompt if needed
*/

use wasm_bindgen_test::*;

wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

use web3::{
    transports::eip_1193::{Eip1193, Provider},
    Web3,
};

use defluencer::crypto::ethereum::EthereumSigner;

use cid::Cid;

use defluencer::crypto::Signer;

use signature::DigestVerifier;

use sha3::{Digest, Keccak256};

use gloo_console::info;

#[wasm_bindgen_test]
async fn web_signature() {
    let provider = Provider::default().unwrap();
    let transport = Eip1193::new(provider.unwrap());
    let web3 = Web3::new(transport);

    let addresses = web3.eth().request_accounts().await.unwrap();
    let addr: [u8; 20] = addresses[0].into();

    let signer = EthereumSigner::new(addr, web3);

    let cid = Cid::try_from("bafybeig6xv5nwphfmvcnektpnojts33jqcuam7bmye2pb54adnrtccjlsu").unwrap();
    let signing_input = cid.hash().digest();

    let (key, sig, _hash_algo) = signer.sign(signing_input).await.unwrap();

    let mut eth_message =
        format!("\x19Ethereum Signed Message:\n{}", signing_input.len()).into_bytes();
    eth_message.extend_from_slice(signing_input);

    let digest = Keccak256::new_with_prefix(eth_message);

    info!(&format!("{:?}", sig));

    assert!(key.verify_digest(digest, &sig).is_ok())
}
