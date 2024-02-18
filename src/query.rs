use std::ops::{ControlFlow, Range};

#[cfg(feature = "rayon")]
use rayon::join;

use crate::{split, ITree, Item, Node};

impl<K, V, S> ITree<K, V, S>
where
    S: AsRef<[Node<K, V>]>,
{
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

    #[cfg(feature = "rayon")]
    /// Query for all intervals overlapping the given interval, in parallel
    pub fn par_query<'a, H>(&'a self, interval: Range<K>, handler: H) -> ControlFlow<()>
    where
        K: Ord + Send + Sync,
        V: Sync,
        H: Fn(&'a Item<K, V>) -> ControlFlow<()> + Sync,
    {
        let nodes = self.nodes.as_ref();

        if !nodes.is_empty() {
            par_query(&QueryArgs { interval, handler }, nodes)?;
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
        let (left, mid, right) = split(nodes);

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

#[cfg(feature = "rayon")]
fn par_query<'a, K, V, H>(args: &QueryArgs<K, H>, mut nodes: &'a [Node<K, V>]) -> ControlFlow<()>
where
    K: Ord + Send + Sync,
    V: Sync,
    H: Fn(&'a (Range<K>, V)) -> ControlFlow<()> + Sync,
{
    loop {
        let (left, mid, right) = split(nodes);

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
                let (left, right) = join(|| par_query(args, left), || par_query(args, right));

                left?;
                right?;

                return ControlFlow::Continue(());
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

    #[cfg(feature = "rayon")]
    use std::sync::Mutex;

    use proptest::{collection::vec, test_runner::TestRunner};

    #[test]
    fn query_random() {
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

    #[cfg(feature = "rayon")]
    #[test]
    fn par_query_random() {
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

                    let result1 = Mutex::new(Vec::new());
                    tree.par_query(query_start..query_end, |(range, ())| {
                        result1.lock().unwrap().push(range);
                        ControlFlow::Continue(())
                    });
                    let mut result1 = result1.into_inner().unwrap();

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
