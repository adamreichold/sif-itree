#![forbid(unsafe_code)]
#![deny(missing_docs, missing_debug_implementations)]

//! A simple library implementing an immutable, flat representation of an [augmented interval tree](https://en.wikipedia.org/wiki/Interval_tree#Augmented_tree)
//!
//! Supports querying for overlapping intervals without temporary allocations and uses a flat memory layout that can be backed by memory maps.

mod query;

use std::marker::PhantomData;
use std::ops::{Deref, Range};

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

/// The items stored in the tree consisting of an interval and an associated value
pub type Item<K, V> = (Range<K>, V);

/// The nodes of which the tree is built consisting of an item and the maximum of the interval upper bounds in the subtree
pub type Node<K, V> = (Item<K, V>, K);

/// Interval tree mapping half-open intervals with boundaries of type `K` to values of type `V`
#[derive(Debug, Default, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct ITree<K, V, S = Box<[Node<K, V>]>> {
    nodes: S,
    _marker: PhantomData<(K, V)>,
}

impl<K, V, S> Deref for ITree<K, V, S>
where
    S: AsRef<[Node<K, V>]>,
{
    type Target = [Node<K, V>];

    fn deref(&self) -> &Self::Target {
        self.nodes.as_ref()
    }
}

impl<K, V, S> AsRef<[Node<K, V>]> for ITree<K, V, S>
where
    S: AsRef<[Node<K, V>]>,
{
    fn as_ref(&self) -> &[Node<K, V>] {
        self.nodes.as_ref()
    }
}

impl<K, V, S> FromIterator<Item<K, V>> for ITree<K, V, S>
where
    K: Ord + Clone,
    S: AsMut<[Node<K, V>]> + FromIterator<Node<K, V>>,
{
    fn from_iter<I>(iter: I) -> Self
    where
        I: IntoIterator<Item = Item<K, V>>,
    {
        let mut nodes = iter
            .into_iter()
            .map(|(interval, value)| {
                let end = interval.end.clone();
                ((interval, value), end)
            })
            .collect::<S>();

        {
            let nodes = nodes.as_mut();

            nodes.sort_unstable_by(|lhs, rhs| (lhs.0).0.start.cmp(&(rhs.0).0.start));

            if !nodes.is_empty() {
                update_max(nodes);
            }
        }

        Self {
            nodes,
            _marker: PhantomData,
        }
    }
}

fn update_max<K, V>(nodes: &mut [Node<K, V>]) -> K
where
    K: Ord + Clone,
{
    let (left, rest) = nodes.split_at_mut(nodes.len() / 2);
    let (mid, right) = rest.split_first_mut().unwrap();

    if !left.is_empty() {
        mid.1 = mid.1.clone().max(update_max(left));
    }

    if !right.is_empty() {
        mid.1 = mid.1.clone().max(update_max(right));
    }

    mid.1.clone()
}

impl<K, V, S> ITree<K, V, S>
where
    S: AsRef<[Node<K, V>]>,
{
    /// Interprets the given `nodes` as a tree
    ///
    /// Supplying `nodes` which are not actually organized as an interval tree is safe but will lead to incorrect results.
    pub fn new_unchecked(nodes: S) -> Self {
        Self {
            nodes,
            _marker: PhantomData,
        }
    }

    /// Iterate over all intervals
    pub fn iter(&self) -> impl ExactSizeIterator<Item = &Item<K, V>> {
        self.nodes.as_ref().iter().map(|node| &node.0)
    }
}

fn split<N>(nodes: &[N]) -> (&[N], &N, &[N]) {
    let (left, rest) = nodes.split_at(nodes.len() / 2);
    let (mid, right) = rest.split_first().unwrap();

    (left, mid, right)
}
