//! An R-Tree: a height-balanced spatial index over axis-aligned bounding boxes,
//! supporting both one-at-a-time insertion and bulk loading.
//!
//! Incremental insertion follows Guttman's original algorithm (R. Guttman, 1984,
//! "R-Trees: A Dynamic Index Structure for Spatial Searching"): `ChooseLeaf` picks
//! the child requiring the least bounding-box enlargement at each level, and a
//! node that overflows `MAX_ENTRIES` is split with the quadratic-cost heuristic
//! (pick the two entries that would waste the most area if grouped together as
//! seeds, then greedily assign the rest to whichever group needs the least
//! enlargement).
//!
//! Bulk loading uses the Sort-Tile-Recursive (STR) algorithm (Leutenegger, Lopez,
//! Edgington, 1997): entries are sorted into vertical slices by x, each slice
//! sorted by y, and chunked into leaves — then the same packing is applied
//! one level up, repeatedly, until a single root remains. STR produces a tree
//! with better query performance than repeated incremental insertion, at the cost
//! of requiring the whole dataset up front.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use tpt_gis_geom::{Point, Rect};

/// Maximum entries (leaf items or child nodes) per node before it splits.
const MAX_ENTRIES: usize = 8;
/// Minimum entries per node after a split (Guttman's `m`), enforced by
/// [`quadratic_split`]'s "assign the rest once the other group would fall below
/// this" rule.
const MIN_ENTRIES: usize = MAX_ENTRIES / 2;

enum Node<T> {
    Leaf(Vec<(Rect, T)>),
    Internal(Vec<(Rect, Box<Node<T>>)>),
}

impl<T> Node<T> {
    /// The bounding box enclosing every entry/child in this node, recomputed from
    /// them directly (nodes don't cache their own bbox — only their *parent* does,
    /// in the `Rect` half of each `(Rect, _)` pair the parent stores).
    fn bbox(&self) -> Rect {
        match self {
            Node::Leaf(entries) => bbox_of(entries),
            Node::Internal(children) => bbox_of(children),
        }
    }
}

/// An R-Tree mapping bounding boxes to values of type `T`.
pub struct RTree<T> {
    root: Node<T>,
}

impl<T> Default for RTree<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> RTree<T> {
    /// Creates an empty R-Tree.
    #[must_use]
    pub fn new() -> Self {
        Self { root: Node::Leaf(Vec::new()) }
    }

    /// Builds an R-Tree from a complete set of `(bounding box, value)` pairs using
    /// Sort-Tile-Recursive bulk loading — substantially faster than the same
    /// number of one-at-a-time [`Self::insert`] calls, and produces a tree with
    /// better-packed nodes (and thus faster queries).
    #[must_use]
    pub fn bulk_load(entries: Vec<(Rect, T)>) -> Self {
        if entries.is_empty() {
            return Self::new();
        }

        let mut level: Vec<(Rect, Node<T>)> = str_pack(entries, MAX_ENTRIES)
            .into_iter()
            .map(|group| (bbox_of(&group), Node::Leaf(group)))
            .collect();

        while level.len() > 1 {
            level = str_pack(level, MAX_ENTRIES)
                .into_iter()
                .map(|group| {
                    let bbox = bbox_of(&group);
                    let children = group.into_iter().map(|(r, n)| (r, Box::new(n))).collect();
                    (bbox, Node::Internal(children))
                })
                .collect();
        }

        let (_, root) = level.into_iter().next().expect("non-empty input produces a root");
        Self { root }
    }

    /// Inserts a `(bounding box, value)` pair, rebalancing the tree (via node
    /// splits, and growing the tree's height if the root itself splits) as needed.
    pub fn insert(&mut self, bbox: Rect, value: T) {
        let old_root = core::mem::replace(&mut self.root, Node::Leaf(Vec::new()));
        let (new_root, split) = insert_into(old_root, bbox, value);
        self.root = match split {
            None => new_root,
            Some((split_bbox, split_node)) => Node::Internal(vec![
                (new_root.bbox(), Box::new(new_root)),
                (split_bbox, Box::new(split_node)),
            ]),
        };
    }

    /// Returns every value whose stored bounding box intersects `region`.
    /// Removes one entry whose bounding box equals `bbox` and whose value equals
    /// `value`. Returns `true` if an entry was removed.
    ///
    /// Underfull nodes are dissolved and their entries reinserted (the standard
    /// "condense tree" step), so the tree stays balanced.
    pub fn remove(&mut self, bbox: &Rect, value: &T) -> bool
    where
        T: PartialEq,
    {
        let mut orphans = Vec::new();
        if !remove_from(&mut self.root, bbox, value, &mut orphans) {
            return false;
        }
        // Shrink the tree while the root is an internal node with a single child.
        loop {
            match &mut self.root {
                Node::Internal(children) if children.len() == 1 => {
                    let (_, child) = children.pop().expect("one child");
                    self.root = *child;
                }
                _ => break,
            }
        }
        for (rect, item) in orphans {
            self.insert(rect, item);
        }
        true
    }

    /// The `k` values whose bounding boxes are nearest to `point`, closest first, each
    /// with its distance. Distance is measured from `point` to the stored bounding box
    /// (zero if the point lies inside it).
    ///
    /// Uses best-first search over the tree, so only the nodes needed for the answer
    /// are visited.
    #[must_use]
    pub fn nearest(&self, point: Point, k: usize) -> Vec<(f64, &T)> {
        self.nearest_iter(point).take(k).collect()
    }

    /// Lazily yields every stored value in order of increasing bounding-box distance
    /// from `point`. Each step only expands the nodes needed for the next value, so
    /// callers that stop early (for example, once a refined distance is known to beat
    /// every remaining box) do not pay for the rest of the tree.
    #[must_use]
    pub fn nearest_iter(&self, point: Point) -> Nearest<'_, T> {
        let mut heap = BinaryHeap::new();
        heap.push(Candidate { dist: 0.0, kind: Kind::Node(&self.root) });
        Nearest { point, heap }
    }

    /// Returns every value whose stored bounding box intersects `region`.
    ///
    /// This is a bounding-box filter, not an exact geometry test — callers with a
    /// precise shape (not just its bbox) should refine these candidates with an
    /// exact predicate (e.g. `tpt_gis_geom::predicates::point_in_ring`), which is
    /// exactly what [`crate::spatial_join`] does.
    #[must_use]
    pub fn query(&self, region: &Rect) -> Vec<&T> {
        let mut results = Vec::new();
        query_node(&self.root, region, &mut results);
        results
    }

    /// Iterates over every value stored in the tree, in an unspecified order.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        let mut results = Vec::new();
        collect_all(&self.root, &mut results);
        results.into_iter()
    }

    /// The number of values stored in the tree.
    #[must_use]
    pub fn len(&self) -> usize {
        self.iter().count()
    }

    /// Returns `true` if the tree holds no values.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Number of direct entries (leaf items or child nodes) in a node.
fn node_len<T>(node: &Node<T>) -> usize {
    match node {
        Node::Leaf(entries) => entries.len(),
        Node::Internal(children) => children.len(),
    }
}

/// Whether `outer` covers `inner` entirely (boundary inclusive).
fn rect_covers(outer: &Rect, inner: &Rect) -> bool {
    inner.min.x >= outer.min.x
        && inner.max.x <= outer.max.x
        && inner.min.y >= outer.min.y
        && inner.max.y <= outer.max.y
}

/// Removes the first leaf entry matching `(bbox, value)` under `node`. Children left
/// underfull are dissolved and their entries pushed onto `orphans` for reinsertion.
fn remove_from<T: PartialEq>(
    node: &mut Node<T>,
    bbox: &Rect,
    value: &T,
    orphans: &mut Vec<(Rect, T)>,
) -> bool {
    match node {
        Node::Leaf(entries) => match entries.iter().position(|(r, v)| r == bbox && v == value) {
            Some(pos) => {
                entries.remove(pos);
                true
            }
            None => false,
        },
        Node::Internal(children) => {
            for i in 0..children.len() {
                if !rect_covers(&children[i].0, bbox) {
                    continue;
                }
                if remove_from(&mut children[i].1, bbox, value, orphans) {
                    if node_len(&children[i].1) < MIN_ENTRIES {
                        let (_, child) = children.remove(i);
                        drain_items(*child, orphans);
                    } else {
                        children[i].0 = children[i].1.bbox();
                    }
                    return true;
                }
            }
            false
        }
    }
}

/// Moves every leaf entry of a subtree into `out`.
fn drain_items<T>(node: Node<T>, out: &mut Vec<(Rect, T)>) {
    match node {
        Node::Leaf(entries) => out.extend(entries),
        Node::Internal(children) => {
            for (_, child) in children {
                drain_items(*child, out);
            }
        }
    }
}

/// Distance from `p` to the nearest point of `rect` (zero when `p` is inside it).
fn min_distance(rect: &Rect, p: Point) -> f64 {
    let dx = (rect.min.x - p.x).max(0.0).max(p.x - rect.max.x);
    let dy = (rect.min.y - p.y).max(0.0).max(p.y - rect.max.y);
    (dx * dx + dy * dy).sqrt()
}

/// A best-first search frontier entry: either a subtree or a stored value, keyed by
/// its distance from the query point. Ordered so `BinaryHeap` pops the nearest first.
struct Candidate<'a, T> {
    dist: f64,
    kind: Kind<'a, T>,
}

enum Kind<'a, T> {
    Node(&'a Node<T>),
    Item(&'a T),
}

/// Lazy nearest-first iterator returned by [`RTree::nearest_iter`].
pub struct Nearest<'a, T> {
    point: Point,
    heap: BinaryHeap<Candidate<'a, T>>,
}

impl<'a, T> Iterator for Nearest<'a, T> {
    type Item = (f64, &'a T);

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(Candidate { dist, kind }) = self.heap.pop() {
            match kind {
                Kind::Item(value) => return Some((dist, value)),
                Kind::Node(Node::Leaf(entries)) => {
                    for (rect, value) in entries {
                        self.heap.push(Candidate {
                            dist: min_distance(rect, self.point),
                            kind: Kind::Item(value),
                        });
                    }
                }
                Kind::Node(Node::Internal(children)) => {
                    for (rect, child) in children {
                        self.heap.push(Candidate {
                            dist: min_distance(rect, self.point),
                            kind: Kind::Node(child),
                        });
                    }
                }
            }
        }
        None
    }
}

impl<T> PartialEq for Candidate<'_, T> {
    fn eq(&self, other: &Self) -> bool {
        self.dist.total_cmp(&other.dist) == Ordering::Equal
    }
}

impl<T> Eq for Candidate<'_, T> {}

impl<T> PartialOrd for Candidate<'_, T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for Candidate<'_, T> {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reversed: the max-heap then yields the smallest distance first.
        other.dist.total_cmp(&self.dist)
    }
}

fn rect_area(r: Rect) -> f64 {
    (r.max.x - r.min.x) * (r.max.y - r.min.y)
}

fn bbox_of<E>(entries: &[(Rect, E)]) -> Rect {
    entries
        .iter()
        .map(|(r, _)| *r)
        .reduce(|a, b| a.union(&b))
        .expect("bbox_of called on a non-empty node")
}

fn query_node<'a, T>(node: &'a Node<T>, region: &Rect, results: &mut Vec<&'a T>) {
    match node {
        Node::Leaf(entries) => {
            for (rect, value) in entries {
                if rect.intersects(region) {
                    results.push(value);
                }
            }
        }
        Node::Internal(children) => {
            for (rect, child) in children {
                if rect.intersects(region) {
                    query_node(child, region, results);
                }
            }
        }
    }
}

fn collect_all<'a, T>(node: &'a Node<T>, results: &mut Vec<&'a T>) {
    match node {
        Node::Leaf(entries) => results.extend(entries.iter().map(|(_, v)| v)),
        Node::Internal(children) => {
            for (_, child) in children {
                collect_all(child, results);
            }
        }
    }
}

/// Recursively inserts `(bbox, value)` into `node`, returning the (possibly
/// restructured) node and, if it overflowed and had to split, its new sibling.
fn insert_into<T>(node: Node<T>, bbox: Rect, value: T) -> (Node<T>, Option<(Rect, Node<T>)>) {
    match node {
        Node::Leaf(mut entries) => {
            entries.push((bbox, value));
            if entries.len() <= MAX_ENTRIES {
                (Node::Leaf(entries), None)
            } else {
                let (a, b) = quadratic_split(entries);
                (Node::Leaf(a), Some((bbox_of(&b), Node::Leaf(b))))
            }
        }
        Node::Internal(mut children) => {
            let idx = choose_subtree(&children, &bbox);
            let (_, child) = children.remove(idx);
            let (updated_child, split) = insert_into(*child, bbox, value);
            children.insert(idx, (updated_child.bbox(), Box::new(updated_child)));

            let Some((split_bbox, split_node)) = split else {
                return (Node::Internal(children), None);
            };
            children.push((split_bbox, Box::new(split_node)));

            if children.len() <= MAX_ENTRIES {
                (Node::Internal(children), None)
            } else {
                let (a, b) = quadratic_split(children);
                (Node::Internal(a), Some((bbox_of(&b), Node::Internal(b))))
            }
        }
    }
}

/// Picks the child whose bounding box needs the least enlargement to include
/// `bbox`, breaking ties by preferring the child with the smaller existing area.
fn choose_subtree<E>(children: &[(Rect, E)], bbox: &Rect) -> usize {
    let mut best_idx = 0;
    let mut best_enlargement = f64::INFINITY;
    let mut best_area = f64::INFINITY;

    for (idx, (rect, _)) in children.iter().enumerate() {
        let enlargement = rect_area(rect.union(bbox)) - rect_area(*rect);
        let area = rect_area(*rect);
        if enlargement < best_enlargement || (enlargement == best_enlargement && area < best_area) {
            best_idx = idx;
            best_enlargement = enlargement;
            best_area = area;
        }
    }
    best_idx
}

/// The two groups produced by [`quadratic_split`].
type Split<E> = (Vec<(Rect, E)>, Vec<(Rect, E)>);

/// Guttman's quadratic-cost split: partitions `entries` (which must number
/// `MAX_ENTRIES + 1`, i.e. one over the limit) into two non-empty groups, each
/// with at least [`MIN_ENTRIES`] members.
fn quadratic_split<E>(mut entries: Vec<(Rect, E)>) -> Split<E> {
    debug_assert_eq!(entries.len(), MAX_ENTRIES + 1);

    // PickSeeds: the pair that would waste the most area if grouped together.
    let mut seed_a = 0;
    let mut seed_b = 1;
    let mut worst_waste = f64::NEG_INFINITY;
    for i in 0..entries.len() {
        for j in (i + 1)..entries.len() {
            let waste = rect_area(entries[i].0.union(&entries[j].0))
                - rect_area(entries[i].0)
                - rect_area(entries[j].0);
            if waste > worst_waste {
                worst_waste = waste;
                seed_a = i;
                seed_b = j;
            }
        }
    }

    let (hi, lo) = if seed_a > seed_b { (seed_a, seed_b) } else { (seed_b, seed_a) };
    let entry_hi = entries.remove(hi);
    let entry_lo = entries.remove(lo);

    let mut bbox_a = entry_lo.0;
    let mut bbox_b = entry_hi.0;
    let mut group_a = vec![entry_lo];
    let mut group_b = vec![entry_hi];

    let mut remaining = entries;
    while !remaining.is_empty() {
        // If all remaining entries must go to one group to satisfy MIN_ENTRIES
        // there, do that and stop (Guttman's QS2).
        if remaining.len() + group_a.len() <= MIN_ENTRIES {
            for entry in remaining.drain(..) {
                bbox_a = bbox_a.union(&entry.0);
                group_a.push(entry);
            }
            break;
        }
        if remaining.len() + group_b.len() <= MIN_ENTRIES {
            for entry in remaining.drain(..) {
                bbox_b = bbox_b.union(&entry.0);
                group_b.push(entry);
            }
            break;
        }

        // PickNext: the entry whose enlargement preference between the two
        // groups is most decisive (largest |d1 - d2|).
        let mut best_idx = 0;
        let mut best_diff = f64::NEG_INFINITY;
        let mut best_d1 = 0.0;
        let mut best_d2 = 0.0;
        for (idx, (rect, _)) in remaining.iter().enumerate() {
            let d1 = rect_area(bbox_a.union(rect)) - rect_area(bbox_a);
            let d2 = rect_area(bbox_b.union(rect)) - rect_area(bbox_b);
            let diff = (d1 - d2).abs();
            if diff > best_diff {
                best_diff = diff;
                best_idx = idx;
                best_d1 = d1;
                best_d2 = d2;
            }
        }

        let entry = remaining.remove(best_idx);
        let goes_to_a = match best_d1.partial_cmp(&best_d2) {
            Some(core::cmp::Ordering::Less) => true,
            Some(core::cmp::Ordering::Greater) => false,
            _ => match rect_area(bbox_a).partial_cmp(&rect_area(bbox_b)) {
                Some(core::cmp::Ordering::Less) => true,
                Some(core::cmp::Ordering::Greater) => false,
                _ => group_a.len() <= group_b.len(),
            },
        };

        if goes_to_a {
            bbox_a = bbox_a.union(&entry.0);
            group_a.push(entry);
        } else {
            bbox_b = bbox_b.union(&entry.0);
            group_b.push(entry);
        }
    }

    (group_a, group_b)
}

fn center(r: Rect) -> Point {
    Point::new((r.min.x + r.max.x) / 2.0, (r.min.y + r.max.y) / 2.0)
}

/// Sort-Tile-Recursive packing: groups `items` into chunks of (at most)
/// `group_size`, ordered so that spatially nearby items end up in the same
/// chunk — used to build both the leaf level and every level above it during
/// [`RTree::bulk_load`].
fn str_pack<E>(mut items: Vec<(Rect, E)>, group_size: usize) -> Vec<Vec<(Rect, E)>> {
    let n = items.len();
    if n == 0 {
        return Vec::new();
    }

    let num_groups = n.div_ceil(group_size);
    let num_slices = (num_groups as f64).sqrt().ceil() as usize;
    let slice_capacity = (num_slices * group_size).max(group_size);

    items.sort_by(|a, b| center(a.0).x.total_cmp(&center(b.0).x));

    let mut start = 0;
    while start < n {
        let end = (start + slice_capacity).min(n);
        items[start..end].sort_by(|a, b| center(a.0).y.total_cmp(&center(b.0).y));
        start = end;
    }

    let mut groups = Vec::with_capacity(num_groups);
    let mut iter = items.into_iter();
    loop {
        let group: Vec<_> = (&mut iter).take(group_size).collect();
        if group.is_empty() {
            break;
        }
        groups.push(group);
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Rect {
        Rect::new(Point::new(min_x, min_y), Point::new(max_x, max_y))
    }

    /// Checks the R-Tree invariant that every parent's cached bounding box truly
    /// encloses all of its children/entries, and that every non-root node meets
    /// the minimum fill factor — recursively, for the whole tree.
    fn assert_invariants<T>(tree: &RTree<T>) {
        fn check<T>(node: &Node<T>, is_root: bool) {
            match node {
                Node::Leaf(entries) => {
                    if !is_root {
                        assert!(entries.len() >= MIN_ENTRIES, "leaf underflow: {}", entries.len());
                    }
                    assert!(entries.len() <= MAX_ENTRIES);
                }
                Node::Internal(children) => {
                    if !is_root {
                        assert!(
                            children.len() >= MIN_ENTRIES,
                            "internal underflow: {}",
                            children.len()
                        );
                    }
                    assert!(children.len() <= MAX_ENTRIES);
                    for (claimed_bbox, child) in children {
                        let actual_bbox = child.bbox();
                        assert!(
                            (claimed_bbox.min.x - actual_bbox.min.x).abs() < 1e-9
                                && (claimed_bbox.min.y - actual_bbox.min.y).abs() < 1e-9
                                && (claimed_bbox.max.x - actual_bbox.max.x).abs() < 1e-9
                                && (claimed_bbox.max.y - actual_bbox.max.y).abs() < 1e-9,
                            "cached bbox doesn't match actual child bbox"
                        );
                        check(child, false);
                    }
                }
            }
        }
        check(&tree.root, true);
    }

    #[test]
    fn empty_tree_has_no_results() {
        let tree: RTree<&str> = RTree::new();
        assert!(tree.query(&rect(0.0, 0.0, 10.0, 10.0)).is_empty());
        assert_eq!(tree.len(), 0);
    }

    #[test]
    fn insert_and_query_basic() {
        let mut tree = RTree::new();
        tree.insert(rect(0.0, 0.0, 1.0, 1.0), "a");
        tree.insert(rect(5.0, 5.0, 6.0, 6.0), "b");
        tree.insert(rect(10.0, 10.0, 11.0, 11.0), "c");

        let hits = tree.query(&rect(4.0, 4.0, 7.0, 7.0));
        assert_eq!(hits, vec![&"b"]);

        let hits_all = tree.query(&rect(-1.0, -1.0, 20.0, 20.0));
        assert_eq!(hits_all.len(), 3);
    }

    #[test]
    fn insert_causing_splits_maintains_invariants() {
        let mut tree = RTree::new();
        for i in 0..500 {
            let x = f64::from(i);
            tree.insert(rect(x, x, x + 0.5, x + 0.5), i);
        }
        assert_eq!(tree.len(), 500);
        assert_invariants(&tree);

        // Every inserted item should be individually findable via a query on its
        // own bbox.
        for i in [0, 1, 250, 499] {
            let x = f64::from(i);
            let hits = tree.query(&rect(x, x, x + 0.5, x + 0.5));
            assert!(hits.contains(&&i), "missing item {i}");
        }
    }

    #[test]
    fn bulk_load_finds_all_items_and_maintains_invariants() {
        let entries: Vec<(Rect, usize)> = (0..1000)
            .map(|i| {
                let x = (i % 50) as f64 * 10.0;
                let y = (i / 50) as f64 * 10.0;
                (rect(x, y, x + 1.0, y + 1.0), i)
            })
            .collect();

        let tree = RTree::bulk_load(entries);
        assert_eq!(tree.len(), 1000);
        assert_invariants(&tree);

        let hits = tree.query(&rect(-1000.0, -1000.0, 10_000.0, 10_000.0));
        assert_eq!(hits.len(), 1000);

        // Spot-check a specific item is retrievable at its known location.
        let hits = tree.query(&rect(0.0, 0.0, 1.0, 1.0));
        assert!(hits.contains(&&0usize));
    }

    /// Non-finite bounding boxes must not panic bulk loading (they used to hit an
    ///  in the sort). Every item is kept, and finite items stay queryable.
    #[test]
    fn bulk_load_tolerates_nan_bboxes() {
        let mut entries: Vec<(Rect, usize)> =
            (0..50).map(|i| (rect(i as f64, 0.0, i as f64 + 1.0, 1.0), i)).collect();
        entries.push((rect(f64::NAN, 0.0, f64::NAN, 1.0), 999));
        let tree = RTree::bulk_load(entries);
        assert_eq!(tree.len(), 51);
        let hits = tree.query(&rect(-10.0, -10.0, 100.0, 100.0));
        assert!(hits.contains(&&0usize));
        assert!(hits.contains(&&49usize));
    }

    #[test]
    fn bulk_load_empty_input_yields_empty_tree() {
        let tree: RTree<()> = RTree::bulk_load(Vec::new());
        assert!(tree.is_empty());
    }

    #[test]
    fn remove_deletes_matching_entry_and_keeps_others() {
        let mut tree = RTree::new();
        for i in 0..200usize {
            let x = (i % 20) as f64 * 3.0;
            let y = (i / 20) as f64 * 3.0;
            tree.insert(rect(x, y, x + 1.0, y + 1.0), i);
        }
        // Remove every third entry.
        for i in (0..200usize).step_by(3) {
            let x = (i % 20) as f64 * 3.0;
            let y = (i / 20) as f64 * 3.0;
            assert!(tree.remove(&rect(x, y, x + 1.0, y + 1.0), &i), "entry {i} not found");
        }
        assert_invariants(&tree);
        let removed = (0..200usize).step_by(3).count();
        assert_eq!(tree.len(), 200 - removed);
        let hits = tree.query(&rect(-1.0, -1.0, 1000.0, 1000.0));
        assert_eq!(hits.len(), 200 - removed);
        assert!(hits.iter().all(|v| **v % 3 != 0));
    }

    #[test]
    fn remove_returns_false_for_missing_entries() {
        let mut tree = RTree::new();
        tree.insert(rect(0.0, 0.0, 1.0, 1.0), 1usize);
        assert!(!tree.remove(&rect(0.0, 0.0, 1.0, 1.0), &2));
        assert!(!tree.remove(&rect(5.0, 5.0, 6.0, 6.0), &1));
        assert_eq!(tree.len(), 1);
    }

    /// `nearest` must agree with a brute-force scan over the stored boxes.
    #[test]
    fn nearest_matches_brute_force() {
        let entries: Vec<(Rect, usize)> = (0..300usize)
            .map(|i| {
                let x = ((i * 37) % 101) as f64;
                let y = ((i * 53) % 97) as f64;
                (rect(x, y, x + 0.5, y + 0.5), i)
            })
            .collect();
        let tree = RTree::bulk_load(entries.clone());
        let p = Point::new(40.0, 40.0);
        let got: Vec<(f64, usize)> = tree.nearest(p, 7).into_iter().map(|(d, v)| (d, *v)).collect();

        let mut expected: Vec<f64> = entries.iter().map(|(r, _)| min_distance(r, p)).collect();
        expected.sort_by(f64::total_cmp);
        assert_eq!(got.len(), 7);
        for (i, (d, _)) in got.iter().enumerate() {
            assert!((d - expected[i]).abs() < 1e-12, "rank {i}: {d} vs {}", expected[i]);
        }
        // Results come back closest first.
        assert!(got.windows(2).all(|w| w[0].0 <= w[1].0));
    }

    #[test]
    fn nearest_handles_k_larger_than_the_tree() {
        let mut tree = RTree::new();
        tree.insert(rect(0.0, 0.0, 1.0, 1.0), "a");
        assert_eq!(tree.nearest(Point::new(5.0, 5.0), 10).len(), 1);
        assert!(tree.nearest(Point::new(5.0, 5.0), 0).is_empty());
    }

    #[test]
    fn disjoint_regions_do_not_match() {
        let mut tree = RTree::new();
        tree.insert(rect(0.0, 0.0, 1.0, 1.0), 1);
        let hits = tree.query(&rect(100.0, 100.0, 101.0, 101.0));
        assert!(hits.is_empty());
    }
}
