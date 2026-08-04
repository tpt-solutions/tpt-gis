//! A region quadtree: a spatial index over a fixed bounding region, recursively
//! subdivided into four quadrants on demand.
//!
//! Unlike [`crate::rtree::RTree`] (which infers its own bounds from whatever is
//! inserted), a quadtree needs its universe of interest up front (see
//! [`Quadtree::new`]) — subdivision is by *position within that region*, not by
//! the data's own extent.
//!
//! An item is only pushed down into a child quadrant if its bounding box fits
//! *entirely* within one; an item straddling a quadrant boundary stays at the
//! node where the split happens, rather than being duplicated into multiple
//! quadrants or forcing an ever-finer split that can never separate it from its
//! neighbors.

use tpt_gis_geom::{Point, Rect};

/// Maximum items held directly at a node before it subdivides into 4 quadrants.
const MAX_ITEMS_PER_NODE: usize = 8;
/// Maximum subdivision depth, to bound recursion for coincident or
/// near-coincident bounding boxes that would otherwise keep splitting forever.
const MAX_DEPTH: usize = 20;

struct QuadNode<T> {
    bounds: Rect,
    items: Vec<(Rect, T)>,
    /// `Some` once this node has subdivided; always exactly 4 elements, in
    /// `[nw, ne, sw, se]` order (see [`quadrant_bounds`]).
    children: Option<Vec<QuadNode<T>>>,
}

impl<T> QuadNode<T> {
    fn new(bounds: Rect) -> Self {
        Self { bounds, items: Vec::new(), children: None }
    }

    fn insert(&mut self, bbox: Rect, value: T, depth: usize) {
        if self.children.is_none() && self.items.len() >= MAX_ITEMS_PER_NODE && depth < MAX_DEPTH {
            self.subdivide();
        }

        if let Some(children) = &mut self.children {
            if let Some(quadrant) = quadrant_containing(&self.bounds, &bbox) {
                children[quadrant].insert(bbox, value, depth + 1);
                return;
            }
        }
        self.items.push((bbox, value));
    }

    fn subdivide(&mut self) {
        let children =
            quadrant_bounds(&self.bounds).into_iter().map(QuadNode::new).collect::<Vec<_>>();
        self.children = Some(children);

        // Push down any existing items that now fit entirely within one quadrant;
        // the rest stay here.
        let existing = core::mem::take(&mut self.items);
        for (bbox, value) in existing {
            self.insert(bbox, value, 0);
        }
    }

    fn query<'a>(&'a self, region: &Rect, results: &mut Vec<&'a T>) {
        for (rect, value) in &self.items {
            if rect.intersects(region) {
                results.push(value);
            }
        }
        if let Some(children) = &self.children {
            for child in children {
                if child.bounds.intersects(region) {
                    child.query(region, results);
                }
            }
        }
    }

    fn collect_all<'a>(&'a self, results: &mut Vec<&'a T>) {
        results.extend(self.items.iter().map(|(_, v)| v));
        if let Some(children) = &self.children {
            for child in children {
                child.collect_all(results);
            }
        }
    }
}

/// Splits `bounds` into its four quadrants, in `[nw, ne, sw, se]` order.
fn quadrant_bounds(bounds: &Rect) -> [Rect; 4] {
    let mid_x = (bounds.min.x + bounds.max.x) / 2.0;
    let mid_y = (bounds.min.y + bounds.max.y) / 2.0;
    [
        Rect::new(Point::new(bounds.min.x, mid_y), Point::new(mid_x, bounds.max.y)), // NW
        Rect::new(Point::new(mid_x, mid_y), Point::new(bounds.max.x, bounds.max.y)), // NE
        Rect::new(Point::new(bounds.min.x, bounds.min.y), Point::new(mid_x, mid_y)), // SW
        Rect::new(Point::new(mid_x, bounds.min.y), Point::new(bounds.max.x, mid_y)), // SE
    ]
}

/// The index of the quadrant of `bounds` that fully contains `bbox`, or `None`
/// if `bbox` straddles more than one quadrant.
fn quadrant_containing(bounds: &Rect, bbox: &Rect) -> Option<usize> {
    quadrant_bounds(bounds).iter().position(|q| rect_contains_rect(q, bbox))
}

fn rect_contains_rect(outer: &Rect, inner: &Rect) -> bool {
    inner.min.x >= outer.min.x
        && inner.max.x <= outer.max.x
        && inner.min.y >= outer.min.y
        && inner.max.y <= outer.max.y
}

/// A quadtree mapping bounding boxes, within a fixed universe region, to values
/// of type `T`.
pub struct Quadtree<T> {
    root: QuadNode<T>,
}

impl<T> Quadtree<T> {
    /// Creates an empty quadtree covering `bounds`. Items inserted with a
    /// bounding box outside `bounds` are still accepted (they're simply kept at
    /// the root, since they can never fit within any subdivided quadrant) but
    /// won't benefit from spatial partitioning — pick `bounds` to cover the data
    /// you expect to insert.
    #[must_use]
    pub fn new(bounds: Rect) -> Self {
        Self { root: QuadNode::new(bounds) }
    }

    /// Inserts a `(bounding box, value)` pair.
    pub fn insert(&mut self, bbox: Rect, value: T) {
        self.root.insert(bbox, value, 0);
    }

    /// Returns every value whose stored bounding box intersects `region`.
    ///
    /// Like [`crate::rtree::RTree::query`], this is a bounding-box filter —
    /// refine with an exact geometric predicate if you need precise containment.
    #[must_use]
    pub fn query(&self, region: &Rect) -> Vec<&T> {
        let mut results = Vec::new();
        self.root.query(region, &mut results);
        results
    }

    /// Iterates over every value stored in the tree, in an unspecified order.
    pub fn iter(&self) -> impl Iterator<Item = &T> {
        let mut results = Vec::new();
        self.root.collect_all(&mut results);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn rect(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Rect {
        Rect::new(Point::new(min_x, min_y), Point::new(max_x, max_y))
    }

    #[test]
    fn empty_quadtree_has_no_results() {
        let tree: Quadtree<&str> = Quadtree::new(rect(0.0, 0.0, 100.0, 100.0));
        assert!(tree.query(&rect(0.0, 0.0, 10.0, 10.0)).is_empty());
        assert_eq!(tree.len(), 0);
    }

    #[test]
    fn insert_and_query_basic() {
        let mut tree = Quadtree::new(rect(0.0, 0.0, 100.0, 100.0));
        tree.insert(rect(1.0, 1.0, 2.0, 2.0), "nw-ish");
        tree.insert(rect(60.0, 60.0, 61.0, 61.0), "ne-ish");
        tree.insert(rect(10.0, 60.0, 11.0, 61.0), "nw-ish-2");

        let hits = tree.query(&rect(0.0, 50.0, 50.0, 100.0));
        assert_eq!(hits, vec![&"nw-ish-2"]);
    }

    #[test]
    fn subdivision_separates_distant_points() {
        let mut tree = Quadtree::new(rect(0.0, 0.0, 100.0, 100.0));
        // More than MAX_ITEMS_PER_NODE points, spread across all 4 quadrants, so
        // this must trigger subdivision.
        for i in 0..40 {
            let (x, y) = match i % 4 {
                0 => (10.0, 90.0), // NW
                1 => (90.0, 90.0), // NE
                2 => (10.0, 10.0), // SW
                _ => (90.0, 10.0), // SE
            };
            tree.insert(rect(x, y, x + 0.1, y + 0.1), i);
        }
        assert!(tree.root.children.is_some(), "expected the root to have subdivided");
        assert_eq!(tree.len(), 40);

        // Querying just the SW quadrant should only return SW-quadrant items.
        let sw_hits = tree.query(&rect(0.0, 0.0, 50.0, 50.0));
        assert_eq!(sw_hits.len(), 10);
        for &&i in &sw_hits {
            assert_eq!(i % 4, 2);
        }
    }

    #[test]
    fn item_straddling_quadrant_boundary_is_still_found() {
        let mut tree = Quadtree::new(rect(0.0, 0.0, 100.0, 100.0));
        // Force a subdivision first.
        for i in 0..40 {
            let x = f64::from(i % 4) * 20.0 + 1.0;
            tree.insert(rect(x, x, x + 0.1, x + 0.1), i);
        }
        // A box spanning the midpoint doesn't fit purely in any one quadrant.
        let straddling = rect(45.0, 45.0, 55.0, 55.0);
        tree.insert(straddling, 9999);

        let hits = tree.query(&rect(40.0, 40.0, 60.0, 60.0));
        assert!(hits.contains(&&9999), "straddling item should still be queryable");
    }

    #[test]
    fn disjoint_region_does_not_match() {
        let mut tree = Quadtree::new(rect(0.0, 0.0, 100.0, 100.0));
        tree.insert(rect(1.0, 1.0, 2.0, 2.0), "a");
        assert!(tree.query(&rect(90.0, 90.0, 95.0, 95.0)).is_empty());
    }
}
