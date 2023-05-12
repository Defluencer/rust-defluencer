use std::collections::HashSet;

use crate::errors::Error;

use chrono::{DateTime, Datelike, Timelike, Utc};

use cid::Cid;

use ipfs_api::{responses::Codec, IpfsService};

use linked_data::{indexes::date_time::*, types::IPLDLink};

/// Adds a value to the index.
/// Returns whether the value was newly inserted.
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

    match seconds.second.get_mut(&date_time.second()) {
        Some(set) => {
            if !set.insert(add_cid.into()) {
                return Ok(false);
            }
        }
        None => {
            let set = HashSet::from([add_cid.into()]);
            seconds.second.insert(date_time.second(), set);
        }
    }

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

    Ok(true)
}

/// Removes a value from the index.
/// Returns whether the value was present in the index.
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

#[cfg(test)]
mod tests {
    #![cfg(not(target_arch = "wasm32"))]

    use crate::Defluencer;

    use super::*;

    use chrono::{Duration, TimeZone};

    use futures::StreamExt;

    use ipfs_api::IpfsService;

    use multihash::MultihashGeneric;
    type Multihash = MultihashGeneric<64>;

    use rand_core::RngCore;

    use rand::Rng;

    use rand_xoshiro::{rand_core::SeedableRng, Xoshiro256StarStar};

    fn random_cid(rng: &mut Xoshiro256StarStar) -> Cid {
        let mut hash = [0u8; 32];
        rng.fill_bytes(&mut hash);

        let multihash = Multihash::wrap(0x12, &hash).unwrap();

        Cid::new_v1(0x71, multihash)
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore]
    async fn empty_index_get_remove() {
        let ipfs = IpfsService::default();

        // Pre-generated with ipfs.dag_put(&Yearly::default(), Codec::default()).await;
        let root = Cid::try_from("bafyreibyn3zsznoi4fnkzakdggyb7qw4dny53hg3eyisauu5s7yhodwhx4")
            .unwrap()
            .into();

        let datetime = Utc::now();

        // Random key
        let key =
            Cid::try_from("bafyreiebxcyrgbybcebsk7dwlkidiyi7y6shpvsmneufdouto3pgumvefe").unwrap();

        let result = remove(&ipfs, datetime, &mut Some(root), key).await.unwrap();

        assert!(!result);

        println!("Root {}", root.link);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore]
    async fn index_duplicate_insert() {
        let ipfs = IpfsService::default();

        let mut index = None;

        let date_time = Utc::now();

        let mut rng = Xoshiro256StarStar::seed_from_u64(2347867832489023);

        let add_cid = random_cid(&mut rng);

        let result = insert(&ipfs, date_time, &mut index, add_cid).await.unwrap();

        println!("Root {}", index.unwrap().link);

        assert!(result);

        let result = insert(&ipfs, date_time, &mut index, add_cid).await.unwrap();

        println!("Root {}", index.unwrap().link);

        assert!(!result);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore]
    async fn index_sequential_insert() {
        let ipfs = IpfsService::default();

        let mut rng = Xoshiro256StarStar::seed_from_u64(2347867832489023);

        let mut date_time = Utc.with_ymd_and_hms(2020, 7, 8, 9, 10, 11).unwrap();

        let mut index = None;

        let count = 256;

        for _ in 0..count {
            let key = random_cid(&mut rng);

            date_time -= Duration::hours(1);
            date_time -= Duration::minutes(1);
            date_time -= Duration::seconds(1);

            if let Err(e) = insert(&ipfs, date_time, &mut index, key).await {
                panic!("Index: {} Key: {} Error: {}", index.unwrap().link, key, e);
            }
        }

        let defluencer = Defluencer::from(ipfs);

        let sum = defluencer
            .stream_content_rev_chrono(index.unwrap())
            .fold(0, |acc, _| async move { acc + 1 })
            .await;

        assert_eq!(count, sum);

        println!("Root {}", index.unwrap().link);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore]
    async fn hamt_sequential_remove() {
        let ipfs = IpfsService::default();

        let mut rng = Xoshiro256StarStar::seed_from_u64(2347867832489023);

        // Pre-generated with index_sequential_insert;
        let mut index = Some(
            Cid::try_from("bafyreigxaq7nptdduxzvzn4vrbh7jwipsnu2r4zcn7qqefcfdijhzmfbjm")
                .unwrap()
                .into(),
        );

        let mut date_time = Utc.with_ymd_and_hms(2020, 7, 8, 9, 10, 11).unwrap();

        for _ in 0..256 {
            let key = random_cid(&mut rng);

            date_time -= Duration::hours(1);
            date_time -= Duration::minutes(1);
            date_time -= Duration::seconds(1);

            match remove(&ipfs, date_time, &mut index, key).await {
                Ok(bool) => assert!(bool),
                Err(e) => panic!("Root: {} Key: {} Error: {}", index.unwrap().link, key, e),
            }
        }

        let defluencer = Defluencer::from(ipfs);

        let sum = defluencer
            .stream_content_rev_chrono(index.unwrap())
            .fold(0, |acc, _| async move { acc + 1 })
            .await;

        assert_eq!(0, sum);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore]
    async fn index_fuzzy() {
        let ipfs = IpfsService::default();

        let mut rng = Xoshiro256StarStar::seed_from_u64(2347867832489023);

        let mut date_time = Utc.with_ymd_and_hms(2020, 7, 8, 9, 10, 11).unwrap();

        let mut index = None;

        let count = 500;

        let mut keys = Vec::with_capacity(count);

        for _ in 0..count {
            if keys.is_empty() || rng.gen_ratio(2, 3) {
                let key = random_cid(&mut rng);

                date_time -= Duration::hours(1);
                date_time -= Duration::minutes(1);
                date_time -= Duration::seconds(1);

                keys.push((key, date_time));

                match insert(&ipfs, date_time, &mut index, key).await {
                    Ok(is_new) => assert!(is_new),
                    Err(e) => panic!("Index: {} Key: {} Error: {}", index.unwrap().link, key, e),
                }
            } else {
                let idx = rng.gen_range(0..keys.len());

                let (key, date_time) = keys.swap_remove(idx);

                match remove(&ipfs, date_time, &mut index, key).await {
                    Ok(was_removed) => assert!(was_removed),
                    Err(e) => panic!("Root: {} Key: {} Error: {}", index.unwrap().link, key, e),
                }
            }
        }

        let defluencer = Defluencer::from(ipfs);

        let sum = defluencer
            .stream_content_rev_chrono(index.unwrap())
            .fold(0, |acc, _| async move { acc + 1 })
            .await;

        println!("Final Count {}\nRoot {}", sum, index.unwrap().link);
    }
}
