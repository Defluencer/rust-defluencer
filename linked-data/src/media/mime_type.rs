use crate::types::IPLDLink;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct MimeTyped {
    pub mime_type: String,

    pub data: IPLDLink,
}
