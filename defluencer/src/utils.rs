use crate::errors::Error;

use chrono::{DateTime, Datelike, Timelike, Utc};

use cid::multibase::Base;

use ipfs_api::{responses::Codec, IpfsService};

use k256::ecdsa::VerifyingKey;

use linked_data::{
    media::mime_type::MimeTyped,
    types::{CryptoKey, IPNSAddress, KeyType},
};

use either::Either;

use cid::Cid;

use multihash::Multihash;

use prost::Message;

#[cfg(target_arch = "wasm32")]
pub async fn add_image(ipfs: &IpfsService, file: web_sys::File) -> Result<Cid, Error> {
    use bytes::Bytes;
    use js_sys::{ArrayBuffer, Uint8Array};
    use wasm_bindgen::JsCast;

    let mime_type = file.type_();

    if !(mime_type == "image/png" || mime_type == "image/jpeg") {
        return Err(Error::Image);
    };

    let size = file.size() as usize;

    let js_value = match wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await {
        Ok(js_value) => js_value,
        Err(js_value) => {
            let error: js_sys::Error = js_value.unchecked_into();
            return Err(Error::JsError(error.to_string()));
        }
    };

    let array_buffer: ArrayBuffer = js_value.unchecked_into();
    let uint8_array: Uint8Array = Uint8Array::new(&array_buffer);
    let vec = uint8_array.to_vec();

    let mime_typed = if size > 1_000_000 {
        let bytes = Bytes::from(vec);
        let cid = ipfs.add(bytes).await?;

        MimeTyped {
            mime_type,
            data: either::Either::Left(cid.into()),
        }
    } else {
        MimeTyped {
            mime_type,
            data: either::Either::Right(vec),
        }
    };

    let cid = ipfs.dag_put(&mime_typed, Codec::default()).await?;

    Ok(cid)
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn add_image(ipfs: &IpfsService, path: &std::path::Path) -> Result<Cid, Error> {
    use tokio::{fs::File, io::AsyncReadExt};

    let mime_type = match mime_guess::MimeGuess::from_path(path).first_raw() {
        Some(mime) => mime.to_owned(),
        None => return Err(Error::Image),
    };

    if !(mime_type == "image/png" || mime_type == "image/jpeg") {
        return Err(Error::Image);
    };

    let mut file = File::open(path).await?;
    let meta = file.metadata().await?;

    let mime_typed = if meta.len() > 1_000_000 {
        let stream = tokio_util::io::ReaderStream::new(file);
        let cid = ipfs.add(stream).await?;

        MimeTyped {
            mime_type,
            data: either::Either::Left(cid.into()),
        }
    } else {
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer).await?;

        MimeTyped {
            mime_type,
            data: either::Either::Right(buffer),
        }
    };

    let cid = ipfs.dag_put(&mime_typed, Codec::default()).await?;

    Ok(cid)
}

/// Add a markdown file to IPFS and return the CID
#[cfg(not(target_arch = "wasm32"))]
pub async fn add_markdown(ipfs: &IpfsService, path: &std::path::Path) -> Result<Cid, Error> {
    let mime_type = match mime_guess::MimeGuess::from_path(path).first_raw() {
        Some(mime) => mime.to_owned(),
        None => return Err(Error::Markdown),
    };

    if mime_type != "text/markdown" {
        return Err(Error::Markdown);
    };

    let file = tokio::fs::File::open(path).await?;
    let stream = tokio_util::io::ReaderStream::new(file);

    let cid = ipfs.add(stream).await?;

    Ok(cid)
}

/// Add a markdown file to IPFS and return the CID
#[cfg(target_arch = "wasm32")]
pub async fn add_markdown(ipfs: &IpfsService, file: web_sys::File) -> Result<Cid, Error> {
    use bytes::Bytes;
    use js_sys::{ArrayBuffer, Uint8Array};
    use wasm_bindgen::JsCast;

    if file.type_() != "text/markdown" {
        return Err(Error::Markdown);
    };

    let size = file.size() as usize;

    let js_value = match wasm_bindgen_futures::JsFuture::from(file.array_buffer()).await {
        Ok(js_value) => js_value,
        Err(js_value) => {
            let error: js_sys::Error = js_value.unchecked_into();
            return Err(Error::JsError(error.to_string()));
        }
    };

    let array_buffer: ArrayBuffer = js_value.unchecked_into();
    let uint8_array: Uint8Array = Uint8Array::new(&array_buffer);
    let vec = uint8_array.to_vec();

    let bytes = Bytes::from(vec);

    let cid = ipfs.add(bytes).await?;

    Ok(cid)
}

pub async fn data_url(ipfs: &IpfsService, mime_type: &MimeTyped) -> Result<String, Error> {
    let mut data_url = String::from("data:");

    data_url.push_str(&mime_type.mime_type);
    data_url.push_str(";base64,");

    let data = match &mime_type.data {
        Either::Right(data) => Base::Base64.encode(data),
        Either::Left(cid) => {
            let data = ipfs.cat(cid.link, Option::<&str>::None).await?;

            Base::Base64.encode(data)
        }
    };

    data_url.push_str(&data);

    Ok(data_url)
}

/// Retrun a path from date time
pub fn get_path(date_time: DateTime<Utc>) -> String {
    format!(
        "year/{}/month/{}/day/{}/hour/{}/minute/{}/second/{}",
        date_time.year(),
        date_time.month(),
        date_time.day(),
        date_time.hour(),
        date_time.minute(),
        date_time.second()
    )
}

pub fn pubkey_to_ipns(public_key: k256::PublicKey) -> IPNSAddress {
    let verifying_key = VerifyingKey::from(public_key);

    let public_key = CryptoKey {
        key_type: KeyType::Secp256k1 as i32,
        data: verifying_key.to_bytes().to_vec(),
    }
    .encode_to_vec(); // Protobuf encoding

    let ipns = {
        let multihash = Multihash::wrap(0x00, &public_key).unwrap();

        Cid::new_v1(0x72, multihash)
    };

    ipns.into()
}
