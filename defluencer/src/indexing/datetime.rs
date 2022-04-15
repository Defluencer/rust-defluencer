use std::collections::HashSet;

use crate::errors::Error;

use chrono::{DateTime, Datelike, Timelike, Utc};

use cid::Cid;

use ipfs_api::{responses::Codec, IpfsService};

use linked_data::{indexes::date_time::*, types::IPLDLink};

pub(crate) async fn insert(
    ipfs: &IpfsService,
    date_time: DateTime<Utc>,
    index: &mut Option<IPLDLink>,
    add_cid: Cid,
) -> Result<bool, Error> {
    let mut yearly = Yearly::default();
    let mut monthly = Monthly::default();
    let mut daily = Daily::default();
    let mut hourly = Hourly::default();
    let mut minutes = Minutes::default();
    let mut seconds = Seconds::default();

    if let Some(index) = index {
        yearly = ipfs.dag_get::<&str, Yearly>(index.link, None).await?;
    }

    if let Some(ipld) = yearly.year.get(&date_time.year()) {
        monthly = ipfs.dag_get::<&str, Monthly>(ipld.link, None).await?;
    }

    if let Some(ipld) = monthly.month.get(&date_time.month()) {
        daily = ipfs.dag_get::<&str, Daily>(ipld.link, None).await?;
    }

    if let Some(ipld) = daily.day.get(&date_time.day()) {
        hourly = ipfs.dag_get::<&str, Hourly>(ipld.link, None).await?;
    }

    if let Some(ipld) = hourly.hour.get(&date_time.hour()) {
        minutes = ipfs.dag_get::<&str, Minutes>(ipld.link, None).await?;
    }

    if let Some(ipld) = minutes.minute.get(&date_time.minute()) {
        seconds = ipfs.dag_get::<&str, Seconds>(ipld.link, None).await?;
    }

    let mut result = false;

    seconds
        .second
        .entry(date_time.second())
        .and_modify(|set| {
            result = set.insert(add_cid.into());
        })
        .or_insert({
            let mut set = HashSet::with_capacity(1);
            result = set.insert(add_cid.into());
            set
        });

    let cid = ipfs.dag_put(&seconds, Codec::default()).await?;

    minutes.minute.insert(date_time.minute(), cid.into());
    let cid = ipfs.dag_put(&minutes, Codec::default()).await?;

    hourly.hour.insert(date_time.hour(), cid.into());
    let cid = ipfs.dag_put(&hourly, Codec::default()).await?;

    daily.day.insert(date_time.day(), cid.into());
    let cid = ipfs.dag_put(&daily, Codec::default()).await?;

    monthly.month.insert(date_time.month(), cid.into());
    let cid = ipfs.dag_put(&monthly, Codec::default()).await?;

    yearly.year.insert(date_time.year(), cid.into());
    let cid = ipfs.dag_put(&yearly, Codec::default()).await?;

    *index = Some(cid.into());

    Ok(result)
}

pub(crate) async fn remove(
    ipfs: &IpfsService,
    date_time: DateTime<Utc>,
    index: &mut Option<IPLDLink>,
    remove_cid: Cid,
) -> Result<bool, Error> {
    let idx = match index {
        Some(idx) => idx,
        None => return Ok(false),
    };

    let mut yearly = ipfs.dag_get::<&str, Yearly>(idx.link, None).await?;

    let mut monthly = match yearly.year.get(&date_time.year()) {
        Some(ipld) => ipfs.dag_get::<&str, Monthly>(ipld.link, None).await?,
        None => return Ok(false),
    };

    let mut daily = match monthly.month.get(&date_time.month()) {
        Some(ipld) => ipfs.dag_get::<&str, Daily>(ipld.link, None).await?,
        None => return Ok(false),
    };

    let mut hourly = match daily.day.get(&date_time.day()) {
        Some(ipld) => ipfs.dag_get::<&str, Hourly>(ipld.link, None).await?,
        None => return Ok(false),
    };

    let mut minutes = match hourly.hour.get(&date_time.hour()) {
        Some(ipld) => ipfs.dag_get::<&str, Minutes>(ipld.link, None).await?,
        None => return Ok(false),
    };

    let mut seconds = match minutes.minute.get(&date_time.minute()) {
        Some(ipld) => ipfs.dag_get::<&str, Seconds>(ipld.link, None).await?,
        None => return Ok(false),
    };

    let set = match seconds.second.get_mut(&date_time.second()) {
        Some(set) => set,
        None => return Ok(false),
    };

    let result = set.remove(&remove_cid.into());

    if set.is_empty() {
        seconds.second.remove(&date_time.second());
    }

    if seconds.second.is_empty() {
        minutes.minute.remove(&date_time.minute());
    } else {
        let cid = ipfs.dag_put(&seconds, Codec::default()).await?;

        minutes.minute.insert(date_time.minute(), cid.into());
    }

    if minutes.minute.is_empty() {
        hourly.hour.remove(&date_time.hour());
    } else {
        let cid = ipfs.dag_put(&minutes, Codec::default()).await?;

        hourly.hour.insert(date_time.hour(), cid.into());
    }

    if hourly.hour.is_empty() {
        daily.day.remove(&date_time.day());
    } else {
        let cid = ipfs.dag_put(&hourly, Codec::default()).await?;

        daily.day.insert(date_time.day(), cid.into());
    }

    if daily.day.is_empty() {
        monthly.month.remove(&date_time.month());
    } else {
        let cid = ipfs.dag_put(&daily, Codec::default()).await?;

        monthly.month.insert(date_time.month(), cid.into());
    }

    if monthly.month.is_empty() {
        yearly.year.remove(&date_time.year());
    } else {
        let cid = ipfs.dag_put(&monthly, Codec::default()).await?;

        yearly.year.insert(date_time.year(), cid.into());
    }

    let cid = ipfs.dag_put(&yearly, Codec::default()).await?;

    *index = Some(cid.into());

    Ok(result)
}
