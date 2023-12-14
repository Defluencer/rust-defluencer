use crate::actors::{SetupData, VideoData};

use std::{fmt::Debug, path::Path};

use futures_util::StreamExt;
use tokio::sync::mpsc::UnboundedSender;

use hyper::{
    body::{Bytes, Incoming},
    header::{HeaderValue, LOCATION},
    Error, Method, Request, Response, StatusCode,
};

use http_body_util::{BodyExt, BodyStream, Empty};

use ipfs_api::IpfsService;

use m3u8_rs::Playlist;

const M3U8: &str = "m3u8";
pub const MP4: &str = "mp4";
pub const M4S: &str = "m4s";

pub async fn put_requests(
    req: Request<Incoming>,
    video_tx: UnboundedSender<VideoData>,
    setup_tx: UnboundedSender<SetupData>,
    ipfs: IpfsService,
) -> Result<Response<Empty<Bytes>>, Error> {
    #[cfg(debug_assertions)]
    println!("Service: {:#?}", req);

    let mut res = Response::new(Empty::new());

    let (parts, body) = req.into_parts();

    let path = Path::new(parts.uri.path());

    if parts.method != Method::PUT
        || path.extension() == None
        || (path.extension().unwrap() != M3U8
            && path.extension().unwrap() != M4S
            && path.extension().unwrap() != MP4)
    {
        return not_found_response(res);
    }

    let body_stream = BodyStream::new(body);

    if path.extension().unwrap() == M3U8 {
        return manifest_response(res, body_stream, path, setup_tx).await;
    }

    //Map frames to bytes dropping trailers frame
    let byte_stream = body_stream.filter_map(|res| async move {
        match res {
            Ok(frame) => match frame.into_data() {
                Ok(bytes) => Some(Ok(bytes)),
                Err(_) => None,
            },
            Err(e) => Some(Err(e)),
        }
    });

    let cid = match ipfs.add(byte_stream).await {
        Ok(res) => res,
        Err(error) => return internal_error_response(res, &error),
    };

    #[cfg(debug_assertions)]
    println!("IPFS: add => {}", &cid.to_string());

    if path.extension().unwrap() == M4S {
        let msg = VideoData::Segment((path.to_path_buf(), cid));

        if let Err(error) = video_tx.send(msg) {
            return internal_error_response(res, &error);
        }
    } else if path.extension().unwrap() == MP4 {
        let msg = SetupData::Segment((path.to_path_buf(), cid));

        if let Err(error) = setup_tx.send(msg) {
            return internal_error_response(res, &error);
        }
    }

    *res.status_mut() = StatusCode::CREATED;

    let header_value = HeaderValue::from_str(parts.uri.path()).expect("Invalid Header Value");

    res.headers_mut().insert(LOCATION, header_value);

    #[cfg(debug_assertions)]
    println!("Service: {:#?}", res);

    Ok(res)
}

fn not_found_response(mut res: Response<Empty<Bytes>>) -> Result<Response<Empty<Bytes>>, Error> {
    *res.status_mut() = StatusCode::NOT_FOUND;

    #[cfg(debug_assertions)]
    println!("Service: {:#?}", res);

    Ok(res)
}

async fn manifest_response(
    mut res: Response<Empty<Bytes>>,
    body: BodyStream<Incoming>,
    path: &Path,
    setup_tx: UnboundedSender<SetupData>,
) -> Result<Response<Empty<Bytes>>, Error> {
    let bytes = BodyExt::collect(body).await?.to_bytes();

    let playlist = match m3u8_rs::parse_playlist(&bytes) {
        Ok((_, playlist)) => playlist,
        Err(e) => return internal_error_response(res, &e),
    };

    if let Playlist::MasterPlaylist(playlist) = playlist {
        let msg = SetupData::Playlist(playlist);

        if let Err(error) = setup_tx.send(msg) {
            return internal_error_response(res, &error);
        }
    }

    *res.status_mut() = StatusCode::NO_CONTENT;

    let header_value = HeaderValue::from_str(path.to_str().unwrap()).unwrap();

    res.headers_mut().insert(LOCATION, header_value);

    #[cfg(debug_assertions)]
    println!("Service: {:#?}", res);

    Ok(res)
}

fn internal_error_response(
    mut res: Response<Empty<Bytes>>,
    error: &dyn Debug,
) -> Result<Response<Empty<Bytes>>, Error> {
    eprintln!("Service: {:#?}", error);

    *res.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;

    #[cfg(debug_assertions)]
    println!("Service: {:#?}", res);

    Ok(res)
}
