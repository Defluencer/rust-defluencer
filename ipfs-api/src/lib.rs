pub mod errors;
pub mod responses;

use std::{borrow::Cow, sync::Arc};

use errors::{Error, IPFSError};
use futures_util::{
    future::{AbortRegistration, Abortable},
    AsyncBufReadExt, Stream, StreamExt, TryStreamExt,
};

use serde::{de::DeserializeOwned, Serialize};

use crate::responses::*;

use cid::{
    multibase::{encode, Base},
    Cid,
};

use reqwest::{
    multipart::{Form, Part},
    Client, Response, Url,
};

use bytes::Bytes;

pub const DEFAULT_URI: &str = "http://127.0.0.1:5001/api/v0/";

type Result<T> = std::result::Result<T, Error>;

#[derive(Clone)]
pub struct IpfsService {
    client: Client,
    base_url: Arc<Url>,
}

impl Default for IpfsService {
    fn default() -> Self {
        let base_url = Url::parse(DEFAULT_URI).expect("Pasrsing URI");
        let base_url = Arc::from(base_url);

        let client = Client::new();

        Self { client, base_url }
    }
}

impl IpfsService {
    pub fn new(url: Url) -> Self {
        let base_url = Arc::from(url);

        let client = Client::new();

        Self { client, base_url }
    }

    #[cfg(target_arch = "wasm32")]
    pub async fn add(&self, bytes: Bytes) -> Result<Cid> {
        let url = self.base_url.join("add")?;

        let part = Part::stream(bytes);

        let form = Form::new().part("path", part);

        let bytes = self
            .client
            .post(url)
            .query(&[("pin", "false")])
            .query(&[("cid-version", "1")])
            .multipart(form)
            .send()
            .await?
            .bytes()
            .await?;

        //println!("{}", std::str::from_utf8(&bytes).unwrap());

        if let Ok(res) = serde_json::from_slice::<AddResponse>(&bytes) {
            return Ok(res.try_into()?);
        }

        let error = serde_json::from_slice::<IPFSError>(&bytes)?;

        Err(error.into())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub async fn add<S>(&self, stream: S) -> Result<Cid>
    where
        S: futures_util::stream::TryStream + Send + Sync + 'static,
        S::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
        Bytes: From<S::Ok>,
    {
        let url = self.base_url.join("add")?;

        let body = reqwest::Body::wrap_stream(stream);
        let part = Part::stream(body);

        let form = Form::new().part("path", part);

        let bytes = self
            .client
            .post(url)
            .query(&[("pin", "false")])
            .query(&[("cid-version", "1")])
            .multipart(form)
            .send()
            .await?
            .bytes()
            .await?;

        //println!("{}", std::str::from_utf8(&bytes).unwrap());

        if let Ok(res) = serde_json::from_slice::<AddResponse>(&bytes) {
            return Ok(res.try_into()?);
        }

        let error = serde_json::from_slice::<IPFSError>(&bytes)?;

        Err(error.into())
    }

    /// Download content from block with this CID.
    pub async fn cat<U>(&self, cid: Cid, path: Option<U>) -> Result<Bytes>
    where
        U: Into<Cow<'static, str>>,
    {
        let url = self.base_url.join("cat")?;

        let mut origin = cid.to_string();

        if let Some(path) = path {
            origin.push_str(&path.into());
        }

        let bytes = self
            .client
            .post(url)
            .query(&[("arg", &origin)])
            .send()
            .await?
            .bytes()
            .await?;

        Ok(bytes)
    }

    /// Pin a CID recursively or not.
    pub async fn pin_add(&self, cid: Cid, recursive: bool) -> Result<PinAddResponse> {
        let url = self.base_url.join("pin/add")?;

        let bytes = self
            .client
            .post(url)
            .query(&[("arg", cid.to_string())])
            .query(&[("recursive", &recursive.to_string())])
            .send()
            .await?
            .bytes()
            .await?;

        //println!("{}", std::str::from_utf8(&bytes).unwrap());

        if let Ok(res) = serde_json::from_slice::<PinAddResponse>(&bytes) {
            return Ok(res);
        }

        let error = serde_json::from_slice::<IPFSError>(&bytes)?;

        Err(error.into())
    }

    pub async fn pin_update(&self, old: Cid, new: Cid) -> Result<PinRmResponse> {
        let url = self.base_url.join("pin/update")?;

        let bytes = self
            .client
            .post(url)
            .query(&[("arg", old.to_string())])
            .query(&[("arg", new.to_string())])
            .send()
            .await?
            .bytes()
            .await?;

        //println!("pin_rm Raw => {}", std::str::from_utf8(&bytes).unwrap());

        if let Ok(res) = serde_json::from_slice::<PinRmResponse>(&bytes) {
            return Ok(res);
        }

        let error = serde_json::from_slice::<IPFSError>(&bytes)?;

        Err(error.into())
    }

    /// Remove Pinned CID.
    pub async fn pin_rm(&self, cid: Cid, recursive: bool) -> Result<PinRmResponse> {
        let url = self.base_url.join("pin/rm")?;

        let bytes = self
            .client
            .post(url)
            .query(&[("arg", cid.to_string())])
            .query(&[("recursive", &recursive.to_string())])
            .send()
            .await?
            .bytes()
            .await?;

        //println!("pin_rm Raw => {}", std::str::from_utf8(&bytes).unwrap());

        if let Ok(res) = serde_json::from_slice::<PinRmResponse>(&bytes) {
            return Ok(res);
        }

        let error = serde_json::from_slice::<IPFSError>(&bytes)?;

        Err(error.into())
    }

    pub async fn pin_ls(&self, pin_mode: PinMode) -> Result<PinList> {
        let url = self.base_url.join("pin/ls")?;

        let bytes = self
            .client
            .post(url)
            .query(&[("type", &pin_mode.to_string())])
            .send()
            .await?
            .bytes()
            .await?;

        //println!("pin_ls Raw => {}", std::str::from_utf8(&bytes).unwrap());

        if let Ok(res) = serde_json::from_slice::<PinLsResponse>(&bytes) {
            return Ok(res.into());
        }

        let error = serde_json::from_slice::<IPFSError>(&bytes)?;

        Err(error.into())
    }

    /// Serialize then add dag node to IPFS. Return a CID.
    pub async fn dag_put<T>(&self, node: &T) -> Result<Cid>
    where
        T: ?Sized + Serialize,
    {
        let data = serde_json::to_vec(node)?;
        let part = Part::bytes(data);
        let form = Form::new().part("object data", part);

        let url = self.base_url.join("dag/put")?;

        let bytes = self
            .client
            .post(url)
            .query(&[("store-codec", "dag-cbor")])
            .query(&[("input-codec", "dag-json")])
            .query(&[("pin", "false")])
            .multipart(form)
            .send()
            .await?
            .bytes()
            .await?;

        //println!("{}", std::str::from_utf8(&bytes).unwrap());

        if let Ok(res) = serde_json::from_slice::<DagPutResponse>(&bytes) {
            return Ok(res.try_into()?);
        }

        let error = serde_json::from_slice::<IPFSError>(&bytes)?;

        Err(error.into())
    }

    /// Deserialize dag node from IPFS path. Return dag node.
    pub async fn dag_get<U, T>(&self, cid: Cid, path: Option<U>) -> Result<T>
    where
        U: Into<Cow<'static, str>>,
        T: ?Sized + DeserializeOwned,
    {
        let mut origin = cid.to_string();

        if let Some(path) = path {
            origin.push_str(&path.into());
        }

        let url = self.base_url.join("dag/get")?;

        let bytes = self
            .client
            .post(url)
            .query(&[("arg", &origin)])
            .query(&[("output-codec", "dag-json")])
            .send()
            .await?
            .bytes()
            .await?;

        //println!("{}", std::str::from_utf8(&bytes).unwrap());

        if let Ok(res) = serde_json::from_slice::<T>(&bytes) {
            return Ok(res);
        }

        let error = serde_json::from_slice::<IPFSError>(&bytes)?;

        Err(error.into())
    }

    pub async fn key_gen(&self, name: impl Into<Cow<'static, str>>) -> Result<KeyPair> {
        let url = self.base_url.join("key/gen")?;

        let bytes = self
            .client
            .post(url)
            .query(&[("arg", name.into())])
            .query(&[("ipns-base", "base32")])
            .send()
            .await?
            .bytes()
            .await?;

        //println!("{}", std::str::from_utf8(&bytes).unwrap());

        if let Ok(res) = serde_json::from_slice::<KeyPair>(&bytes) {
            return Ok(res);
        }

        let error = serde_json::from_slice::<IPFSError>(&bytes)?;

        Err(error.into())
    }

    /// Returns all IPNS keys on this IPFS node.
    pub async fn key_list(&self) -> Result<KeyList> {
        let url = self.base_url.join("key/list")?;

        let bytes = self
            .client
            .post(url)
            .query(&[("l", "true"), ("ipns-base", "base32")])
            .send()
            .await?
            .bytes()
            .await?;

        //println!("{}", std::str::from_utf8(&bytes).unwrap());

        if let Ok(res) = serde_json::from_slice::<KeyListResponse>(&bytes) {
            return Ok(res.into());
        }

        let error = serde_json::from_slice::<IPFSError>(&bytes)?;

        Err(error.into())
    }

    /// Publish new IPNS record.
    pub async fn name_publish<U>(&self, cid: Cid, key: U) -> Result<NamePublishResponse>
    where
        U: Into<Cow<'static, str>>,
    {
        let url = self.base_url.join("name/publish")?;

        let bytes = self
            .client
            .post(url)
            .query(&[("arg", cid.to_string())])
            .query(&[("lifetime", "4320h")]) // 6 months
            .query(&[("key", &key.into())])
            .query(&[("ipns-base", "base32")])
            .send()
            .await?
            .bytes()
            .await?;

        //println!("{}", std::str::from_utf8(&bytes).unwrap());

        if let Ok(res) = serde_json::from_slice::<NamePublishResponse>(&bytes) {
            return Ok(res);
        }

        let error = serde_json::from_slice::<IPFSError>(&bytes)?;

        Err(error.into())
    }

    /// Resolve IPNS name. Returns CID.
    pub async fn name_resolve(&self, ipns: Cid) -> Result<Cid> {
        let url = self.base_url.join("name/resolve")?;

        let bytes = self
            .client
            .post(url)
            .query(&[("arg", ipns.to_string())])
            .send()
            .await?
            .bytes()
            .await?;

        //println!("{}", std::str::from_utf8(&bytes).unwrap());

        if let Ok(res) = serde_json::from_slice::<NameResolveResponse>(&bytes) {
            return Ok(res.try_into()?);
        }

        let error = serde_json::from_slice::<IPFSError>(&bytes)?;

        Err(error.into())
    }

    /// Get node associated with IPNS key.
    pub async fn ipns_get<T>(&self, key: impl Into<Cow<'static, str>>) -> Result<(Cid, T)>
    where
        T: ?Sized + DeserializeOwned,
    {
        let key: &str = &key.into();

        let key_list = self.key_list().await?;

        let cid = match key_list.get(key) {
            Some(keypair) => *keypair,
            None => return Err(Error::Ipns),
        };

        let cid = self.name_resolve(cid).await?;

        let node = self.dag_get(cid, Option::<&str>::None).await?;

        Ok((cid, node))
    }

    /// Serialize the new node, update the pin then publish under this IPNS key.
    pub async fn ipns_update<T>(
        &self,
        key: impl Into<Cow<'static, str>>,
        old_cid: Cid,
        content: &T,
    ) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        let cid = self.dag_put(content).await?;

        self.pin_update(cid, old_cid).await?;

        self.name_publish(cid, key).await?;

        Ok(())
    }

    /// Serialize the new node, pin then publish under this IPNS key.
    pub async fn ipns_put<T>(
        &self,
        key: impl Into<Cow<'static, str>>,
        recursive: bool,
        content: &T,
    ) -> Result<()>
    where
        T: ?Sized + Serialize,
    {
        let cid = self.dag_put(content).await?;

        self.pin_add(cid, recursive).await?;

        self.name_publish(cid, key).await?;

        Ok(())
    }

    ///Return peer id as cid v1.
    pub async fn peer_id(&self) -> Result<Cid> {
        let url = self.base_url.join("id")?;

        let bytes = self.client.post(url).send().await?.bytes().await?;

        //println!("{}", std::str::from_utf8(&bytes).unwrap());

        if let Ok(res) = serde_json::from_slice::<IdResponse>(&bytes) {
            return Ok(res.try_into()?);
        }

        let error = serde_json::from_slice::<IPFSError>(&bytes)?;

        Err(error.into())
    }

    /// Send data on the specified topic.
    pub async fn pubsub_pub<T, D>(&self, topic: T, data: D) -> Result<()>
    where
        T: AsRef<[u8]>,
        D: Into<Cow<'static, [u8]>>,
    {
        let url = self.base_url.join("pubsub/pub")?;

        let topic = encode(Base::Base64Url, topic);

        let part = Part::bytes(data);
        let form = Form::new().part("data", part);

        self.client
            .post(url)
            .query(&[("arg", &topic)])
            .multipart(form)
            .send()
            .await?;

        Ok(())
    }

    pub async fn pubsub_sub<T>(&self, topic: T) -> Result<Response>
    where
        T: AsRef<[u8]>,
    {
        let url = self.base_url.join("pubsub/sub")?;

        let topic = encode(Base::Base64Url, topic);

        let response = self
            .client
            .post(url)
            .query(&[("arg", topic)])
            .send()
            .await?;

        Ok(response)
    }
}

pub fn pubsub_stream(
    response: Response,
    regis: AbortRegistration,
) -> impl Stream<Item = Result<PubSubMessage>> {
    let stream = response.bytes_stream();

    let abortable_stream = Abortable::new(stream, regis);

    //TODO implement from reqwest error for std::io::Error
    let line_stream = abortable_stream
        //.err_into()
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error))
        .into_async_read()
        .lines();

    line_stream.map(|item| match item {
        Ok(line) => {
            if let Ok(response) = serde_json::from_str::<PubsubSubResponse>(&line) {
                return Ok(response.try_into()?);
            }

            let ipfs_error = serde_json::from_str::<IPFSError>(&line)?;

            return Err(ipfs_error.into());
        }
        Err(e) => Err(e.into()),
    })
}
