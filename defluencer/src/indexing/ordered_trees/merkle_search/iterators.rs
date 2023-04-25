use std::ops::Bound;

use cid::Cid;
use either::Either;

use crate::indexing::ordered_trees::traits::{Key, Value};

use super::tree::TreeNode;

impl<K: Key, V: Value> IntoIterator for TreeNode<K, V> {
    type Item = Either<(Cid, (Bound<K>, Bound<K>)), (K, V)>;

    type IntoIter = NodeIterator<K, V>;

    fn into_iter(self) -> Self::IntoIter {
        NodeIterator {
            node: self,
            k_v_idx: 0,
            link_idx: 0,
        }
    }
}

pub struct NodeIterator<K: Key, V: Value> {
    pub node: TreeNode<K, V>,
    pub k_v_idx: usize,
    pub link_idx: usize,
}

impl<'a, K: Key, V: Value> Iterator for NodeIterator<K, V> {
    type Item = Either<(Cid, (Bound<K>, Bound<K>)), (K, V)>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.k_v_idx == self.node.key_values.len() {
            return None;
        }

        if let Some((insert_idx, link)) = self.node.index_links.get(self.link_idx) {
            if self.k_v_idx == *insert_idx {
                let l_bound = {
                    if self.k_v_idx == 0 {
                        Bound::Unbounded
                    } else {
                        match self
                            .node
                            .key_values
                            .get(self.k_v_idx - 1)
                            .map(|(key, _)| key.clone())
                        {
                            Some(key) => Bound::Excluded(key),
                            None => Bound::Unbounded,
                        }
                    }
                };

                let h_bound = match self
                    .node
                    .key_values
                    .get(self.k_v_idx)
                    .map(|(key, _)| key.clone())
                {
                    Some(key) => Bound::Excluded(key),
                    None => Bound::Unbounded,
                };

                let range = (l_bound, h_bound);

                self.link_idx += 1;
                return Some(Either::Left((*link, range)));
            }
        }

        let (key, value) = self.node.key_values[self.k_v_idx].clone();

        self.k_v_idx += 1;
        Some(Either::Right((key, value)))
    }
}
