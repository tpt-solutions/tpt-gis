//! Polygon boolean operations: union, intersection, difference, and symmetric difference.
//!
//! This implements the sweep-line method of Martinez, Rueda and Feito (2009), "A new
//! algorithm for computing Boolean operations on polygons". The steps are:
//!
//! 1. **Subdivision.** Every edge is split wherever it meets another edge, so that
//!    edges only touch at shared endpoints. Coincident (overlapping) edges are split
//!    at each other's endpoints, so they either coincide exactly or do not overlap.
//! 2. **Sweep.** Edge endpoints are processed in lexicographic order (x, then y). The
//!    sweep keeps the set of edges crossing the sweep position. For each edge, the
//!    parity of same-polygon edges below it tells whether the regions on either side
//!    lie inside each input polygon. The edge is part of the result exactly when the
//!    boolean operation differs between the two sides. Its direction is chosen so the
//!    result lies on its left.
//! 3. **Ring assembly.** The retained directed edges form closed rings. Counter-
//!    clockwise rings are exterior boundaries, clockwise rings are holes, and each
//!    hole is attached to the smallest exterior that contains it.
//!
//! Inputs are assumed to be valid polygons (simple rings, holes inside the exterior).
//! Touching and coincident boundaries are handled. The output has no collinear
//! redundant vertices.
//!
//! Complexity is O(m²) in the number of edges `m`: the subdivision compares every pair
//! of edges, and the sweep scans the active set at each event.

use alloc::vec;
use alloc::vec::Vec;
use core::cmp::Ordering;

use libm::atan2;

use crate::float_ext::Fabs as _;
use crate::point::Point;
use crate::polygon::Polygon;
use crate::predicates::point_in_ring;

/// A boolean operation on two polygons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipOp {
    /// Points in either polygon.
    Union,
    /// Points in both polygons.
    Intersection,
    /// Points in the first polygon but not the second.
    Difference,
    /// Points in exactly one of the two polygons.
    Xor,
}

impl ClipOp {
    fn apply(self, in_a: bool, in_b: bool) -> bool {
        match self {
            ClipOp::Union => in_a || in_b,
            ClipOp::Intersection => in_a && in_b,
            ClipOp::Difference => in_a && !in_b,
            ClipOp::Xor => in_a != in_b,
        }
    }
}

/// A polygon produced by a boolean operation: a counter-clockwise exterior ring followed
/// by clockwise hole rings. Rings are open (the first point is not repeated).
#[derive(Debug, Clone, PartialEq)]
pub struct OwnedPolygon {
    /// Exterior ring first, then holes.
    pub rings: Vec<Vec<Point>>,
}

impl OwnedPolygon {
    /// The planar area of the polygon (exterior minus holes).
    #[must_use]
    pub fn area(&self) -> f64 {
        let mut total = 0.0;
        for (i, ring) in self.rings.iter().enumerate() {
            let a = ring_area(ring).fabs();
            if i == 0 {
                total += a;
            } else {
                total -= a;
            }
        }
        total
    }
}

/// Computes `a op b` and returns the result as a set of polygons. An empty vector means
/// the result has no area.
#[must_use]
pub fn clip(a: &Polygon, b: &Polygon, op: ClipOp) -> Vec<OwnedPolygon> {
    let scale = coordinate_scale(a, b);
    let segs = split_edges(&collect_edges(a, b), scale);
    let edges = sweep(&segs, op);
    assemble(edges, 1e-10 * scale)
}

/// Union of two polygons.
#[must_use]
pub fn union(a: &Polygon, b: &Polygon) -> Vec<OwnedPolygon> {
    clip(a, b, ClipOp::Union)
}

/// Intersection of two polygons.
#[must_use]
pub fn intersection(a: &Polygon, b: &Polygon) -> Vec<OwnedPolygon> {
    clip(a, b, ClipOp::Intersection)
}

/// The part of `a` not covered by `b`.
#[must_use]
pub fn difference(a: &Polygon, b: &Polygon) -> Vec<OwnedPolygon> {
    clip(a, b, ClipOp::Difference)
}

/// Symmetric difference of two polygons.
#[must_use]
pub fn symmetric_difference(a: &Polygon, b: &Polygon) -> Vec<OwnedPolygon> {
    clip(a, b, ClipOp::Xor)
}

// ----------------------------------------------------------------------------------
// Geometry helpers
// ----------------------------------------------------------------------------------

/// A directed-free edge with `a` lexicographically before `b`, tagged with the polygon it
/// came from (0 = first operand, 1 = second).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Seg {
    a: Point,
    b: Point,
    poly: u8,
}

fn lex_cmp(p: Point, q: Point) -> Ordering {
    p.x.total_cmp(&q.x).then(p.y.total_cmp(&q.y))
}

fn sub(p: Point, q: Point) -> Point {
    Point::new(p.x - q.x, p.y - q.y)
}

fn dot(u: Point, v: Point) -> f64 {
    u.x * v.x + u.y * v.y
}

fn cross(u: Point, v: Point) -> f64 {
    u.x * v.y - u.y * v.x
}

/// Positive when `c` lies to the left of the directed line `a → b`.
fn orient(a: Point, b: Point, c: Point) -> f64 {
    cross(sub(b, a), sub(c, a))
}

fn dist(p: Point, q: Point) -> f64 {
    let d = sub(p, q);
    libm::sqrt(d.x * d.x + d.y * d.y)
}

/// Signed shoelace area: positive for counter-clockwise rings.
fn ring_area(ring: &[Point]) -> f64 {
    let n = ring.len();
    if n < 3 {
        return 0.0;
    }
    let mut sum = 0.0;
    for i in 0..n {
        let p = ring[i];
        let q = ring[(i + 1) % n];
        sum += p.x * q.y - q.x * p.y;
    }
    sum / 2.0
}

/// A scale used to set tolerances: the largest coordinate magnitude, at least 1.
fn coordinate_scale(a: &Polygon, b: &Polygon) -> f64 {
    let mut m: f64 = 1.0;
    for p in a.exterior.iter().chain(b.exterior.iter()) {
        m = m.max(p.x.fabs()).max(p.y.fabs());
    }
    for ring in a.interiors.iter().chain(b.interiors.iter()) {
        for p in ring.iter() {
            m = m.max(p.x.fabs()).max(p.y.fabs());
        }
    }
    m
}

// ----------------------------------------------------------------------------------
// Step 1: subdivision
// ----------------------------------------------------------------------------------

fn collect_edges(a: &Polygon, b: &Polygon) -> Vec<Seg> {
    let mut segs = Vec::new();
    for (poly, p) in [(0u8, a), (1u8, b)] {
        for ring in core::iter::once(p.exterior).chain(p.interiors.iter().copied()) {
            let n = ring.len();
            for i in 0..n {
                let (p0, p1) = (ring[i], ring[(i + 1) % n]);
                if p0 == p1 {
                    continue;
                }
                let (lo, hi) = if lex_cmp(p0, p1) == Ordering::Less { (p0, p1) } else { (p1, p0) };
                segs.push(Seg { a: lo, b: hi, poly });
            }
        }
    }
    segs
}

/// Points where segments `s` and `t` meet, pushed onto `out`. Endpoints of one segment
/// that lie on the other are included, so overlapping collinear edges are split at each
/// other's ends.
fn meeting_points(s: Seg, t: Seg, eps: f64, out: &mut Vec<Point>) {
    let d1 = sub(s.b, s.a);
    let d2 = sub(t.b, t.a);
    let denom = cross(d1, d2);
    let endpoints = [s.a, s.b, t.a, t.b];

    if denom.fabs() > 0.0 {
        let w = sub(t.a, s.a);
        let u = cross(w, d2) / denom;
        let v = cross(w, d1) / denom;
        if (0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&v) {
            let x = Point::new(s.a.x + u * d1.x, s.a.y + u * d1.y);
            // Snap to a nearby endpoint so that touching edges share the exact point.
            let snapped = endpoints
                .iter()
                .copied()
                .filter(|e| dist(*e, x) <= eps)
                .min_by(|p, q| dist(*p, x).total_cmp(&dist(*q, x)));
            out.push(snapped.unwrap_or(x));
        }
        return;
    }

    // Parallel: collinear only if t.a lies on the line through s.
    let len = dist(s.a, s.b).max(f64::MIN_POSITIVE);
    if orient(s.a, s.b, t.a).fabs() / len > eps {
        return;
    }
    for e in [t.a, t.b] {
        if on_segment(e, s, eps) {
            out.push(e);
        }
    }
    for e in [s.a, s.b] {
        if on_segment(e, t, eps) {
            out.push(e);
        }
    }
}

fn on_segment(p: Point, s: Seg, eps: f64) -> bool {
    let len = dist(s.a, s.b).max(f64::MIN_POSITIVE);
    orient(s.a, s.b, p).fabs() / len <= eps
        && p.x >= s.a.x.min(s.b.x) - eps
        && p.x <= s.a.x.max(s.b.x) + eps
        && p.y >= s.a.y.min(s.b.y) - eps
        && p.y <= s.a.y.max(s.b.y) + eps
}

/// A set of vertices in which points within `eps` of each other are identified. All
/// coordinates are mapped to a single representative, so that edges which meet
/// approximately (for example, a computed crossing near an existing corner) share one
/// exact point after subdivision.
struct Canon {
    /// Representatives, sorted lexicographically.
    pts: Vec<Point>,
    eps: f64,
}

impl Canon {
    fn new(eps: f64) -> Self {
        Self { pts: Vec::new(), eps }
    }

    fn get(&mut self, p: Point) -> Point {
        let lo = self.pts.partition_point(|q| q.x < p.x - self.eps);
        for &q in &self.pts[lo..] {
            if q.x > p.x + self.eps {
                break;
            }
            if dist(q, p) <= self.eps {
                return q;
            }
        }
        let at = self.pts.partition_point(|q| lex_cmp(*q, p) == Ordering::Less);
        self.pts.insert(at, p);
        p
    }
}

/// Splits every edge at all points where it meets another edge, and returns the pieces.
/// Pieces are lexicographically ordered (`a < b`) and keep their polygon tag.
fn split_edges(segs: &[Seg], scale: f64) -> Vec<Seg> {
    let eps = 1e-10 * scale;
    let mut canon = Canon::new(eps);

    // Canonicalise the input endpoints first, so that the intersection tests below work
    // on shared vertices. Edges that collapse to a point are dropped.
    let mut edges: Vec<Seg> = Vec::with_capacity(segs.len());
    for s in segs {
        let a = canon.get(s.a);
        let b = canon.get(s.b);
        if a == b {
            continue;
        }
        let (lo, hi) = if lex_cmp(a, b) == Ordering::Less { (a, b) } else { (b, a) };
        edges.push(Seg { a: lo, b: hi, poly: s.poly });
    }

    let mut cuts: Vec<Vec<Point>> = vec![Vec::new(); edges.len()];
    let mut scratch = Vec::new();
    for i in 0..edges.len() {
        for j in (i + 1)..edges.len() {
            scratch.clear();
            meeting_points(edges[i], edges[j], eps, &mut scratch);
            for &raw in &scratch {
                let p = canon.get(raw);
                cuts[i].push(p);
                cuts[j].push(p);
            }
        }
    }

    let mut out = Vec::new();
    for (i, s) in edges.iter().enumerate() {
        let mut pts = core::mem::take(&mut cuts[i]);
        pts.push(s.a);
        pts.push(s.b);
        // Order the points by their position along the edge, not lexicographically. For a
        // nearly vertical edge the x values of its points can round in either direction, so
        // lexicographic order can put a cut before the edge's own endpoint.
        let dir = sub(s.b, s.a);
        let along = |p: &Point| dot(sub(*p, s.a), dir);
        pts.sort_by(|p, q| along(p).total_cmp(&along(q)));
        pts.dedup();
        for w in pts.windows(2) {
            let (lo, hi) =
                if lex_cmp(w[0], w[1]) == Ordering::Less { (w[0], w[1]) } else { (w[1], w[0]) };
            out.push(Seg { a: lo, b: hi, poly: s.poly });
        }
    }
    out
}

// ----------------------------------------------------------------------------------
// Step 2: sweep
// ----------------------------------------------------------------------------------

/// Directed result edges, with the result on their left.
type ResultEdge = (Point, Point);

/// Identifies groups of pieces with identical endpoints (coincident edges).
struct Groups {
    of: Vec<usize>,
    count: Vec<[usize; 2]>,
}

fn group_pieces(segs: &[Seg]) -> Groups {
    let mut order: Vec<usize> = (0..segs.len()).collect();
    order.sort_by(|&i, &j| lex_cmp(segs[i].a, segs[j].a).then(lex_cmp(segs[i].b, segs[j].b)));
    let mut of = vec![0usize; segs.len()];
    let mut count: Vec<[usize; 2]> = Vec::new();
    let mut prev: Option<(Point, Point)> = None;
    for &i in &order {
        let key = (segs[i].a, segs[i].b);
        if prev != Some(key) {
            count.push([0, 0]);
            prev = Some(key);
        }
        let g = count.len() - 1;
        of[i] = g;
        count[g][usize::from(segs[i].poly)] += 1;
    }
    Groups { of, count }
}

fn sweep(segs: &[Seg], op: ClipOp) -> Vec<ResultEdge> {
    let groups = group_pieces(segs);
    let mut emitted = vec![false; groups.count.len()];

    // Events: (point, kind, piece). Right endpoints (kind 0) sort before left endpoints
    // (kind 1) at the same point.
    let mut events: Vec<(Point, u8, usize)> = Vec::with_capacity(segs.len() * 2);
    for (i, s) in segs.iter().enumerate() {
        events.push((s.a, 1, i));
        events.push((s.b, 0, i));
    }
    events.sort_by(|x, y| lex_cmp(x.0, y.0).then(x.1.cmp(&y.1)));

    let mut active: Vec<usize> = Vec::new();
    let mut result = Vec::new();
    let mut k = 0;
    while k < events.len() {
        // Process every event at one point as a batch. Retiring the pieces that end
        // here and inserting all pieces that start here must both happen before any of
        // them is classified, so that the classification sees every piece at this point.
        let p = events[k].0;
        let mut end = k;
        while end < events.len() && events[end].0 == p {
            end += 1;
        }
        let mut starting = Vec::new();
        for &(_, kind, i) in &events[k..end] {
            if kind == 0 {
                if let Some(pos) = active.iter().position(|&t| t == i) {
                    active.swap_remove(pos);
                }
            } else {
                active.push(i);
                starting.push(i);
            }
        }

        for &i in &starting {
            let g = groups.of[i];
            if emitted[g] {
                continue;
            }
            emitted[g] = true;
            let s = segs[i];
            let dir_s = sub(s.b, p);
            // Parity of same-polygon pieces strictly below this group's line.
            let mut below = [0usize; 2];
            for &t in &active {
                if groups.of[t] == g {
                    continue;
                }
                let st = segs[t];
                let is_below = if st.a == p {
                    // Also starting here: below if this piece is counter-clockwise from it.
                    cross(sub(st.b, st.a), dir_s) > 0.0
                } else {
                    // Otherwise it spans p: below if p lies above it.
                    orient(st.a, st.b, p) > 0.0
                };
                if is_below {
                    below[usize::from(st.poly)] += 1;
                }
            }
            let count = groups.count[g];
            let in_below = [below[0] % 2 == 1, below[1] % 2 == 1];
            // Crossing the group's own pieces toggles membership.
            let in_above = [in_below[0] ^ (count[0] % 2 == 1), in_below[1] ^ (count[1] % 2 == 1)];
            let res_above = op.apply(in_above[0], in_above[1]);
            let res_below = op.apply(in_below[0], in_below[1]);
            if res_above != res_below {
                if res_above {
                    result.push((s.a, s.b));
                } else {
                    result.push((s.b, s.a));
                }
            }
        }
        k = end;
    }
    result
}

// ----------------------------------------------------------------------------------
// Step 3: ring assembly
// ----------------------------------------------------------------------------------

fn assemble(mut edges: Vec<ResultEdge>, eps: f64) -> Vec<OwnedPolygon> {
    edges.sort_by(|x, y| lex_cmp(x.0, y.0));
    let mut used = vec![false; edges.len()];

    // All rings, closed and oriented by the interior-on-left convention.
    let mut rings: Vec<Vec<Point>> = Vec::new();
    for start in 0..edges.len() {
        if used[start] {
            continue;
        }
        let mut ring = Vec::new();
        let mut e = start;
        let mut closed = false;
        loop {
            used[e] = true;
            let (from, to) = edges[e];
            ring.push(from);
            if to == ring[0] {
                closed = true;
                break;
            }
            let incoming = sub(to, from);
            // Among unused edges leaving `to`, take the most counter-clockwise turn.
            let mut best: Option<(usize, f64)> = None;
            let first = edges.partition_point(|x| lex_cmp(x.0, to) == Ordering::Less);
            for k in first..edges.len() {
                if edges[k].0 != to {
                    break;
                }
                if used[k] {
                    continue;
                }
                let out = sub(edges[k].1, to);
                let turn = atan2(cross(incoming, out), incoming.x * out.x + incoming.y * out.y);
                if best.map_or(true, |(_, t)| turn > t) {
                    best = Some((k, turn));
                }
            }
            match best {
                Some((k, _)) => e = k,
                // An open chain cannot close into a ring; drop it.
                None => break,
            }
        }
        if closed && ring.len() >= 3 {
            rings.push(remove_collinear(ring));
        }
    }

    let mut exteriors: Vec<Vec<Point>> = Vec::new();
    let mut holes: Vec<Vec<Point>> = Vec::new();
    for ring in rings {
        let area = ring_area(&ring);
        if area > 0.0 {
            exteriors.push(ring);
        } else if area < 0.0 {
            holes.push(ring);
        }
    }

    // Attach each hole to the smallest exterior containing it.
    let mut polygons: Vec<OwnedPolygon> =
        exteriors.into_iter().map(|ext| OwnedPolygon { rings: vec![ext] }).collect();
    for hole in holes {
        let exterior_rings: Vec<&[Point]> =
            polygons.iter().map(|p| p.rings[0].as_slice()).collect();
        let Some(probe) = interior_probe(&hole, &exterior_rings, eps) else {
            continue;
        };
        let mut target: Option<(usize, f64)> = None;
        for (idx, poly) in polygons.iter().enumerate() {
            if point_in_ring(probe, &poly.rings[0]) {
                let area = ring_area(&poly.rings[0]).fabs();
                if target.map_or(true, |(_, a)| area < a) {
                    target = Some((idx, area));
                }
            }
        }
        if let Some((idx, _)) = target {
            polygons[idx].rings.push(hole);
        }
    }
    polygons
}

/// A point of `hole` (a vertex or edge midpoint) that lies clear of every exterior edge,
/// so that point-in-ring tests against the exteriors are unambiguous. A hole may touch
/// its exterior at a vertex, so the first vertex is not always usable.
fn interior_probe(hole: &[Point], exteriors: &[&[Point]], eps: f64) -> Option<Point> {
    let n = hole.len();
    let clear = |p: Point| {
        exteriors.iter().all(|ring| {
            let m = ring.len();
            (0..m).all(|i| {
                let (a, b) = (ring[i], ring[(i + 1) % m]);
                let (lo, hi) = if lex_cmp(a, b) == Ordering::Less { (a, b) } else { (b, a) };
                !on_segment(p, Seg { a: lo, b: hi, poly: 0 }, eps)
            })
        })
    };
    for &v in hole {
        if clear(v) {
            return Some(v);
        }
    }
    for i in 0..n {
        let (a, b) = (hole[i], hole[(i + 1) % n]);
        let mid = Point::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
        if clear(mid) {
            return Some(mid);
        }
    }
    None
}

/// Removes vertices that lie exactly on the straight line between their neighbours.
fn remove_collinear(ring: Vec<Point>) -> Vec<Point> {
    let n = ring.len();
    if n < 4 {
        return ring;
    }
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let prev = ring[(i + n - 1) % n];
        let cur = ring[i];
        let next = ring[(i + 1) % n];
        if orient(prev, cur, next) != 0.0 {
            out.push(cur);
        }
    }
    if out.len() < 3 {
        ring
    } else {
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    fn rect(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point> {
        vec![
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
            Point::new(x0, y0),
        ]
    }

    fn total_area(result: &[OwnedPolygon]) -> f64 {
        result.iter().map(OwnedPolygon::area).sum()
    }

    #[test]
    fn overlapping_squares_all_operations() {
        let a_ring = rect(0.0, 0.0, 10.0, 10.0);
        let b_ring = rect(5.0, 5.0, 15.0, 15.0);
        let a = Polygon::from_exterior(&a_ring);
        let b = Polygon::from_exterior(&b_ring);
        assert!((total_area(&union(&a, &b)) - 175.0).fabs() < 1e-9);
        assert!((total_area(&intersection(&a, &b)) - 25.0).fabs() < 1e-9);
        assert!((total_area(&difference(&a, &b)) - 75.0).fabs() < 1e-9);
        assert!((total_area(&symmetric_difference(&a, &b)) - 150.0).fabs() < 1e-9);
    }

    #[test]
    fn disjoint_squares() {
        let a_ring = rect(0.0, 0.0, 1.0, 1.0);
        let b_ring = rect(5.0, 5.0, 6.0, 6.0);
        let a = Polygon::from_exterior(&a_ring);
        let b = Polygon::from_exterior(&b_ring);
        let u = union(&a, &b);
        assert_eq!(u.len(), 2);
        assert!((total_area(&u) - 2.0).fabs() < 1e-9);
        assert!(intersection(&a, &b).is_empty());
        assert!((total_area(&difference(&a, &b)) - 1.0).fabs() < 1e-9);
    }

    #[test]
    fn identical_squares() {
        let ring = rect(0.0, 0.0, 10.0, 10.0);
        let a = Polygon::from_exterior(&ring);
        assert!((total_area(&union(&a, &a)) - 100.0).fabs() < 1e-9);
        assert!((total_area(&intersection(&a, &a)) - 100.0).fabs() < 1e-9);
        assert!(difference(&a, &a).is_empty());
        assert!(symmetric_difference(&a, &a).is_empty());
    }

    /// Squares sharing an edge: their union is one rectangle, and the intersection has
    /// no area.
    #[test]
    fn squares_touching_along_an_edge() {
        let a_ring = rect(0.0, 0.0, 10.0, 10.0);
        let b_ring = rect(10.0, 0.0, 20.0, 10.0);
        let a = Polygon::from_exterior(&a_ring);
        let b = Polygon::from_exterior(&b_ring);
        let u = union(&a, &b);
        assert_eq!(u.len(), 1, "{u:?}");
        assert!((total_area(&u) - 200.0).fabs() < 1e-9);
        assert!(intersection(&a, &b).is_empty());
    }

    #[test]
    fn difference_creates_a_hole() {
        let outer_ring = rect(0.0, 0.0, 10.0, 10.0);
        let inner_ring = rect(4.0, 4.0, 6.0, 6.0);
        let a = Polygon::from_exterior(&outer_ring);
        let b = Polygon::from_exterior(&inner_ring);
        let d = difference(&a, &b);
        assert_eq!(d.len(), 1);
        assert_eq!(d[0].rings.len(), 2, "expected one hole");
        assert!((total_area(&d) - 96.0).fabs() < 1e-9);
    }

    #[test]
    fn union_fills_a_hole_covered_by_the_other_polygon() {
        let outer_ring = rect(0.0, 0.0, 10.0, 10.0);
        let hole_ring = rect(4.0, 4.0, 6.0, 6.0);
        let cover_ring = rect(3.0, 3.0, 7.0, 7.0);
        let holes: [&[Point]; 1] = [&hole_ring];
        let a = Polygon::new(&outer_ring, &holes);
        let b = Polygon::from_exterior(&cover_ring);
        let u = union(&a, &b);
        assert!((total_area(&u) - 100.0).fabs() < 1e-9);
        assert!(u.iter().all(|p| p.rings.len() == 1));
        // Intersection is the cover minus the hole part it overlaps: 16 - 4 = 12.
        assert!((total_area(&intersection(&a, &b)) - 12.0).fabs() < 1e-9);
    }

    #[test]
    fn collinear_output_vertices_are_removed() {
        let a_ring = rect(0.0, 0.0, 10.0, 10.0);
        let b_ring = rect(0.0, 0.0, 5.0, 10.0);
        let a = Polygon::from_exterior(&a_ring);
        let b = Polygon::from_exterior(&b_ring);
        // The shared edge at x=0 and y=0..10 is split where it meets b's corners, but
        // the union is a plain rectangle: exactly four corners remain.
        let u = union(&a, &b);
        assert_eq!(u.len(), 1);
        assert_eq!(u[0].rings[0].len(), 4, "{:?}", u[0].rings[0]);
    }

    /// Deterministic pseudo-random integer rectangles (no external RNG needed).
    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self, max: u64) -> u64 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (self.0 >> 33) % max
        }
    }

    fn random_rect(rng: &mut Lcg) -> Vec<Point> {
        let x0 = rng.next(8) as f64;
        let y0 = rng.next(8) as f64;
        let w = 1 + rng.next(6);
        let h = 1 + rng.next(6);
        rect(x0, y0, x0 + w as f64, y0 + h as f64)
    }

    fn contains(result: &[OwnedPolygon], p: Point) -> bool {
        result.iter().any(|poly| {
            point_in_ring(p, &poly.rings[0]) && !poly.rings[1..].iter().any(|h| point_in_ring(p, h))
        })
    }

    /// Oracle test: at sample points that avoid every edge, membership in the result must
    /// equal the boolean combination of membership in the inputs. Integer rectangles
    /// generate many coincident and touching edges, which is where the sweep is most
    /// fragile.
    #[test]
    fn matches_point_membership_oracle_on_random_rectangles() {
        let mut rng = Lcg(42);
        for trial in 0..3000 {
            let ra = random_rect(&mut rng);
            let rb = random_rect(&mut rng);
            let a = Polygon::from_exterior(&ra);
            let b = Polygon::from_exterior(&rb);
            for op in [ClipOp::Union, ClipOp::Intersection, ClipOp::Difference, ClipOp::Xor] {
                let result = clip(&a, &b, op);
                for i in 0..20 {
                    for j in 0..20 {
                        // Offset avoids integer grid lines, so no sample lies on an edge.
                        let p = Point::new(i as f64 * 0.5 + 0.25, j as f64 * 0.5 + 0.25);
                        let in_a = point_in_ring(p, &ra);
                        let in_b = point_in_ring(p, &rb);
                        assert_eq!(
                            contains(&result, p),
                            op.apply(in_a, in_b),
                            "trial {trial} op {op:?} point {p:?} a {ra:?} b {rb:?}"
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod oracle_tests {
    use super::*;
    use alloc::vec;

    struct Lcg(u64);
    impl Lcg {
        fn next(&mut self, max: u64) -> u64 {
            self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            (self.0 >> 33) % max
        }
    }

    fn triangle(rng: &mut Lcg) -> Vec<Point> {
        let mut pts = Vec::new();
        for _ in 0..3 {
            pts.push(Point::new(rng.next(10) as f64, rng.next(10) as f64));
        }
        pts.push(pts[0]);
        pts
    }

    fn rect(rng: &mut Lcg) -> Vec<Point> {
        let x0 = rng.next(8) as f64;
        let y0 = rng.next(8) as f64;
        let w = 1 + rng.next(6);
        let h = 1 + rng.next(6);
        vec![
            Point::new(x0, y0),
            Point::new(x0 + w as f64, y0),
            Point::new(x0 + w as f64, y0 + h as f64),
            Point::new(x0, y0 + h as f64),
            Point::new(x0, y0),
        ]
    }

    fn area_of(ring: &[Point]) -> f64 {
        ring_area(ring)
    }

    fn contains(result: &[OwnedPolygon], p: Point) -> bool {
        result.iter().any(|poly| {
            point_in_ring(p, &poly.rings[0]) && !poly.rings[1..].iter().any(|h| point_in_ring(p, h))
        })
    }

    /// Checks `clip` against point membership on an off-grid sample lattice.
    fn check(a_rings: &[Vec<Point>], b_rings: &[Vec<Point>], ctx: &str) {
        let a_holes: Vec<&[Point]> = a_rings[1..].iter().map(Vec::as_slice).collect();
        let b_holes: Vec<&[Point]> = b_rings[1..].iter().map(Vec::as_slice).collect();
        let a = Polygon::new(&a_rings[0], &a_holes);
        let b = Polygon::new(&b_rings[0], &b_holes);
        let in_poly = |rings: &[Vec<Point>], p: Point| {
            point_in_ring(p, &rings[0]) && !rings[1..].iter().any(|h| point_in_ring(p, h))
        };
        for op in [ClipOp::Union, ClipOp::Intersection, ClipOp::Difference, ClipOp::Xor] {
            let result = clip(&a, &b, op);
            for i in 0..24 {
                for j in 0..24 {
                    let p = Point::new(i as f64 * 0.43 + 0.2371, j as f64 * 0.43 + 0.1293);
                    let expected = op.apply(in_poly(a_rings, p), in_poly(b_rings, p));
                    assert_eq!(contains(&result, p), expected, "{ctx} op {op:?} p {p:?}");
                }
            }
            // Every output ring must be simple enough to carry a sign: exteriors CCW.
            for poly in &result {
                assert!(area_of(&poly.rings[0]) > 0.0, "{ctx}: exterior not CCW");
                for h in &poly.rings[1..] {
                    assert!(area_of(h) < 0.0, "{ctx}: hole not CW");
                }
            }
        }
    }

    #[test]
    fn oracle_random_triangles() {
        let mut rng = Lcg(7);
        for trial in 0..3000 {
            let ta = triangle(&mut rng);
            let tb = triangle(&mut rng);
            if area_of(&ta).fabs() < 1e-12 || area_of(&tb).fabs() < 1e-12 {
                continue;
            }
            check(
                core::slice::from_ref(&ta),
                core::slice::from_ref(&tb),
                &alloc::format!("triangles trial {trial} {ta:?} {tb:?}"),
            );
        }
    }

    #[test]
    fn oracle_random_rectangles_with_holes() {
        let mut rng = Lcg(99);
        for trial in 0..3000 {
            let outer = rect(&mut rng);
            let (ox0, oy0) = (outer[0].x, outer[0].y);
            let (ox1, oy1) = (outer[2].x, outer[2].y);
            // A hole strictly inside the outer rectangle, when there is room for one.
            let mut a_rings = vec![outer.clone()];
            if ox1 - ox0 >= 3.0 && oy1 - oy0 >= 3.0 {
                let hx = ox0 + 1.0;
                let hy = oy0 + 1.0;
                a_rings.push(vec![
                    Point::new(hx, hy),
                    Point::new(hx + 1.0, hy),
                    Point::new(hx + 1.0, hy + 1.0),
                    Point::new(hx, hy + 1.0),
                    Point::new(hx, hy),
                ]);
            }
            let b_rings = vec![rect(&mut rng)];
            check(
                &a_rings,
                &b_rings,
                &alloc::format!("holes trial {trial} {a_rings:?} {b_rings:?}"),
            );
        }
    }

    /// Random star-shaped polygons with curved (many-vertex) boundaries and irrational-ish
    /// coordinates. This is the regime where nearly coincident vertices appeared.
    fn star(rng: &mut Lcg) -> Vec<Point> {
        let cx = rng.next(1000) as f64 / 100.0;
        let cy = rng.next(1000) as f64 / 100.0;
        let n = 5 + rng.next(8) as usize;
        let mut ring = Vec::with_capacity(n + 1);
        for k in 0..n {
            let theta = core::f64::consts::TAU * k as f64 / n as f64 + rng.next(100) as f64 * 1e-3;
            let r = 1.0 + rng.next(400) as f64 / 100.0;
            ring.push(Point::new(cx + r * libm::cos(theta), cy + r * libm::sin(theta)));
        }
        ring.push(ring[0]);
        ring
    }

    #[test]
    fn oracle_random_star_polygons() {
        let mut rng = Lcg(2024);
        for trial in 0..400 {
            let sa = star(&mut rng);
            let sb = star(&mut rng);
            check(
                core::slice::from_ref(&sa),
                core::slice::from_ref(&sb),
                &alloc::format!("stars trial {trial}"),
            );
        }
    }

    /// Regression: a disk whose vertices sit on a rectangle's corners. The sliver between
    /// nearly identical vertices used to break the ring and drop the disk's outer half.
    #[test]
    fn disk_touching_rectangle_corners_unions_fully() {
        let rect = rect_ring(0.0, -1.0, 10.0, 1.0);
        let disk: Vec<Point> = (0..8)
            .map(|k| {
                let t = core::f64::consts::TAU * k as f64 / 8.0;
                Point::new(libm::cos(t), libm::sin(t))
            })
            .collect();
        let a = Polygon::from_exterior(&rect);
        let mut closed = disk.clone();
        closed.push(disk[0]);
        let b = Polygon::from_exterior(&closed);
        let u = union(&a, &b);
        let area: f64 = u.iter().map(OwnedPolygon::area).sum();
        // The rectangle has area 20. The octagon (area 2√2) is split by the rectangle's left
        // edge, and the half at x < 0 adds √2 to the union.
        assert!((area - (20.0 + libm::sqrt(2.0))).fabs() < 1e-9, "area {area}");
    }

    fn rect_ring(x0: f64, y0: f64, x1: f64, y1: f64) -> Vec<Point> {
        alloc::vec![
            Point::new(x0, y0),
            Point::new(x1, y0),
            Point::new(x1, y1),
            Point::new(x0, y1),
            Point::new(x0, y0),
        ]
    }
}
