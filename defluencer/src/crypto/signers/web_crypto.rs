#![cfg(target_arch = "wasm32")]

use async_trait::async_trait;

use js_sys::{Array, ArrayBuffer, Object, Uint8Array, JSON};

use multibase::Base;

use async_signature::AsyncSigner;

use wasm_bindgen::{JsCast, JsValue, UnwrapThrowExt};

use wasm_bindgen_futures::JsFuture;

use web_sys::{window, Crypto, CryptoKeyPair, SubtleCrypto};

use dag_jose::{AlgorithmType, AsyncBlockSigner, CurveType, JsonWebKey, KeyType};

use p256::ecdsa::Signature;

use ipns_records::{AsyncRecordSigner, CryptoKey};

use rexie::{ObjectStore, Rexie, RexieBuilder};

#[derive(Clone)]
pub struct WebSigner {
    db: Rexie,
    store_name: String,
    db_key: JsValue,
}

impl WebSigner {
    /// Create a new CryptoKey in the Browser and then save it to local storage.
    async fn new(store_name: String) -> Self {
        let window = window().unwrap_throw();
        let crypto = window.crypto().unwrap_throw();
        let subtle = crypto.subtle();

        let algorithm = JSON::parse(r#"{ name: "ECDSA", namedCurve: "P-256" }"#).unwrap_throw();
        let algorithm = Object::from(algorithm);

        let key_usages = Array::of2(&JsValue::from_str("sign"), &JsValue::from_str("verify"));

        let promise = subtle
            .generate_key_with_object(&algorithm, false, &key_usages)
            .unwrap_throw();

        let key_pair: CryptoKeyPair = JsFuture::from(promise)
            .await
            .unwrap_throw()
            .unchecked_into();

        let rexie = Rexie::builder("defluencer")
            .version(1)
            .add_object_store(
                ObjectStore::new(&store_name)
                    .key_path("name")
                    .auto_increment(true),
            )
            .build()
            .await
            .unwrap_throw();

        let transaction = rexie
            .transaction(&[store_name], TransactionMode::ReadWrite)
            .unwrap_throw();
        let db_store = transaction.store(&store_name).unwrap_throw();

        let db_key = db_store.add(&key_pair, None).await.unwrap_throw();

        transaction.done().await.unwrap_throw();

        Self {
            db: rexie,
            store_name,
            db_key,
        }
    }

    fn get_key_pair(&self) -> CryptoKeyPair {
        let transaction = self
            .db
            .transaction(&[self.store_name], TransactionMode::ReadOnly)
            .unwrap_throw();
        let db_store = transaction.store(&self.store_name).unwrap_throw();
        let key_pair: CryptoKeyPair = db_store
            .get(&self.db_key.into())
            .await
            .unwrap_throw()
            .unchecked_into();

        let public_key =
            js_sys::Reflect::get(&key_pair, &JsValue::from("publicKey")).unwrap_throw();
        let private_key: web_sys::CryptoKey = private_key.unchecked_into();
    }

    fn get_pubkey(&self, key_pair: CryptoKeyPair) -> CryptoKey {
        js_sys::Reflect::get(&key_pair, &JsValue::from("publicKey"))
            .unwrap_throw()
            .unchecked_into()
    }

    fn get_privkey(&self, key_pair: CryptoKeyPair) -> CryptoKey {
        js_sys::Reflect::get(&key_pair, &JsValue::from("privateKey"))
            .unwrap_throw()
            .unchecked_into()
    }
}

#[async_trait]
impl AsyncSigner<Signature> for WebSigner {
    async fn sign_async(&self, msg: &[u8]) -> Result<Signature, signature::Error> {
        let window = window().unwrap_throw();
        let crypto = window.crypto().unwrap_throw();
        let subtle = crypto.subtle();

        let algorithm = JSON::parse(r#"{ name: "ECDSA", hash: {name: "SHA-384"} }"#).unwrap_throw();
        let algorithm = Object::from(algorithm);

        let private_key = self.get_privkey(self.get_key_pair());

        let buf = Uint8Array::new_with_length(msg.len() as u32);
        buf.copy_from(&msg);

        let promise = subtle
            .sign_with_object_and_buffer_source(&algorithm, &private_key, &buf)
            .unwrap_throw();

        let result = JsFuture::from(promise).await.unwrap_throw();
        let buffer: Uint8Array = result.unchecked_into();

        let mut vec = Vec::with_capacity(buffer.length() as usize);
        buffer.copy_to(&mut vec);

        let sig = Signature::from_bytes(&vec);

        Ok(sig)
    }
}

#[async_trait]
impl AsyncBlockSigner<Signature> for WebSigner {
    fn algorithm(&self) -> AlgorithmType {
        AlgorithmType::ES256
    }

    async fn web_key(&self) -> JsonWebKey {
        let window = window().unwrap_throw();
        let crypto = window.crypto().unwrap_throw();
        let subtle = crypto.subtle();

        let pubkey = self.get_pubkey(self.get_key_pair());

        let promise = subtle.export_key("jwk", &pubkey).unwrap_throw();
        let result = JsFuture::from(promise).await.unwrap_throw();

        let js_string = JSON::stringify(&result).unwrap_throw();

        serde_json::from_str(&js_string.into()).unwrap()
    }
}

#[async_trait]
impl AsyncRecordSigner<Signature> for WebSigner {
    fn crypto_key(&self) -> CryptoKey {
        todo!()
    }
}
