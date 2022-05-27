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

use cid::{multihash::Multihash, Cid};

use prost::Message;

#[cfg(target_arch = "wasm32")]
pub async fn add_image(ipfs: &IpfsService, file: web_sys::File) -> Result<Cid, Error> {
    use futures::AsyncReadExt;
    use wasm_bindgen::JsCast;

    let mime_type = file.type_();

    if !(mime_type == "image/png" || mime_type == "image/jpeg") {
        return Err(Error::Image);
    };

    let size = file.size() as usize;

    // TODO disallow image that are too big.

    let readable_stream = wasm_streams::ReadableStream::from_raw(file.stream().unchecked_into());

    let mut async_read = readable_stream.into_async_read();

    let mut bytes = Vec::with_capacity(size);
    async_read.read_to_end(&mut bytes).await?;

    let mime_typed = MimeTyped {
        mime_type,
        data: either::Either::Right(bytes),
    };

    let cid = ipfs.dag_put(&mime_typed, Codec::default()).await?;

    Ok(cid)
}

#[cfg(not(target_arch = "wasm32"))]
pub async fn add_image(ipfs: &IpfsService, path: &std::path::Path) -> Result<Cid, Error> {
    let mime_type = match mime_guess::MimeGuess::from_path(path).first_raw() {
        Some(mime) => mime.to_owned(),
        None => return Err(Error::Image),
    };

    if !(mime_type == "image/png" || mime_type == "image/jpeg") {
        return Err(Error::Image);
    };

    let file = tokio::fs::File::open(path).await?;
    let stream = tokio_util::io::ReaderStream::new(file);
    let cid = ipfs.add(stream).await?;

    let mime_typed = MimeTyped {
        mime_type,
        data: either::Either::Left(cid.into()),
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
    use futures::AsyncReadExt;
    use wasm_bindgen::JsCast;

    if file.type_() != "text/markdown" {
        return Err(Error::Markdown);
    };

    let size = file.size() as usize;

    let readable_stream = wasm_streams::ReadableStream::from_raw(file.stream().unchecked_into());

    let mut async_read = readable_stream.into_async_read();

    let mut bytes = Vec::with_capacity(size);
    async_read.read_to_end(&mut bytes).await?;
    let bytes = bytes::Bytes::from(bytes);

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
