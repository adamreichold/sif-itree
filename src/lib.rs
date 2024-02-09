#![forbid(unsafe_code)]
#![deny(missing_docs, missing_debug_implementations)]

//! A simple library implementing an immutable, flat representation of an [augmented interval tree](https://en.wikipedia.org/wiki/Interval_tree#Augmented_tree)
//!
//! Supports querying for overlapping intervals without temporary allocations and uses a flat memory layout that can be backed by memory maps.

use std::marker::PhantomData;
use std::ops::{ControlFlow, Deref, Range};

/// The items stored in the tree consisting of an interval and an associated value
pub type Item<K, V> = (Range<K>, V);

/// The nodes of which the tree is built consisting of an item and the maximum of the interval upper bounds in the subtree
pub type Node<K, V> = (Item<K, V>, K);

/// Interval tree mapping half-open intervals with boundaries of type `K` to values of type `V`
#[derive(Debug, Default, Clone)]
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

    /// Query for all intervals overlapping the given interval
    pub fn query<'a, H>(&'a self, interval: Range<K>, handler: H) -> ControlFlow<()>
    where
        K: Ord,
        H: FnMut(&'a Item<K, V>) -> ControlFlow<()>,
    {
        let nodes = self.nodes.as_ref();

        if !nodes.is_empty() {
            query(&mut QueryArgs { interval, handler }, nodes)?;
        }

        ControlFlow::Continue(())
    }
}

struct QueryArgs<K, H> {
    interval: Range<K>,
    handler: H,
}

fn query<'a, K, V, H>(args: &mut QueryArgs<K, H>, mut nodes: &'a [Node<K, V>]) -> ControlFlow<()>
where
    K: Ord,
    H: FnMut(&'a (Range<K>, V)) -> ControlFlow<()>,
{
    loop {
        let (left, rest) = nodes.split_at(nodes.len() / 2);
        let (mid, right) = rest.split_first().unwrap();

        let mut go_left = false;
        let mut go_right = false;

        if args.interval.start < mid.1 {
            if !left.is_empty() {
                go_left = true;
            }

            if args.interval.end > (mid.0).0.start {
                if !right.is_empty() {
                    go_right = true;
                }

                if args.interval.start < (mid.0).0.end {
                    (args.handler)(&mid.0)?;
                }
            }
        }

        match (go_left, go_right) {
            (true, true) => {
                query(args, left)?;

                nodes = right;
            }
            (true, false) => nodes = left,
            (false, true) => nodes = right,
            (false, false) => return ControlFlow::Continue(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use proptest::{collection::vec, test_runner::TestRunner};

    #[test]
    fn random() {
        const DOM: Range<i32> = -1000..1000;
        const LEN: usize = 1000_usize;

        TestRunner::default()
            .run(
                &(vec(DOM, LEN), vec(DOM, LEN), DOM, DOM),
                |(start, end, query_start, query_end)| {
                    let tree = start
                        .iter()
                        .zip(&end)
                        .map(|(&start, &end)| (start..end, ()))
                        .collect::<ITree<_, _>>();

                    let mut result1 = Vec::new();
                    tree.query(query_start..query_end, |(range, ())| {
                        result1.push(range);
                        ControlFlow::Continue(())
                    });

                    let mut result2 = tree
                        .iter()
                        .filter(|(range, ())| query_end > range.start && query_start < range.end)
                        .map(|(range, ())| range)
                        .collect::<Vec<_>>();

                    result1.sort_unstable_by_key(|range| (range.start, range.end));
                    result2.sort_unstable_by_key(|range| (range.start, range.end));
                    assert_eq!(result1, result2);

                    Ok(())
                },
            )
            .unwrap()
    }
}
