//! Faces and edges recovered from tagged polygons.
//!
//! A face is the set of polygons sharing a tag. Its boundary is found by cancelling, on each
//! supporting line, the polygon sides travelled in both directions; this tolerates the
//! T-junctions left by the BSP booleans. An edge is a chain of boundary pieces between two faces.
use crate::geometry::{Mesh, Vec3};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub struct Face {
    pub tag: u32,
    /// Outward normal and offset when the face is planar.
    pub plane: Option<(Vec3, f64)>,
    pub area: f64,
    pub centroid: Vec3,
}
#[derive(Clone, Debug)]
pub struct Edge {
    pub faces: [u32; 2],
    pub segments: Vec<[Vec3; 2]>,
    /// Endpoints when the edge is a single straight line.
    pub line: Option<[Vec3; 2]>,
    /// In-face directions leaving the edge (perpendicular to it) for faces[0] and faces[1].
    pub inward: [Vec3; 2],
    pub length: f64,
}
impl Edge {
    pub fn midpoint(&self) -> Vec3 {
        if let Some([a, b]) = self.line {
            return (a + b) * 0.5;
        }
        let mut best = self.segments[0];
        let mut walked = 0.0;
        for s in &self.segments {
            walked += (s[1] - s[0]).length();
            best = *s;
            if walked >= self.length / 2.0 {
                break;
            }
        }
        (best[0] + best[1]) * 0.5
    }
    pub fn distance_to(&self, p: Vec3) -> f64 {
        self.segments
            .iter()
            .map(|s| segment_distance(p, s[0], s[1]))
            .fold(f64::INFINITY, f64::min)
    }
}
pub fn segment_distance(p: Vec3, a: Vec3, b: Vec3) -> f64 {
    let d = b - a;
    let t = ((p - a).dot(d) / d.dot(d).max(1e-18)).clamp(0.0, 1.0);
    (a + d * t - p).length()
}
#[derive(Clone, Debug, Default)]
pub struct Topology {
    pub faces: Vec<Face>,
    pub edges: Vec<Edge>,
}
impl Topology {
    pub fn face(&self, tag: u32) -> Option<&Face> {
        self.faces.iter().find(|f| f.tag == tag)
    }
}
type LineKey = (i64, i64, i64, i64, i64, i64);
fn line_key(a: Vec3, b: Vec3) -> (LineKey, Vec3, f64, f64, i8) {
    let mut d = (b - a).normalized();
    let mut sign = 1;
    let first = if d.x.abs() > 1e-9 {
        d.x
    } else if d.y.abs() > 1e-9 {
        d.y
    } else {
        d.z
    };
    if first < 0.0 {
        d = -d;
        sign = -1;
    }
    let base = a - d * a.dot(d);
    let q = |v: f64, s: f64| (v * s).round() as i64;
    (
        (
            q(d.x, 1e5),
            q(d.y, 1e5),
            q(d.z, 1e5),
            q(base.x, 1e4),
            q(base.y, 1e4),
            q(base.z, 1e4),
        ),
        d,
        a.dot(d),
        b.dot(d),
        sign,
    )
}
struct Line {
    dir: Vec3,
    base: Vec3,
    /// (tag, start, end, orientation)
    pieces: Vec<(u32, f64, f64, i8)>,
}
pub fn analyse(mesh: &Mesh) -> Topology {
    let mut lines: HashMap<LineKey, Line> = HashMap::new();
    let mut faces: HashMap<u32, (Vec3, f64, bool, f64, Vec3)> = HashMap::new();
    for p in &mesh.polygons {
        let mut area = 0.0;
        let mut centroid = Vec3::default();
        for i in 1..p.vertices.len().saturating_sub(1) {
            let a = (p.vertices[i] - p.vertices[0])
                .cross(p.vertices[i + 1] - p.vertices[0])
                .length()
                * 0.5;
            area += a;
            centroid = centroid + (p.vertices[0] + p.vertices[i] + p.vertices[i + 1]) * (a / 3.0);
        }
        let entry = faces
            .entry(p.tag)
            .or_insert((p.normal, p.w, true, 0.0, Vec3::default()));
        if entry.0.dot(p.normal) < 0.999_999 || (entry.1 - p.w).abs() > 1e-5 {
            entry.2 = false;
        }
        entry.3 += area;
        entry.4 = entry.4 + centroid;
        for i in 0..p.vertices.len() {
            let a = p.vertices[i];
            let b = p.vertices[(i + 1) % p.vertices.len()];
            if (b - a).length() < 1e-9 {
                continue;
            }
            let (key, dir, ta, tb, sign) = line_key(a, b);
            let line = lines.entry(key).or_insert_with(|| Line {
                dir,
                base: a - dir * a.dot(dir),
                pieces: Vec::new(),
            });
            line.pieces.push((p.tag, ta.min(tb), ta.max(tb), sign));
        }
    }
    let mut faces: Vec<Face> = faces
        .into_iter()
        .map(|(tag, (n, w, planar, area, c))| Face {
            tag,
            plane: planar.then_some((n, w)),
            area,
            centroid: if area > 0.0 { c / area } else { c },
        })
        .collect();
    faces.sort_by_key(|f| f.tag);
    // Boundary pieces between two faces, per supporting line.
    type FaceEdge = ([Vec3; 2], [i8; 2], LineKey);
    let mut pairs: HashMap<(u32, u32), Vec<FaceEdge>> = HashMap::new();
    for (key, line) in &lines {
        let mut cuts: Vec<f64> = line.pieces.iter().flat_map(|p| [p.1, p.2]).collect();
        cuts.sort_by(f64::total_cmp);
        cuts.dedup_by(|a, b| (*a - *b).abs() < 1e-7);
        for w in cuts.windows(2) {
            let (t0, t1) = (w[0], w[1]);
            if t1 - t0 < 1e-7 {
                continue;
            }
            let mid = (t0 + t1) / 2.0;
            let mut cover: Vec<(u32, i32)> = Vec::new();
            for &(tag, s, e, sign) in &line.pieces {
                if s <= mid && mid <= e {
                    if let Some(c) = cover.iter_mut().find(|c| c.0 == tag) {
                        c.1 += sign as i32;
                    } else {
                        cover.push((tag, sign as i32));
                    }
                }
            }
            cover.retain(|c| c.1 != 0);
            if cover.len() != 2 || cover[0].1.signum() == cover[1].1.signum() {
                continue;
            }
            let (a, b) = if cover[0].0 < cover[1].0 {
                (cover[0], cover[1])
            } else {
                (cover[1], cover[0])
            };
            let seg = [line.base + line.dir * t0, line.base + line.dir * t1];
            pairs.entry((a.0, b.0)).or_default().push((
                seg,
                [a.1.signum() as i8, b.1.signum() as i8],
                *key,
            ));
        }
    }
    let face_normal = |tag: u32, at: Vec3| -> Vec3 {
        // Normal of the polygon of this face nearest to the point.
        mesh.polygons
            .iter()
            .filter(|p| p.tag == tag)
            .map(|p| {
                (
                    p.vertices
                        .iter()
                        .map(|v| (*v - at).length())
                        .fold(f64::INFINITY, f64::min),
                    p.normal,
                )
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|v| v.1)
            .unwrap_or_default()
    };
    let mut edges = Vec::new();
    let mut keys: Vec<_> = pairs.keys().copied().collect();
    keys.sort();
    for pair in keys {
        let mut pieces = pairs.remove(&pair).unwrap_or_default();
        // Split into connected chains.
        while let Some(first) = pieces.pop() {
            let mut chain = vec![first];
            let mut grew = true;
            while grew {
                grew = false;
                let mut i = 0;
                while i < pieces.len() {
                    let s = pieces[i].0;
                    let touches = chain.iter().any(|c| {
                        [c.0[0], c.0[1]]
                            .iter()
                            .any(|p| (*p - s[0]).length() < 1e-6 || (*p - s[1]).length() < 1e-6)
                    });
                    if touches {
                        chain.push(pieces.swap_remove(i));
                        grew = true;
                    } else {
                        i += 1;
                    }
                }
            }
            let segments: Vec<[Vec3; 2]> = chain.iter().map(|c| c.0).collect();
            let length: f64 = segments.iter().map(|s| (s[1] - s[0]).length()).sum();
            if length < 1e-6 {
                continue;
            }
            let straight = chain.iter().all(|c| c.2 == chain[0].2);
            let line = straight.then(|| {
                let dir = (segments[0][1] - segments[0][0]).normalized();
                let ts: Vec<f64> = segments
                    .iter()
                    .flat_map(|s| [s[0].dot(dir), s[1].dot(dir)])
                    .collect();
                let lo = ts.iter().copied().fold(f64::INFINITY, f64::min);
                let hi = ts.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                let base = segments[0][0] - dir * segments[0][0].dot(dir);
                [base + dir * lo, base + dir * hi]
            });
            // A face lies on the left of its boundary when seen from outside: inward = n × d.
            let s = chain[0].0;
            let d = (s[1] - s[0]).normalized();
            let mid = (s[0] + s[1]) * 0.5;
            let inward = [0, 1].map(|k| {
                let tag = if k == 0 { pair.0 } else { pair.1 };
                let n = face_normal(tag, mid);
                n.cross(d * chain[0].1[k] as f64).normalized()
            });
            edges.push(Edge {
                faces: [pair.0, pair.1],
                segments,
                line,
                inward,
                length,
            });
        }
    }
    Topology { faces, edges }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{cuboid, cylinder, BooleanOp};
    #[test]
    fn box_has_six_faces_and_twelve_edges() {
        let t = analyse(&cuboid(10.0, 20.0, 5.0).unwrap());
        assert_eq!(t.faces.len(), 6);
        assert!(t.faces.iter().all(|f| f.plane.is_some()));
        assert_eq!(t.edges.len(), 12);
        assert!(t.edges.iter().all(|e| e.line.is_some()));
        let total: f64 = t.edges.iter().map(|e| e.length).sum();
        assert!((total - 4.0 * 35.0).abs() < 1e-6);
    }
    #[test]
    fn edges_survive_booleans_with_t_junctions() {
        let a = cuboid(40.0, 40.0, 10.0).unwrap();
        let hole = cylinder(5.0, 20.0)
            .transformed([0.0, 0.0, -5.0], [0.0; 3])
            .retagged(|t| t + 100);
        let m = a.boolean(&hole, BooleanOp::Subtract).unwrap();
        let t = analyse(&m);
        assert_eq!(t.faces.len(), 7);
        // 12 box edges + 2 circular edges.
        assert_eq!(t.edges.len(), 14);
        let circles: Vec<_> = t.edges.iter().filter(|e| e.line.is_none()).collect();
        assert_eq!(circles.len(), 2);
        for c in circles {
            assert!((c.length - std::f64::consts::TAU * 5.0).abs() < 0.1);
        }
    }
    #[test]
    fn inward_directions_point_into_faces() {
        let t = analyse(&cuboid(10.0, 10.0, 10.0).unwrap());
        for e in &t.edges {
            let m = e.midpoint();
            for k in 0..2 {
                let f = t.face(e.faces[k]).unwrap();
                assert!((f.centroid - m).dot(e.inward[k]) > 0.0);
            }
        }
    }
}
