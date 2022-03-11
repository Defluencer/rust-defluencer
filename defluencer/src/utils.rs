use crate::errors::Error;

use cid::multibase::Base;

use ipfs_api::IpfsService;

use linked_data::media::mime_type::MimeTyped;

use either::Either;

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
