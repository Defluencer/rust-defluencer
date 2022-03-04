use crate::IPLDLink;

use std::borrow::Cow;

use serde::{Deserialize, Serialize};

use cid::Cid;

use either::Either;

#[derive(Serialize, Deserialize)]
pub struct MimeTyped {
    pub mime_type: String,
    pub data: Either<IPLDLink, Vec<u8>>,
}

impl MimeTyped {
    pub fn new_linked_data(mime_type: impl Into<Cow<'static, str>>, cid: Cid) -> Self {
        Self {
            mime_type: mime_type.into().into_owned(),
            data: Either::Left(cid.into()),
        }
    }

    pub fn new_raw_data(mime_type: impl Into<Cow<'static, str>>, data: Vec<u8>) -> Self {
        Self {
            mime_type: mime_type.into().into_owned(),
            data: Either::Right(data),
        }
    }
}
