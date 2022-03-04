//use std::collections::HashSet;

//use crate::IPLDLink;

use cid::Cid;

use serde::{Deserialize, Serialize};

use serde_with::{serde_as, DisplayFromStr};

//use either::Either;

/// List of who you follow.
#[serde_as]
#[derive(Serialize, Deserialize, Debug, Default, PartialEq, Eq, Clone)]
pub struct Follows {
    pub ens: Vec<String>,

    #[serde_as(as = "Vec<DisplayFromStr>")]
    pub ipns: Vec<Cid>,
}

/* #[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash)]
pub struct Friend {
    /// Domain name on the Ethereum Name Service or
    /// Link to friend's beacon.
    #[serde(with = "either::serde_untagged")]
    pub friend: Either<String, IPLDLink>,
} */

/* #[cfg(test)]
mod tests {
    use super::*;
    use cid::Cid;

    #[test]
    fn serde_test() {
        let mut old_friends = Friendlies {
            friends: HashSet::with_capacity(2),
        };

        old_friends.friends.insert(Friend {
            friend: Either::Left("friend1".to_owned()),
        });

        old_friends.friends.insert(Friend {
            friend: Either::Right(Cid::default().into()),
        });

        let json = serde_json::to_string_pretty(&old_friends).expect("Serializing");
        println!("{}", json);

        let new_friends = serde_json::from_str(&json).expect("Deserializing");
        println!("{:?}", new_friends);

        assert_eq!(old_friends, new_friends);
    }
} */
