//! The GeoPackage R-tree spatial index (the `gpkg_rtree_index` extension).
//!
//! An index is SQLite's `rtree` virtual table plus three shadow tables, and six triggers
//! that keep it in step with the feature table. This module builds the shadow-table rows
//! and the trigger text. The tree is packed with Sort-Tile-Recursive and stored in the
//! layout SQLite's R-tree module reads: a `_node` row per node (a blob of a 2-byte depth,
//! a 2-byte count, and 32-byte cells of `id` plus four big-endian `f32` bounds), a
//! `_parent` row per non-root node, and a `_rowid` row per entry. Node 1 is the root.

use std::collections::VecDeque;

use crate::geometry::{Geometry, Point};

/// Entries per node: SQLite's `RTREE_MAXCELLS`. Each cell is 8 + 16 bytes, so a node is a
/// 4-byte header and 51 cells, 1228 bytes. SQLite stores every node at exactly that size.
const MAX_CELLS: usize = 51;

/// The stored size of every node blob.
const NODE_BYTES: usize = 4 + 24 * MAX_CELLS;

/// One R-tree entry: an id (a feature rowid in a leaf, or a child's index while building)
/// and its bounds as `[minx, maxx, miny, maxy]`, rounded outward to `f32`.
type Entry = (i64, [f32; 4]);

/// The three shadow tables' rows, as `(rowid, values)`.
pub(crate) struct RTreeRows {
    /// `_node` rows: `(nodeno, blob)`.
    pub nodes: Vec<(i64, Vec<u8>)>,
    /// `_parent` rows: `(nodeno, parentnode)`.
    pub parents: Vec<(i64, i64)>,
    /// `_rowid` rows: `(feature rowid, leaf nodeno)`.
    pub rowids: Vec<(i64, i64)>,
}

/// The smallest `f32` not above `v`, so a stored bound never excludes a point it should not.
fn round_down(v: f64) -> f32 {
    let f = v as f32;
    if f64::from(f) <= v {
        f
    } else {
        next_down(f)
    }
}

/// The largest `f32` not below `v`.
fn round_up(v: f64) -> f32 {
    let f = v as f32;
    if f64::from(f) >= v {
        f
    } else {
        next_up(f)
    }
}

fn next_down(f: f32) -> f32 {
    if f == 0.0 {
        -f32::from_bits(1)
    } else if f > 0.0 {
        f32::from_bits(f.to_bits() - 1)
    } else {
        f32::from_bits(f.to_bits() + 1)
    }
}

fn next_up(f: f32) -> f32 {
    -next_down(-f)
}

/// The `[minx, maxx, miny, maxy]` bounds of a geometry, rounded outward to `f32`, or `None`
/// for an empty geometry.
pub(crate) fn geometry_bounds(geometry: &Geometry) -> Option<[f32; 4]> {
    let mut acc: Option<[f64; 4]> = None;
    extend_with_geometry(geometry, &mut acc);
    acc.map(|b| [round_down(b[0]), round_up(b[1]), round_down(b[2]), round_up(b[3])])
}

fn extend_with_geometry(geometry: &Geometry, acc: &mut Option<[f64; 4]>) {
    match geometry {
        Geometry::Point(p) => extend_points(std::slice::from_ref(p), acc),
        Geometry::LineString(points) | Geometry::MultiPoint(points) => extend_points(points, acc),
        Geometry::Polygon(poly) => {
            extend_points(poly.exterior(), acc);
            for ring in poly.interiors() {
                extend_points(ring, acc);
            }
        }
        Geometry::MultiLineString(lines) => {
            for line in lines {
                extend_points(line, acc);
            }
        }
        Geometry::MultiPolygon(polys) => {
            for poly in polys {
                extend_with_geometry(&Geometry::Polygon(poly.clone()), acc);
            }
        }
        Geometry::GeometryCollection(items) => {
            for item in items {
                extend_with_geometry(item, acc);
            }
        }
    }
}

fn extend_points(points: &[Point], acc: &mut Option<[f64; 4]>) {
    for p in points {
        let b = acc.get_or_insert([p.x, p.x, p.y, p.y]);
        b[0] = b[0].min(p.x);
        b[1] = b[1].max(p.x);
        b[2] = b[2].min(p.y);
        b[3] = b[3].max(p.y);
    }
}

fn center(b: &[f32; 4]) -> (f64, f64) {
    ((f64::from(b[0]) + f64::from(b[1])) / 2.0, (f64::from(b[2]) + f64::from(b[3])) / 2.0)
}

fn union(group: &[Entry]) -> [f32; 4] {
    let mut out = [f32::INFINITY, f32::NEG_INFINITY, f32::INFINITY, f32::NEG_INFINITY];
    for (_, b) in group {
        out[0] = out[0].min(b[0]);
        out[1] = out[1].max(b[1]);
        out[2] = out[2].min(b[2]);
        out[3] = out[3].max(b[3]);
    }
    out
}

/// Sort-Tile-Recursive: splits `items` into groups of at most [`MAX_CELLS`], sorted into
/// vertical slices by x and then by y within each slice.
fn str_groups(mut items: Vec<Entry>) -> Vec<Vec<Entry>> {
    if items.len() <= MAX_CELLS {
        return vec![items];
    }
    let n = items.len();
    let leaves = n.div_ceil(MAX_CELLS);
    let slices = (leaves as f64).sqrt().ceil() as usize;
    let per_slice = n.div_ceil(slices);
    items.sort_by(|a, b| center(&a.1).0.total_cmp(&center(&b.1).0));
    let mut groups = Vec::new();
    let mut rest = items;
    while !rest.is_empty() {
        let take = per_slice.min(rest.len());
        let mut slice: Vec<Entry> = rest.drain(..take).collect();
        slice.sort_by(|a, b| center(&a.1).1.total_cmp(&center(&b.1).1));
        while !slice.is_empty() {
            let take = MAX_CELLS.min(slice.len());
            groups.push(slice.drain(..take).collect());
        }
    }
    groups
}

/// A node while the tree is being built: its level (0 for leaves) and its cells.
struct BuildNode {
    level: u16,
    cells: Vec<Entry>,
}

/// Packs `entries` (feature rowid and outward-rounded bounds) into an R-tree and returns
/// the shadow-table rows. An empty input gives a single empty root.
pub(crate) fn build(entries: &[(i64, [f32; 4])]) -> RTreeRows {
    if entries.is_empty() {
        return RTreeRows {
            nodes: vec![(1, vec![0; NODE_BYTES])],
            parents: Vec::new(),
            rowids: Vec::new(),
        };
    }
    let mut arena: Vec<BuildNode> = Vec::new();
    let mut current: Vec<Entry> = entries.to_vec();
    let mut level: u16 = 0;
    let root;
    loop {
        let groups = str_groups(current);
        let mut next: Vec<Entry> = Vec::with_capacity(groups.len());
        for group in groups {
            let bounds = union(&group);
            let index = arena.len();
            arena.push(BuildNode { level, cells: group });
            next.push((index as i64, bounds));
        }
        if next.len() <= 1 {
            // One node at this level: it is the root (or the empty root when there are no
            // entries at all).
            root = arena.len() - 1;
            break;
        }
        current = next;
        level += 1;
    }

    // Number the nodes breadth first from the root, so that the root is node 1.
    let mut number = vec![0i64; arena.len()];
    let mut queue = VecDeque::new();
    number[root] = 1;
    queue.push_back(root);
    let mut next_number = 2i64;
    while let Some(index) = queue.pop_front() {
        if arena[index].level == 0 {
            continue;
        }
        for &(child, _) in &arena[index].cells {
            let child = child as usize;
            number[child] = next_number;
            next_number += 1;
            queue.push_back(child);
        }
    }

    let mut rows = RTreeRows { nodes: Vec::new(), parents: Vec::new(), rowids: Vec::new() };
    for (index, node) in arena.iter().enumerate() {
        let nodeno = number[index];
        let mut blob = Vec::with_capacity(NODE_BYTES);
        blob.extend_from_slice(&node.level.to_be_bytes());
        blob.extend_from_slice(&(node.cells.len() as u16).to_be_bytes());
        for &(id, bounds) in &node.cells {
            let id = if node.level == 0 {
                rows.rowids.push((id, nodeno));
                id
            } else {
                let child = id as usize;
                rows.parents.push((number[child], nodeno));
                number[child]
            };
            blob.extend_from_slice(&id.to_be_bytes());
            for v in bounds {
                blob.extend_from_slice(&v.to_be_bytes());
            }
        }
        // SQLite reads each node as a fixed-size blob, so unused cell space is zero-filled.
        blob.resize(NODE_BYTES, 0);
        rows.nodes.push((nodeno, blob));
    }
    rows.nodes.sort_by_key(|(n, _)| *n);
    rows.parents.sort_by_key(|(n, _)| *n);
    rows.rowids.sort_by_key(|(id, _)| *id);
    rows
}

/// The shadow-table names of the index on `table`.`geometry`.
pub(crate) fn index_name(table: &str, geometry: &str) -> String {
    format!("rtree_{table}_{geometry}")
}

/// The `CREATE VIRTUAL TABLE` statement of the index.
pub(crate) fn virtual_table_sql(table: &str, geometry: &str) -> String {
    format!(
        "CREATE VIRTUAL TABLE \"{}\" USING rtree(id, minx, maxx, miny, maxy)",
        index_name(table, geometry)
    )
}

/// The six triggers that keep the index in step with the feature table, as
/// `(name, sql)` pairs. `id` is the feature table's integer primary key (or `rowid`).
///
/// The `ST_*` functions the triggers call are the GeoPackage SQL functions; they are not
/// part of SQLite, and the triggers only run where those are defined (GDAL, SpatiaLite,
/// or the GeoPackage extension of the application).
pub(crate) fn triggers(table: &str, id: &str, geometry: &str) -> Vec<(String, String)> {
    let index = index_name(table, geometry);
    let insert = format!(
        "INSERT OR REPLACE INTO \"{index}\" VALUES (NEW.\"{id}\", \
         ST_MinX(NEW.\"{geometry}\"), ST_MaxX(NEW.\"{geometry}\"), \
         ST_MinY(NEW.\"{geometry}\"), ST_MaxY(NEW.\"{geometry}\"));"
    );
    let non_empty = format!("(NEW.\"{geometry}\" NOT NULL AND NOT ST_IsEmpty(NEW.\"{geometry}\"))");
    let empty = format!("(NEW.\"{geometry}\" ISNULL OR ST_IsEmpty(NEW.\"{geometry}\"))");
    let delete = format!("DELETE FROM \"{index}\" WHERE id = OLD.\"{id}\";");
    let mut out = Vec::new();
    let mut add = |suffix: &str, sql: String| {
        out.push((format!("{index}_{suffix}"), sql));
    };
    add(
        "insert",
        format!(
            "CREATE TRIGGER \"{index}_insert\" AFTER INSERT ON \"{table}\" \
             WHEN {non_empty} BEGIN {insert} END"
        ),
    );
    add(
        "update1",
        format!(
            "CREATE TRIGGER \"{index}_update1\" AFTER UPDATE OF \"{geometry}\" ON \"{table}\" \
             WHEN OLD.\"{id}\" = NEW.\"{id}\" AND {non_empty} BEGIN {insert} END"
        ),
    );
    add(
        "update2",
        format!(
            "CREATE TRIGGER \"{index}_update2\" AFTER UPDATE OF \"{geometry}\" ON \"{table}\" \
             WHEN OLD.\"{id}\" = NEW.\"{id}\" AND {empty} BEGIN {delete} END"
        ),
    );
    add(
        "update3",
        format!(
            "CREATE TRIGGER \"{index}_update3\" AFTER UPDATE ON \"{table}\" \
             WHEN OLD.\"{id}\" != NEW.\"{id}\" AND {non_empty} \
             BEGIN {delete} {insert} END"
        ),
    );
    add(
        "update4",
        format!(
            "CREATE TRIGGER \"{index}_update4\" AFTER UPDATE ON \"{table}\" \
             WHEN OLD.\"{id}\" != NEW.\"{id}\" AND {empty} BEGIN {delete} END"
        ),
    );
    add(
        "delete",
        format!(
            "CREATE TRIGGER \"{index}_delete\" AFTER DELETE ON \"{table}\" \
             WHEN OLD.\"{geometry}\" NOT NULL BEGIN {delete} END"
        ),
    );
    out
}

/// The `gpkg_extensions` row that declares the index.
pub(crate) fn extension_row(table: &str, geometry: &str) -> [String; 5] {
    [
        table.to_string(),
        geometry.to_string(),
        "gpkg_rtree_index".to_string(),
        "http://www.geopackage.org/spec120/#extension_rtree".to_string(),
        "write-only".to_string(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounds_round_outward() {
        let b = round_down(0.1);
        assert!(f64::from(b) <= 0.1);
        let u = round_up(0.1);
        assert!(f64::from(u) >= 0.1);
        assert!(next_down(0.0) < 0.0);
        assert!(next_up(0.0) > 0.0);
    }

    #[test]
    fn packs_a_single_leaf_root() {
        let entries = vec![(1, [0.0, 1.0, 0.0, 1.0]), (2, [2.0, 3.0, 2.0, 3.0])];
        let rows = build(&entries);
        assert_eq!(rows.nodes.len(), 1);
        assert_eq!(rows.nodes[0].0, 1);
        assert_eq!(rows.rowids, vec![(1, 1), (2, 1)]);
        assert!(rows.parents.is_empty());
    }

    #[test]
    fn large_inputs_get_a_tree_with_parents() {
        let entries: Vec<Entry> = (0..1000)
            .map(|i| {
                let x = (i % 40) as f32;
                let y = (i / 40) as f32;
                (i as i64 + 1, [x, x + 0.5, y, y + 0.5])
            })
            .collect();
        let rows = build(&entries);
        assert!(rows.nodes.len() > 10);
        assert!(rows.nodes.iter().all(|(_, blob)| blob.len() == NODE_BYTES));
        assert_eq!(rows.rowids.len(), 1000);
        // Every non-root node has exactly one parent.
        assert_eq!(rows.parents.len(), rows.nodes.len() - 1);
    }

    #[test]
    fn empty_input_is_an_empty_root() {
        let rows = build(&[]);
        assert_eq!(rows.nodes.len(), 1);
        assert_eq!(rows.nodes[0].0, 1);
        assert_eq!(rows.nodes[0].1.len(), NODE_BYTES);
        assert_eq!(&rows.nodes[0].1[..4], &[0, 0, 0, 0]);
        assert!(rows.rowids.is_empty());
    }

    #[test]
    fn bounds_of_each_geometry_kind() {
        let poly = crate::geometry::Polygon {
            rings: vec![vec![Point::new(0.0, 0.0), Point::new(4.0, 0.0), Point::new(4.0, 2.0)]],
        };
        let b = geometry_bounds(&Geometry::Polygon(poly)).unwrap();
        assert!(b[0] <= 0.0 && b[1] >= 4.0 && b[2] <= 0.0 && b[3] >= 2.0);
        assert_eq!(geometry_bounds(&Geometry::MultiPoint(Vec::new())), None);
    }
}
