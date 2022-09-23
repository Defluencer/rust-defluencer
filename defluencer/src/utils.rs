use crate::errors::Error;

use chrono::{DateTime, Datelike, Timelike, Utc};

use ipfs_api::IpfsService;

use cid::Cid;

#[cfg(target_arch = "wasm32")]
pub async fn add_image(ipfs: &IpfsService, file: web_sys::File) -> Result<Cid, Error> {
    use bytes::Bytes;
    use js_sys::{ArrayBuffer, Uint8Array};
    use wasm_bindgen::JsCast;

    let mime_type = file.type_();

    if !(mime_type == "image/png" || mime_type == "image/jpeg") {
        return Err(Error::Image);
    };

    //let size = file.size() as usize;

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

#[cfg(not(target_arch = "wasm32"))]
pub async fn add_image(ipfs: &IpfsService, path: &std::path::Path) -> Result<Cid, Error> {
    use tokio::fs::File;

    let mime_type = match mime_guess::MimeGuess::from_path(path).first_raw() {
        Some(mime) => mime.to_owned(),
        None => return Err(Error::Image),
    };

    if !(mime_type == "image/png" || mime_type == "image/jpeg") {
        return Err(Error::Image);
    };

    let file = File::open(path).await?;

    let stream = tokio_util::io::ReaderStream::new(file);

    let cid = ipfs.add(stream).await?;

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

    //let size = file.size() as usize;

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

/// A variable-length unsigned integer
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Debug)]
pub struct VarInt(pub u64);

impl VarInt {
    pub fn len(&self) -> usize {
        match self.0 {
            0..=0xFC => 1,
            0xFD..=0xFFFF => 3,
            0x10000..=0xFFFFFFFF => 5,
            _ => 9,
        }
    }

    pub fn consensus_encode(&self) -> Vec<u8> {
        match self.0 {
            0..=0xFC => vec![self.0 as u8],
            0xFD..=0xFFFF => {
                let bytes = (self.0 as u16).to_ne_bytes();
                vec![0xFD, bytes[0], bytes[1]]
            }
            0x10000..=0xFFFFFFFF => {
                let bytes = (self.0 as u32).to_ne_bytes();
                vec![0xFE, bytes[0], bytes[1], bytes[2], bytes[3]]
            }
            _ => {
                let bytes = (self.0 as u64).to_ne_bytes();
                vec![
                    0xFF, bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6],
                    bytes[7],
                ]
            }
        }
    }

    pub fn consensus_decode(data: &[u8]) -> Self {
        match data[0] {
            0xFF => {
                let x = u64::from_ne_bytes([
                    data[1], data[2], data[3], data[4], data[5], data[6], data[7], data[8],
                ]);
                VarInt(x)
            }
            0xFE => {
                let x = u32::from_ne_bytes([data[1], data[2], data[3], data[4]]);
                VarInt(x as u64)
            }
            0xFD => {
                let x = u16::from_ne_bytes([data[1], data[2]]);
                VarInt(x as u64)
            }
            n => VarInt(n as u64),
        }
    }
}
