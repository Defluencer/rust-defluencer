use std::fmt::Debug;

use libipld_core::ipld::Ipld;

/// Trait for tree keys.
///
/// Notable bounds are; ordered and compatible with Ipld.
///
/// As for ```str``` and ```String``` read this std [note](https://doc.rust-lang.org/std/cmp/trait.Ord.html#impl-Ord-for-str)
pub trait Key:
    Debug + Default + Clone + Ord + TryFrom<Ipld> + Into<Ipld> + Send + Sync + 'static
{
}
impl<T: Debug + Default + Clone + Ord + TryFrom<Ipld> + Into<Ipld> + Send + Sync + 'static> Key
    for T
{
}

/// Trait for tree values.
///
/// Only notable bound is compatibility with Ipld.
pub trait Value:
    Debug + Default + Clone + TryFrom<Ipld> + Into<Ipld> + Send + Sync + 'static
{
}
impl<T: Debug + Default + Clone + TryFrom<Ipld> + Into<Ipld> + Send + Sync + 'static> Value for T {}
