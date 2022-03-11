pub mod anchoring_systems;
pub mod content_cache;
pub mod errors;
pub mod moderation_cache;
pub mod signature_system;
pub mod user;
pub mod utils;

use std::borrow::Cow;

use anchoring_systems::IPNSAnchor;
use cid::Cid;

use linked_data::{beacon::Beacon, identity::Identity};

use heck::{ToSnakeCase, ToTitleCase};

use ipfs_api::{errors::Error, responses::KeyPair, IpfsService};
use signature_system::IPNSSignature;
use user::User;

type IPNSUser = User<IPNSAnchor, IPNSSignature>;

pub struct Defluencer {
    ipfs: IpfsService,
}

impl Defluencer {
    pub fn new() -> Self {
        let ipfs = IpfsService::default();

        Self { ipfs }
    }

    /* /// Create a new IPNS user on this IPFS node.
    ///
    /// Names are converted to title case.
    pub async fn create_ipns_user(
        &self,
        display_name: impl Into<Cow<'static, str>>,
    ) -> Result<IPNSUser, Error> {
        let name = display_name.into();
        let key_name = name.to_snake_case();
        let display_name = name.to_title_case();

        let avatar = Cid::default().into(); //TODO provide a default avatar Cid

        let beacon = Beacon {
            identity: Identity {
                display_name,
                avatar,
            },
            content: Default::default(),
            comments: Default::default(),
            live: Default::default(),
            follows: Default::default(),
            bans: Default::default(),
            mods: Default::default(),
        };

        //TODO generate ed25519 key pair then import into IPFS.

        let KeyPair { id: _, name } = self.ipfs.key_import(key_name, key_pair).await?;

        let user = IPNSUser::new(
            self.ipfs.clone(),
            IPNSAnchor::new(self.ipfs.clone(), name.clone()),
            IPNSSignature::new(self.ipfs.clone(), key_pair),
        );

        self.ipfs.ipns_put(name, false, &beacon).await?;

        Ok(user)
    } */

    /* /// Search this IPFS node for users.
    ///
    /// IPNS records that resolve to beacons are considered local users.
    pub async fn get_ipns_users(&self) -> Result<Vec<IPNSUser>, Error> {
        let list = self.ipfs.key_list().await?;

        let (names, keys): (Vec<String>, Vec<Cid>) = list.into_iter().unzip();

        let futs: Vec<_> = keys
            .into_iter()
            .map(|key| self.ipfs.name_resolve(key))
            .collect();

        let results: Vec<Result<Cid, Error>> = future::join_all(futs).await;

        let list: Vec<(String, _)> = results
            .into_iter()
            .zip(names.into_iter())
            .filter_map(|(result, name)| match result {
                Ok(cid) => Some((name, self.ipfs.dag_get::<&str, Beacon>(cid, Option::None))),
                _ => None,
            })
            .collect();

        let (names, futs): (Vec<String>, Vec<_>) = list.into_iter().unzip();

        let results: Vec<Result<Beacon, Error>> = future::join_all(futs).await;

        let users: Vec<IPNSUser> = results
            .into_iter()
            .zip(names.into_iter())
            .filter_map(|(result, name)| match result {
                Ok(_) => Some(IPNSUser::new(
                    self.ipfs.clone(),
                    IPNSAnchor::new(self.ipfs.clone(), name),
                    IPNSSignature::new(self.ipfs.clone(), key_pair),
                )),
                _ => None,
            })
            .collect();

        Ok(users)
    } */
}
