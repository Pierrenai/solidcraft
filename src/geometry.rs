//! Double precision polygon geometry and bounded BSP constructive solid geometry.
use serde::{Deserialize, Serialize};
use std::ops::{Add, Div, Mul, Neg, Sub};
const EPS: f64 = 1e-7;
const MAX_POLYGONS: usize = 50_000;

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Vec3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}
impl Vec3 {
    pub const fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }
    pub fn dot(self, b: Self) -> f64 {
        self.x * b.x + self.y * b.y + self.z * b.z
    }
    pub fn cross(self, b: Self) -> Self {
        Self::new(
            self.y * b.z - self.z * b.y,
            self.z * b.x - self.x * b.z,
            self.x * b.y - self.y * b.x,
        )
    }
    pub fn length(self) -> f64 {
        self.dot(self).sqrt()
    }
    pub fn normalized(self) -> Self {
        let n = self.length();
        if n < EPS {
            Self::default()
        } else {
            self / n
        }
    }
    pub fn array(self) -> [f64; 3] {
        [self.x, self.y, self.z]
    }
    pub fn from_array(a: [f64; 3]) -> Self {
        Self::new(a[0], a[1], a[2])
    }
    pub fn rotate(self, degrees: [f64; 3]) -> Self {
        let [x, y, z] = degrees.map(f64::to_radians);
        let (s, c) = x.sin_cos();
        let v = Self::new(self.x, c * self.y - s * self.z, s * self.y + c * self.z);
        let (s, c) = y.sin_cos();
        let v = Self::new(c * v.x + s * v.z, v.y, -s * v.x + c * v.z);
        let (s, c) = z.sin_cos();
        Self::new(c * v.x - s * v.y, s * v.x + c * v.y, v.z)
    }
}
impl Add for Vec3 {
    type Output = Self;
    fn add(self, b: Self) -> Self {
        Self::new(self.x + b.x, self.y + b.y, self.z + b.z)
    }
}
impl Sub for Vec3 {
    type Output = Self;
    fn sub(self, b: Self) -> Self {
        Self::new(self.x - b.x, self.y - b.y, self.z - b.z)
    }
}
impl Mul<f64> for Vec3 {
    type Output = Self;
    fn mul(self, b: f64) -> Self {
        Self::new(self.x * b, self.y * b, self.z * b)
    }
}
impl Div<f64> for Vec3 {
    type Output = Self;
    fn div(self, b: f64) -> Self {
        self * (1.0 / b)
    }
}
impl Neg for Vec3 {
    type Output = Self;
    fn neg(self) -> Self {
        self * -1.0
    }
}

#[derive(Clone, Debug)]
pub struct Polygon {
    pub vertices: Vec<Vec3>,
    pub normal: Vec3,
    pub w: f64,
    /// Face identifier, kept through boolean operations: `feature id << 16 | local face`.
    pub tag: u32,
}
impl Polygon {
    pub fn new(vertices: Vec<Vec3>) -> Option<Self> {
        let mut clean = Vec::new();
        for v in vertices {
            if clean.last().is_none_or(|p: &Vec3| (*p - v).length() > EPS) {
                clean.push(v);
            }
        }
        if clean.len() > 2 && (clean[0] - *clean.last()?).length() < EPS {
            clean.pop();
        }
        if clean.len() < 3 {
            return None;
        }
        let mut normal = Vec3::default();
        for i in 1..clean.len() - 1 {
            let n = (clean[i] - clean[0]).cross(clean[i + 1] - clean[0]);
            if n.length() > EPS * EPS {
                normal = n.normalized();
                break;
            }
        }
        if normal.length() < 0.5 {
            return None;
        }
        let w = normal.dot(clean[0]);
        Some(Self {
            vertices: clean,
            normal,
            w,
            tag: 0,
        })
    }
    pub fn tagged(mut self, tag: u32) -> Self {
        self.tag = tag;
        self
    }
    fn flip(&mut self) {
        self.vertices.reverse();
        self.normal = -self.normal;
        self.w = -self.w;
    }
}
#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub polygons: Vec<Polygon>,
}
impl Mesh {
    pub fn triangles(&self) -> impl Iterator<Item = [Vec3; 3]> + '_ {
        self.polygons.iter().flat_map(|p| {
            (1..p.vertices.len() - 1).filter_map(move |i| {
                let t = [p.vertices[0], p.vertices[i], p.vertices[i + 1]];
                ((t[1] - t[0]).cross(t[2] - t[0]).length() > EPS * EPS).then_some(t)
            })
        })
    }
    pub fn volume(&self) -> f64 {
        self.triangles()
            .map(|t| t[0].dot(t[1].cross(t[2])) / 6.0)
            .sum::<f64>()
            .abs()
    }
    pub fn bounds(&self) -> Option<(Vec3, Vec3)> {
        let mut min = Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
        let mut max = -min;
        let mut found = false;
        for v in self.polygons.iter().flat_map(|p| &p.vertices) {
            found = true;
            min.x = min.x.min(v.x);
            min.y = min.y.min(v.y);
            min.z = min.z.min(v.z);
            max.x = max.x.max(v.x);
            max.y = max.y.max(v.y);
            max.z = max.z.max(v.z);
        }
        found.then_some((min, max))
    }
    pub fn transformed(&self, position: [f64; 3], rotation: [f64; 3]) -> Self {
        let shift = Vec3::from_array(position);
        Self {
            polygons: self
                .polygons
                .iter()
                .filter_map(|p| {
                    Polygon::new(
                        p.vertices
                            .iter()
                            .map(|v| v.rotate(rotation) + shift)
                            .collect(),
                    )
                    .map(|n| n.tagged(p.tag))
                })
                .collect(),
        }
    }
    /// Applies a point mapping. `mirror` must be true when the mapping reverses orientation.
    pub fn mapped(&self, f: impl Fn(Vec3) -> Vec3, mirror: bool) -> Self {
        Self {
            polygons: self
                .polygons
                .iter()
                .filter_map(|p| {
                    let mut v: Vec<Vec3> = p.vertices.iter().map(|v| f(*v)).collect();
                    if mirror {
                        v.reverse();
                    }
                    Polygon::new(v).map(|n| n.tagged(p.tag))
                })
                .collect(),
        }
    }
    pub fn retagged(mut self, f: impl Fn(u32) -> u32) -> Self {
        for p in &mut self.polygons {
            p.tag = f(p.tag);
        }
        self
    }
    pub fn append(&mut self, other: Mesh) {
        self.polygons.extend(other.polygons);
    }
    pub fn boolean(&self, other: &Self, op: BooleanOp) -> Result<Self, String> {
        if self.polygons.len() + other.polygons.len() > MAX_POLYGONS {
            return Err("Trop de faces pour cette opération.".into());
        }
        if self.polygons.is_empty() || other.polygons.is_empty() {
            return Err("Un solide source est vide.".into());
        }
        let mut a = Node::default();
        a.build(self.polygons.clone(), 0)?;
        let mut b = Node::default();
        b.build(other.polygons.clone(), 0)?;
        match op {
            BooleanOp::Union => {
                a.clip_to(&b)?;
                b.clip_to(&a)?;
                b.invert();
                b.clip_to(&a)?;
                b.invert();
                a.build(b.all_polygons(), 0)?;
            }
            BooleanOp::Subtract => {
                a.invert();
                a.clip_to(&b)?;
                b.clip_to(&a)?;
                b.invert();
                b.clip_to(&a)?;
                b.invert();
                a.build(b.all_polygons(), 0)?;
                a.invert();
            }
            BooleanOp::Intersect => {
                a.invert();
                b.clip_to(&a)?;
                b.invert();
                a.clip_to(&b)?;
                b.clip_to(&a)?;
                a.build(b.all_polygons(), 0)?;
                a.invert();
            }
        }
        let mesh = Self {
            polygons: a.all_polygons(),
        };
        if mesh.polygons.is_empty() || mesh.volume() < 1e-9 {
            return Err("Cette opération produit un solide vide.".into());
        }
        if mesh.polygons.len() > MAX_POLYGONS {
            return Err("Le résultat dépasse la limite de complexité.".into());
        }
        Ok(mesh)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum BooleanOp {
    Union,
    Subtract,
    Intersect,
}
impl BooleanOp {
    pub fn label(self) -> &'static str {
        match self {
            Self::Union => "Union",
            Self::Subtract => "Soustraction",
            Self::Intersect => "Intersection",
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct Plane {
    normal: Vec3,
    w: f64,
}
impl Plane {
    // Keep coplanar polygons separate: their orientation decides whether a surface belongs to the outside.
    fn split(self, poly: Polygon) -> (Vec<Polygon>, Vec<Polygon>, Vec<Polygon>, Vec<Polygon>) {
        let mut cf = Vec::new();
        let mut cb = Vec::new();
        let mut front = Vec::new();
        let mut back = Vec::new();
        let distances: Vec<f64> = poly
            .vertices
            .iter()
            .map(|v| self.normal.dot(*v) - self.w)
            .collect();
        let types: Vec<u8> = distances
            .iter()
            .map(|&d| {
                if d < -EPS {
                    2
                } else if d > EPS {
                    1
                } else {
                    0
                }
            })
            .collect();
        let kind = types.iter().fold(0, |a, b| a | b);
        match kind {
            0 => {
                if self.normal.dot(poly.normal) > 0.0 {
                    cf.push(poly)
                } else {
                    cb.push(poly)
                }
            }
            1 => front.push(poly),
            2 => back.push(poly),
            _ => {
                let mut f = Vec::new();
                let mut b = Vec::new();
                for i in 0..poly.vertices.len() {
                    let j = (i + 1) % poly.vertices.len();
                    let vi = poly.vertices[i];
                    let vj = poly.vertices[j];
                    if types[i] != 2 {
                        f.push(vi);
                    }
                    if types[i] != 1 {
                        b.push(vi);
                    }
                    if (types[i] | types[j]) == 3 {
                        let t = (distances[i] / (distances[i] - distances[j])).clamp(0.0, 1.0);
                        let v = vi + (vj - vi) * t;
                        f.push(v);
                        b.push(v);
                    }
                }
                if let Some(p) = Polygon::new(f) {
                    front.push(p.tagged(poly.tag));
                }
                if let Some(p) = Polygon::new(b) {
                    back.push(p.tagged(poly.tag));
                }
            }
        }
        (cf, cb, front, back)
    }
}
#[derive(Default, Clone, Debug)]
struct Node {
    plane: Option<Plane>,
    polygons: Vec<Polygon>,
    front: Option<Box<Node>>,
    back: Option<Box<Node>>,
}
impl Node {
    fn build(&mut self, polys: Vec<Polygon>, depth: usize) -> Result<(), String> {
        if polys.is_empty() {
            return Ok(());
        }
        if depth > 512 || polys.len() > MAX_POLYGONS {
            return Err("Opération trop complexe. Simplifiez les solides ou les perçages.".into());
        }
        let plane = *self.plane.get_or_insert(Plane {
            normal: polys[0].normal,
            w: polys[0].w,
        });
        let mut front = Vec::new();
        let mut back = Vec::new();
        for p in polys {
            let (cf, cb, f, b) = plane.split(p);
            self.polygons.extend(cf);
            self.polygons.extend(cb);
            front.extend(f);
            back.extend(b);
        }
        if !front.is_empty() {
            self.front
                .get_or_insert_with(Default::default)
                .build(front, depth + 1)?;
        }
        if !back.is_empty() {
            self.back
                .get_or_insert_with(Default::default)
                .build(back, depth + 1)?;
        }
        Ok(())
    }
    fn all_polygons(&self) -> Vec<Polygon> {
        let mut out = self.polygons.clone();
        if let Some(n) = &self.front {
            out.extend(n.all_polygons());
        }
        if let Some(n) = &self.back {
            out.extend(n.all_polygons());
        }
        out
    }
    fn invert(&mut self) {
        for p in &mut self.polygons {
            p.flip();
        }
        if let Some(p) = &mut self.plane {
            p.normal = -p.normal;
            p.w = -p.w;
        }
        if let Some(n) = &mut self.front {
            n.invert();
        }
        if let Some(n) = &mut self.back {
            n.invert();
        }
        std::mem::swap(&mut self.front, &mut self.back);
    }
    fn clip(&self, polys: Vec<Polygon>) -> Result<Vec<Polygon>, String> {
        let Some(plane) = self.plane else {
            return Ok(polys);
        };
        let mut front = Vec::new();
        let mut back = Vec::new();
        for p in polys {
            let (cf, cb, f, b) = plane.split(p);
            front.extend(cf);
            front.extend(f);
            back.extend(cb);
            back.extend(b);
        }
        if front.len() + back.len() > MAX_POLYGONS {
            return Err("Limite de complexité atteinte.".into());
        }
        if let Some(n) = &self.front {
            front = n.clip(front)?;
        }
        if let Some(n) = &self.back {
            back = n.clip(back)?;
        } else {
            back.clear();
        }
        front.extend(back);
        Ok(front)
    }
    fn clip_to(&mut self, other: &Self) -> Result<(), String> {
        self.polygons = other.clip(std::mem::take(&mut self.polygons))?;
        if let Some(n) = &mut self.front {
            n.clip_to(other)?;
        }
        if let Some(n) = &mut self.back {
            n.clip_to(other)?;
        }
        Ok(())
    }
}
fn cross2(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
}
pub fn triangulate(points: &[[f64; 2]]) -> Result<Vec<[usize; 3]>, String> {
    let n = points.len();
    if !(3..=600).contains(&n)
        || points
            .iter()
            .flatten()
            .any(|v| !v.is_finite() || v.abs() > 5000.0)
    {
        return Err("Le contour doit comporter entre 3 et 600 points valides.".into());
    }
    for i in 0..n {
        let a = points[i];
        let b = points[(i + 1) % n];
        if (a[0] - b[0]).hypot(a[1] - b[1]) < EPS {
            return Err("Deux points consécutifs se confondent.".into());
        }
        for j in i + 2..n {
            if i == 0 && j == n - 1 {
                continue;
            }
            let c = points[j];
            let d = points[(j + 1) % n];
            if cross2(a, b, c) * cross2(a, b, d) <= 0.0
                && cross2(c, d, a) * cross2(c, d, b) <= 0.0
                && a[0].min(b[0]) <= c[0].max(d[0]) + EPS
                && c[0].min(d[0]) <= a[0].max(b[0]) + EPS
                && a[1].min(b[1]) <= c[1].max(d[1]) + EPS
                && c[1].min(d[1]) <= a[1].max(b[1]) + EPS
            {
                return Err("Le contour se croise ou se touche.".into());
            }
        }
    }
    let area: f64 = (0..n)
        .map(|i| points[i][0] * points[(i + 1) % n][1] - points[(i + 1) % n][0] * points[i][1])
        .sum();
    if area.abs() < EPS {
        return Err("Le contour a une surface nulle.".into());
    }
    let mut ids: Vec<usize> = if area > 0.0 {
        (0..n).collect()
    } else {
        (0..n).rev().collect()
    };
    let mut out = Vec::new();
    while ids.len() > 3 {
        let mut ear = None;
        for i in 0..ids.len() {
            let a = ids[(i + ids.len() - 1) % ids.len()];
            let b = ids[i];
            let c = ids[(i + 1) % ids.len()];
            if cross2(points[a], points[b], points[c]) <= EPS {
                continue;
            }
            let inside = ids.iter().any(|&p| {
                p != a
                    && p != b
                    && p != c
                    && cross2(points[a], points[b], points[p]) >= -EPS
                    && cross2(points[b], points[c], points[p]) >= -EPS
                    && cross2(points[c], points[a], points[p]) >= -EPS
            });
            if !inside {
                ear = Some((i, [a, b, c]));
                break;
            }
        }
        let Some((i, t)) = ear else {
            return Err("Contour dégénéré : retirez les points alignés.".into());
        };
        out.push(t);
        ids.remove(i);
    }
    out.push([ids[0], ids[1], ids[2]]);
    Ok(out)
}
pub fn extrude(points: &[[f64; 2]], height: f64) -> Result<Mesh, String> {
    let sides: Vec<u32> = (0..points.len() as u32).map(|i| 3 + i).collect();
    extrude_tagged(points, height, [1, 2], &sides)
}
/// Prism between z = 0 and z = height. `sides[i]` tags the wall leaving point i.
pub fn extrude_tagged(
    points: &[[f64; 2]],
    height: f64,
    caps: [u32; 2],
    sides: &[u32],
) -> Result<Mesh, String> {
    let tris = triangulate(points)?;
    let signed_area: f64 = (0..points.len())
        .map(|i| {
            points[i][0] * points[(i + 1) % points.len()][1]
                - points[(i + 1) % points.len()][0] * points[i][1]
        })
        .sum();
    let mut polygons = Vec::new();
    let point = |i: usize, z: f64| Vec3::new(points[i][0], points[i][1], z);
    let sign = signed_area.signum();
    let convex = (0..points.len()).all(|i| {
        cross2(
            points[i],
            points[(i + 1) % points.len()],
            points[(i + 2) % points.len()],
        ) * sign
            >= -EPS
    });
    if convex {
        let mut indices: Vec<_> = (0..points.len()).collect();
        if signed_area < 0.0 {
            indices.reverse();
        }
        if let Some(p) = Polygon::new(indices.iter().rev().map(|i| point(*i, 0.0)).collect()) {
            polygons.push(p.tagged(caps[0]));
        }
        if let Some(p) = Polygon::new(indices.iter().map(|i| point(*i, height)).collect()) {
            polygons.push(p.tagged(caps[1]));
        }
    } else {
        for t in tris {
            if let Some(p) =
                Polygon::new(vec![point(t[2], 0.0), point(t[1], 0.0), point(t[0], 0.0)])
            {
                polygons.push(p.tagged(caps[0]));
            }
            if let Some(p) = Polygon::new(vec![
                point(t[0], height),
                point(t[1], height),
                point(t[2], height),
            ]) {
                polygons.push(p.tagged(caps[1]));
            }
        }
    }
    for i in 0..points.len() {
        let j = (i + 1) % points.len();
        let mut verts = vec![
            point(i, 0.0),
            point(j, 0.0),
            point(j, height),
            point(i, height),
        ];
        if signed_area < 0.0 {
            verts.reverse();
        }
        if let Some(p) = Polygon::new(verts) {
            polygons.push(p.tagged(sides.get(i).copied().unwrap_or(0)));
        }
    }
    Ok(Mesh { polygons })
}
pub fn cuboid(width: f64, depth: f64, height: f64) -> Result<Mesh, String> {
    extrude(
        &[
            [-width / 2.0, -depth / 2.0],
            [width / 2.0, -depth / 2.0],
            [width / 2.0, depth / 2.0],
            [-width / 2.0, depth / 2.0],
        ],
        height,
    )
}
pub fn cylinder(radius: f64, height: f64) -> Mesh {
    let n = 96;
    let points: Vec<Vec3> = (0..n)
        .map(|i| {
            let t = i as f64 * std::f64::consts::TAU / n as f64;
            Vec3::new(radius * t.cos(), radius * t.sin(), 0.0)
        })
        .collect();
    let mut polygons = Vec::new();
    let top: Vec<_> = points
        .iter()
        .map(|p| *p + Vec3::new(0.0, 0.0, height))
        .collect();
    let mut bottom = points.clone();
    bottom.reverse();
    if let Some(p) = Polygon::new(bottom) {
        polygons.push(p.tagged(1));
    }
    if let Some(p) = Polygon::new(top.clone()) {
        polygons.push(p.tagged(2));
    }
    for i in 0..n {
        let j = (i + 1) % n;
        if let Some(p) = Polygon::new(vec![points[i], points[j], top[j], top[i]]) {
            polygons.push(p.tagged(3));
        }
    }
    Mesh { polygons }
}
