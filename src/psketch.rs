//! Profile sketches of the feature-based part: points, lines, circles and arcs on a plane,
//! geometric relations and driving dimensions solved together.
use crate::geometry::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct SketchPlane {
    pub origin: [f64; 3],
    pub u: [f64; 3],
    pub v: [f64; 3],
    /// Face of the part the plane was taken from; the plane follows that face on rebuild.
    #[serde(default)]
    pub face: Option<u32>,
}
impl SketchPlane {
    pub fn new(origin: Vec3, u: Vec3, v: Vec3) -> Self {
        Self {
            origin: origin.array(),
            u: u.normalized().array(),
            v: v.normalized().array(),
            face: None,
        }
    }
    /// Plan de face (XZ, regardé depuis -Y).
    pub fn front() -> Self {
        Self::new(
            Vec3::default(),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        )
    }
    /// Plan de dessus (XY).
    pub fn top() -> Self {
        Self::new(
            Vec3::default(),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        )
    }
    /// Plan de droite (YZ, regardé depuis +X).
    pub fn right() -> Self {
        Self::new(
            Vec3::default(),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        )
    }
    pub fn origin(&self) -> Vec3 {
        Vec3::from_array(self.origin)
    }
    pub fn u(&self) -> Vec3 {
        Vec3::from_array(self.u)
    }
    pub fn v(&self) -> Vec3 {
        Vec3::from_array(self.v)
    }
    pub fn normal(&self) -> Vec3 {
        self.u().cross(self.v()).normalized()
    }
    pub fn world(&self, p: [f64; 2]) -> Vec3 {
        self.origin() + self.u() * p[0] + self.v() * p[1]
    }
    pub fn local(&self, p: Vec3) -> [f64; 2] {
        let d = p - self.origin();
        [d.dot(self.u()), d.dot(self.v())]
    }
    pub fn offset(&self, distance: f64) -> Self {
        let mut p = *self;
        p.origin = (self.origin() + self.normal() * distance).array();
        p.face = None;
        p
    }
    pub fn flipped(&self) -> Self {
        let mut p = *self;
        p.u = (-self.u()).array();
        p
    }
    /// Plane through a face, with axes chosen like a drawing view of that face.
    pub fn from_face(point: Vec3, normal: Vec3, tag: Option<u32>) -> Self {
        let n = normal.normalized();
        let z = Vec3::new(0.0, 0.0, 1.0);
        let up = if n.dot(z).abs() > 0.9 {
            Vec3::new(0.0, 1.0, 0.0) * n.z.signum()
        } else {
            (z - n * n.dot(z)).normalized()
        };
        let mut p = Self::new(point, up.cross(n), up);
        p.face = tag;
        p
    }
    pub fn is_valid(&self) -> bool {
        let (u, v) = (self.u(), self.v());
        self.origin
            .iter()
            .chain(&self.u)
            .chain(&self.v)
            .all(|x| x.is_finite() && x.abs() <= 1e5)
            && (u.length() - 1.0).abs() < 1e-6
            && (v.length() - 1.0).abs() < 1e-6
            && u.dot(v).abs() < 1e-6
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Entity {
    Line {
        a: usize,
        b: usize,
        #[serde(default)]
        construction: bool,
    },
    Circle {
        c: usize,
        r: f64,
        #[serde(default)]
        construction: bool,
    },
    /// Counter-clockwise from `a` to `b` around `c`.
    Arc {
        c: usize,
        a: usize,
        b: usize,
        #[serde(default)]
        construction: bool,
    },
}
impl Entity {
    pub fn construction(&self) -> bool {
        match *self {
            Self::Line { construction, .. }
            | Self::Circle { construction, .. }
            | Self::Arc { construction, .. } => construction,
        }
    }
    pub fn set_construction(&mut self, value: bool) {
        match self {
            Self::Line { construction, .. }
            | Self::Circle { construction, .. }
            | Self::Arc { construction, .. } => *construction = value,
        }
    }
    pub fn points(&self) -> Vec<usize> {
        match *self {
            Self::Line { a, b, .. } => vec![a, b],
            Self::Circle { c, .. } => vec![c],
            Self::Arc { c, a, b, .. } => vec![c, a, b],
        }
    }
    pub fn label(&self) -> &'static str {
        match self {
            Self::Line {
                construction: true, ..
            } => "Ligne de construction",
            Self::Line { .. } => "Ligne",
            Self::Circle { .. } => "Cercle",
            Self::Arc { .. } => "Arc",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Rel {
    Coincident(usize, usize),
    Horizontal(usize),
    Vertical(usize),
    HorizontalPoints(usize, usize),
    VerticalPoints(usize, usize),
    Parallel(usize, usize),
    Perpendicular(usize, usize),
    Equal(usize, usize),
    Tangent(usize, usize),
    Concentric(usize, usize),
    Midpoint(usize, usize),
    PointOn(usize, usize),
    Fixed(usize, [f64; 2]),
}
impl Rel {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Coincident(..) => "Coïncidente",
            Self::Horizontal(_) | Self::HorizontalPoints(..) => "Horizontale",
            Self::Vertical(_) | Self::VerticalPoints(..) => "Verticale",
            Self::Parallel(..) => "Parallèle",
            Self::Perpendicular(..) => "Perpendiculaire",
            Self::Equal(..) => "Égale",
            Self::Tangent(..) => "Tangente",
            Self::Concentric(..) => "Concentrique",
            Self::Midpoint(..) => "Point milieu",
            Self::PointOn(..) => "Sur l’entité",
            Self::Fixed(..) => "Fixe",
        }
    }
    /// Entities referenced (for display and deletion).
    pub fn entities(&self) -> Vec<usize> {
        match *self {
            Self::Horizontal(e) | Self::Vertical(e) => vec![e],
            Self::Parallel(a, b)
            | Self::Perpendicular(a, b)
            | Self::Equal(a, b)
            | Self::Tangent(a, b)
            | Self::Concentric(a, b) => vec![a, b],
            Self::Midpoint(_, e) | Self::PointOn(_, e) => vec![e],
            _ => vec![],
        }
    }
    pub fn points(&self) -> Vec<usize> {
        match *self {
            Self::Coincident(a, b) | Self::HorizontalPoints(a, b) | Self::VerticalPoints(a, b) => {
                vec![a, b]
            }
            Self::Midpoint(p, _) | Self::PointOn(p, _) | Self::Fixed(p, _) => vec![p],
            _ => vec![],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum DimKind {
    Distance(usize, usize),
    /// Horizontal distance between points (sign kept from creation).
    DistanceX(usize, usize),
    DistanceY(usize, usize),
    PointLine(usize, usize),
    Length(usize),
    Diameter(usize),
    Radius(usize),
    Angle(usize, usize),
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Dim {
    pub kind: DimKind,
    pub value: f64,
    /// Label position relative to the measured geometry, in sketch units.
    pub offset: [f64; 2],
    #[serde(default)]
    pub flip: bool,
}
impl Dim {
    pub fn text(&self) -> String {
        match self.kind {
            DimKind::Diameter(_) => format!("Ø{}", fmt(self.value)),
            DimKind::Radius(_) => format!("R{}", fmt(self.value)),
            DimKind::Angle(..) => format!("{}°", fmt(self.value)),
            _ => fmt(self.value),
        }
    }
}
pub fn fmt(v: f64) -> String {
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    s.replace('.', ",")
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ProfileSketch {
    pub plane: Option<SketchPlane>,
    /// Point 0 is the sketch origin and never moves.
    pub points: Vec<[f64; 2]>,
    pub entities: Vec<Entity>,
    pub rels: Vec<Rel>,
    pub dims: Vec<Dim>,
}

/// Closed contour sampled for features. `entity[i]` is the sketch entity of segment i.
#[derive(Clone, Debug)]
pub struct Contour {
    pub points: Vec<[f64; 2]>,
    pub entity: Vec<usize>,
    /// Number of other contours around this one: even = material, odd = hole.
    pub depth: usize,
}

#[derive(Clone, Debug, Default)]
pub struct Status {
    pub dof: usize,
    /// Per point: true when fully determined.
    pub defined_points: Vec<bool>,
    pub defined_radius: Vec<bool>,
}
impl Status {
    pub fn fully_defined(&self) -> bool {
        self.dof == 0
    }
}

const CIRCLE_STEPS: usize = 96;

impl ProfileSketch {
    pub fn on(plane: SketchPlane) -> Self {
        Self {
            plane: Some(plane),
            points: vec![[0.0, 0.0]],
            ..Self::default()
        }
    }
    pub fn add_point(&mut self, p: [f64; 2]) -> usize {
        self.points.push(p);
        self.points.len() - 1
    }
    pub fn add(&mut self, e: Entity) -> usize {
        self.entities.push(e);
        self.entities.len() - 1
    }
    pub fn validate(&self) -> Result<(), String> {
        let np = self.points.len();
        if np == 0
            || np > 400
            || self.entities.len() > 300
            || self.rels.len() > 600
            || self.dims.len() > 300
            || self
                .points
                .iter()
                .flatten()
                .any(|v| !v.is_finite() || v.abs() > 1e5)
        {
            return Err("Esquisse trop complexe ou invalide.".into());
        }
        if let Some(p) = &self.plane {
            if !p.is_valid() {
                return Err("Plan d’esquisse invalide.".into());
            }
        }
        let ne = self.entities.len();
        for e in &self.entities {
            if e.points().iter().any(|p| *p >= np) {
                return Err("Entité d’esquisse invalide.".into());
            }
            if let Entity::Circle { r, .. } = e {
                if !r.is_finite() || *r < 1e-4 || *r > 1e5 {
                    return Err("Rayon invalide.".into());
                }
            }
        }
        for r in &self.rels {
            if r.points().iter().any(|p| *p >= np) || r.entities().iter().any(|e| *e >= ne) {
                return Err("Relation invalide.".into());
            }
        }
        for d in &self.dims {
            let ok = match d.kind {
                DimKind::Distance(a, b) | DimKind::DistanceX(a, b) | DimKind::DistanceY(a, b) => {
                    a < np && b < np
                }
                DimKind::PointLine(p, e) => p < np && e < ne,
                DimKind::Length(e) | DimKind::Diameter(e) | DimKind::Radius(e) => e < ne,
                DimKind::Angle(a, b) => a < ne && b < ne,
            };
            if !ok || !d.value.is_finite() || d.value < 0.0 || d.value > 1e5 {
                return Err("Cote invalide.".into());
            }
        }
        Ok(())
    }
    fn radius_vars(&self) -> Vec<usize> {
        self.entities
            .iter()
            .enumerate()
            .filter(|(_, e)| matches!(e, Entity::Circle { .. }))
            .map(|(i, _)| i)
            .collect()
    }
    fn pack(&self) -> Vec<f64> {
        let mut x: Vec<f64> = self.points[1..].iter().flatten().copied().collect();
        for i in self.radius_vars() {
            if let Entity::Circle { r, .. } = self.entities[i] {
                x.push(r);
            }
        }
        x
    }
    fn unpack(&self, x: &[f64]) -> (Vec<[f64; 2]>, Vec<f64>) {
        let mut pts = vec![[0.0, 0.0]];
        let n = self.points.len() - 1;
        for i in 0..n {
            pts.push([x[2 * i], x[2 * i + 1]]);
        }
        let mut radii = vec![0.0; self.entities.len()];
        for (k, i) in self.radius_vars().into_iter().enumerate() {
            radii[i] = x[2 * n + k];
        }
        for (i, e) in self.entities.iter().enumerate() {
            if let Entity::Arc { c, a, .. } = *e {
                radii[i] = dist(pts[c], pts[a]);
            }
        }
        (pts, radii)
    }
    fn residuals(&self, x: &[f64], extra: &[Rel]) -> Vec<f64> {
        let (p, r) = self.unpack(x);
        let mut out = Vec::new();
        let ent = |e: usize| self.entities[e];
        let line = |e: usize| match ent(e) {
            Entity::Line { a, b, .. } => Some((p[a], p[b])),
            _ => None,
        };
        let center = |e: usize| match ent(e) {
            Entity::Circle { c, .. } | Entity::Arc { c, .. } => Some(p[c]),
            _ => None,
        };
        let unit = |v: [f64; 2]| {
            let l = v[0].hypot(v[1]).max(1e-9);
            [v[0] / l, v[1] / l]
        };
        for e in &self.entities {
            if let Entity::Arc { c, a, b, .. } = *e {
                out.push(dist(p[c], p[a]) - dist(p[c], p[b]));
            }
        }
        for rel in self.rels.iter().chain(extra) {
            match *rel {
                Rel::Coincident(a, b) => {
                    out.push(p[a][0] - p[b][0]);
                    out.push(p[a][1] - p[b][1]);
                }
                Rel::Horizontal(e) => {
                    if let Some((a, b)) = line(e) {
                        out.push(b[1] - a[1]);
                    }
                }
                Rel::Vertical(e) => {
                    if let Some((a, b)) = line(e) {
                        out.push(b[0] - a[0]);
                    }
                }
                Rel::HorizontalPoints(a, b) => out.push(p[b][1] - p[a][1]),
                Rel::VerticalPoints(a, b) => out.push(p[b][0] - p[a][0]),
                Rel::Parallel(e, f) | Rel::Perpendicular(e, f) => {
                    if let (Some(l1), Some(l2)) = (line(e), line(f)) {
                        let u = unit(sub(l1.1, l1.0));
                        let v = unit(sub(l2.1, l2.0));
                        out.push(
                            10.0 * if matches!(rel, Rel::Parallel(..)) {
                                u[0] * v[1] - u[1] * v[0]
                            } else {
                                u[0] * v[0] + u[1] * v[1]
                            },
                        );
                    }
                }
                Rel::Equal(e, f) => match (line(e), line(f)) {
                    (Some(l1), Some(l2)) => out.push(dist(l1.0, l1.1) - dist(l2.0, l2.1)),
                    (None, None) => out.push(r[e] - r[f]),
                    _ => {}
                },
                Rel::Tangent(e, f) => match (line(e), line(f)) {
                    (Some(l), None) | (None, Some(l)) => {
                        let circle = if line(e).is_some() { f } else { e };
                        if let Some(c) = center(circle) {
                            out.push(line_distance(c, l.0, l.1).abs() - r[circle]);
                        }
                    }
                    (None, None) => {
                        if let (Some(c1), Some(c2)) = (center(e), center(f)) {
                            out.push(dist(c1, c2) - (r[e] + r[f]));
                        }
                    }
                    _ => {}
                },
                Rel::Concentric(e, f) => {
                    if let (Some(c1), Some(c2)) = (center(e), center(f)) {
                        out.push(c1[0] - c2[0]);
                        out.push(c1[1] - c2[1]);
                    }
                }
                Rel::Midpoint(q, e) => {
                    if let Some((a, b)) = line(e) {
                        out.push(p[q][0] - (a[0] + b[0]) / 2.0);
                        out.push(p[q][1] - (a[1] + b[1]) / 2.0);
                    }
                }
                Rel::PointOn(q, e) => {
                    if let Some((a, b)) = line(e) {
                        out.push(line_distance(p[q], a, b));
                    } else if let Some(c) = center(e) {
                        out.push(dist(p[q], c) - r[e]);
                    }
                }
                Rel::Fixed(q, at) => {
                    out.push(p[q][0] - at[0]);
                    out.push(p[q][1] - at[1]);
                }
            }
        }
        for d in &self.dims {
            let v = d.value;
            match d.kind {
                DimKind::Distance(a, b) => out.push(dist(p[a], p[b]) - v),
                DimKind::DistanceX(a, b) => {
                    out.push(p[b][0] - p[a][0] - if d.flip { -v } else { v })
                }
                DimKind::DistanceY(a, b) => {
                    out.push(p[b][1] - p[a][1] - if d.flip { -v } else { v })
                }
                DimKind::PointLine(q, e) => {
                    if let Some((a, b)) = line(e) {
                        out.push(line_distance(p[q], a, b).abs() - v);
                    }
                }
                DimKind::Length(e) => {
                    if let Some((a, b)) = line(e) {
                        out.push(dist(a, b) - v);
                    }
                }
                DimKind::Diameter(e) => out.push(2.0 * r[e] - v),
                DimKind::Radius(e) => out.push(r[e] - v),
                DimKind::Angle(e, f) => {
                    if let (Some(l1), Some(l2)) = (line(e), line(f)) {
                        let u = unit(sub(l1.1, l1.0));
                        let w = unit(sub(l2.1, l2.0));
                        let angle = (u[0] * w[1] - u[1] * w[0])
                            .atan2(u[0] * w[0] + u[1] * w[1])
                            .abs();
                        out.push(10.0 * (angle - v.to_radians()));
                    }
                }
            }
        }
        out
    }
    fn store(&mut self, x: &[f64]) {
        let (p, r) = self.unpack(x);
        self.points = p;
        for (i, e) in self.entities.iter_mut().enumerate() {
            if let Entity::Circle { r: radius, .. } = e {
                *radius = r[i];
            }
        }
    }
    /// Solves relations and dimensions. On failure the sketch is left unchanged.
    pub fn solve(&mut self) -> Result<(), String> {
        self.solve_with(&[])
    }
    /// Solves with temporary relations, used while dragging geometry.
    pub fn solve_with(&mut self, extra: &[Rel]) -> Result<(), String> {
        self.validate()?;
        let mut x = self.pack();
        if x.is_empty() {
            return Ok(());
        }
        let this = self.clone();
        crate::solver::solve(&mut x, |x| this.residuals(x, extra))?;
        if x.iter().any(|v| !v.is_finite() || v.abs() > 1e5) {
            return Err("Solution hors limites.".into());
        }
        self.store(&x);
        Ok(())
    }
    pub fn is_solved(&self) -> bool {
        let x = self.pack();
        x.is_empty() || self.residuals(&x, &[]).iter().all(|v| v.abs() < 1e-4)
    }
    /// Degrees of freedom left, per point: a variable is determined when every null-space
    /// direction of the constraint Jacobian leaves it unchanged.
    pub fn status(&self) -> Status {
        let x = self.pack();
        let n = x.len();
        let np = self.points.len();
        let mut status = Status {
            dof: n,
            defined_points: vec![false; np],
            defined_radius: vec![false; self.entities.len()],
        };
        if np > 0 {
            status.defined_points[0] = true;
        }
        if n == 0 || n > 400 {
            return status;
        }
        let r0 = self.residuals(&x, &[]);
        let m = r0.len();
        let mut j = vec![vec![0.0; n]; m];
        for col in 0..n {
            let mut p = x.clone();
            let h = 1e-6 * (1.0 + x[col].abs());
            p[col] += h;
            let r = self.residuals(&p, &[]);
            for row in 0..m {
                j[row][col] = (r[row] - r0[row]) / h;
            }
        }
        // Reduced row echelon form.
        let mut pivots = Vec::new();
        let mut row = 0;
        for col in 0..n {
            if row >= m {
                break;
            }
            let Some(best) = (row..m).max_by(|&a, &b| j[a][col].abs().total_cmp(&j[b][col].abs()))
            else {
                break;
            };
            if j[best][col].abs() < 1e-7 {
                continue;
            }
            j.swap(row, best);
            let d = j[row][col];
            for v in j[row].iter_mut() {
                *v /= d;
            }
            let pivot_row = j[row].clone();
            for (r, rowv) in j.iter_mut().enumerate() {
                if r != row {
                    let f = rowv[col];
                    if f != 0.0 {
                        for (k, v) in rowv.iter_mut().enumerate() {
                            *v -= f * pivot_row[k];
                        }
                    }
                }
            }
            pivots.push(col);
            row += 1;
        }
        let free: Vec<usize> = (0..n).filter(|c| !pivots.contains(c)).collect();
        status.dof = free.len();
        let mut determined = vec![true; n];
        for &f in &free {
            determined[f] = false;
            for (r, &pc) in pivots.iter().enumerate() {
                if j[r][f].abs() > 1e-7 {
                    determined[pc] = false;
                }
            }
        }
        for i in 1..np {
            status.defined_points[i] = determined[2 * (i - 1)] && determined[2 * (i - 1) + 1];
        }
        for (k, e) in self.radius_vars().into_iter().enumerate() {
            status.defined_radius[e] = determined[2 * (np - 1) + k];
        }
        for (i, e) in self.entities.iter().enumerate() {
            if let Entity::Arc { a, .. } = *e {
                status.defined_radius[i] = status.defined_points[a];
            }
        }
        status
    }
    /// Current measured value of a dimension.
    pub fn measure(&self, kind: DimKind) -> f64 {
        let x = self.pack();
        let d = Dim {
            kind,
            value: 0.0,
            offset: [0.0; 2],
            flip: false,
        };
        let probe = Self {
            dims: vec![d],
            rels: vec![],
            ..self.clone()
        };
        let r = probe.residuals(&x, &[]);
        let arcs = self
            .entities
            .iter()
            .filter(|e| matches!(e, Entity::Arc { .. }))
            .count();
        let v = r.get(arcs).copied().unwrap_or(0.0);
        match kind {
            DimKind::Angle(..) => (v / 10.0).to_degrees(),
            _ => v,
        }
    }
    pub fn radius(&self, e: usize) -> f64 {
        match self.entities[e] {
            Entity::Circle { r, .. } => r,
            Entity::Arc { c, a, .. } => dist(self.points[c], self.points[a]),
            _ => 0.0,
        }
    }
    /// Points along an entity for display.
    pub fn polyline(&self, e: usize) -> Vec<[f64; 2]> {
        let p = &self.points;
        match self.entities[e] {
            Entity::Line { a, b, .. } => vec![p[a], p[b]],
            Entity::Circle { c, r, .. } => (0..=CIRCLE_STEPS)
                .map(|i| {
                    let t = i as f64 / CIRCLE_STEPS as f64 * std::f64::consts::TAU;
                    [p[c][0] + r * t.cos(), p[c][1] + r * t.sin()]
                })
                .collect(),
            Entity::Arc { c, a, b, .. } => arc_points(p[c], p[a], p[b], false),
        }
    }
    pub fn distance_to(&self, e: usize, q: [f64; 2]) -> f64 {
        let pl = self.polyline(e);
        pl.windows(2)
            .map(|w| segment_distance(q, w[0], w[1]))
            .fold(f64::INFINITY, f64::min)
    }
    /// Removes an entity and points no longer used, renumbering references.
    pub fn delete_entity(&mut self, e: usize) {
        self.entities.remove(e);
        let fix = |i: usize| if i > e { i - 1 } else { i };
        self.rels.retain(|r| !r.entities().contains(&e));
        for r in &mut self.rels {
            *r = match *r {
                Rel::Horizontal(x) => Rel::Horizontal(fix(x)),
                Rel::Vertical(x) => Rel::Vertical(fix(x)),
                Rel::Parallel(a, b) => Rel::Parallel(fix(a), fix(b)),
                Rel::Perpendicular(a, b) => Rel::Perpendicular(fix(a), fix(b)),
                Rel::Equal(a, b) => Rel::Equal(fix(a), fix(b)),
                Rel::Tangent(a, b) => Rel::Tangent(fix(a), fix(b)),
                Rel::Concentric(a, b) => Rel::Concentric(fix(a), fix(b)),
                Rel::Midpoint(p, x) => Rel::Midpoint(p, fix(x)),
                Rel::PointOn(p, x) => Rel::PointOn(p, fix(x)),
                other => other,
            };
        }
        self.dims.retain(|d| match d.kind {
            DimKind::PointLine(_, x)
            | DimKind::Length(x)
            | DimKind::Diameter(x)
            | DimKind::Radius(x) => x != e,
            DimKind::Angle(a, b) => a != e && b != e,
            _ => true,
        });
        for d in &mut self.dims {
            d.kind = match d.kind {
                DimKind::PointLine(p, x) => DimKind::PointLine(p, fix(x)),
                DimKind::Length(x) => DimKind::Length(fix(x)),
                DimKind::Diameter(x) => DimKind::Diameter(fix(x)),
                DimKind::Radius(x) => DimKind::Radius(fix(x)),
                DimKind::Angle(a, b) => DimKind::Angle(fix(a), fix(b)),
                other => other,
            };
        }
        self.purge_points();
    }
    pub fn purge_points(&mut self) {
        let mut used = vec![false; self.points.len()];
        used[0] = true;
        for e in &self.entities {
            for p in e.points() {
                used[p] = true;
            }
        }
        let mut map = vec![usize::MAX; self.points.len()];
        let mut next = 0;
        for (i, u) in used.iter().enumerate() {
            if *u {
                map[i] = next;
                next += 1;
            }
        }
        if next == self.points.len() {
            return;
        }
        self.points = self
            .points
            .iter()
            .enumerate()
            .filter(|(i, _)| used[*i])
            .map(|(_, p)| *p)
            .collect();
        for e in &mut self.entities {
            match e {
                Entity::Line { a, b, .. } => {
                    *a = map[*a];
                    *b = map[*b];
                }
                Entity::Circle { c, .. } => *c = map[*c],
                Entity::Arc { c, a, b, .. } => {
                    *c = map[*c];
                    *a = map[*a];
                    *b = map[*b];
                }
            }
        }
        self.rels.retain(|r| r.points().iter().all(|p| used[*p]));
        for r in &mut self.rels {
            *r = match *r {
                Rel::Coincident(a, b) => Rel::Coincident(map[a], map[b]),
                Rel::HorizontalPoints(a, b) => Rel::HorizontalPoints(map[a], map[b]),
                Rel::VerticalPoints(a, b) => Rel::VerticalPoints(map[a], map[b]),
                Rel::Midpoint(p, e) => Rel::Midpoint(map[p], e),
                Rel::PointOn(p, e) => Rel::PointOn(map[p], e),
                Rel::Fixed(p, at) => Rel::Fixed(map[p], at),
                other => other,
            };
        }
        self.dims.retain(|d| match d.kind {
            DimKind::Distance(a, b) | DimKind::DistanceX(a, b) | DimKind::DistanceY(a, b) => {
                used[a] && used[b]
            }
            DimKind::PointLine(p, _) => used[p],
            _ => true,
        });
        for d in &mut self.dims {
            d.kind = match d.kind {
                DimKind::Distance(a, b) => DimKind::Distance(map[a], map[b]),
                DimKind::DistanceX(a, b) => DimKind::DistanceX(map[a], map[b]),
                DimKind::DistanceY(a, b) => DimKind::DistanceY(map[a], map[b]),
                DimKind::PointLine(p, e) => DimKind::PointLine(map[p], e),
                other => other,
            };
        }
    }
    /// Closed contours made of non-construction entities.
    pub fn contours(&self) -> Result<Vec<Contour>, String> {
        let mut loops: Vec<(Vec<[f64; 2]>, Vec<usize>)> = Vec::new();
        let mut adjacency: Vec<Vec<usize>> = vec![Vec::new(); self.points.len()];
        for (i, e) in self.entities.iter().enumerate() {
            if e.construction() {
                continue;
            }
            match *e {
                Entity::Line { a, b, .. } | Entity::Arc { a, b, .. } => {
                    if a == b {
                        return Err("Entité de longueur nulle.".into());
                    }
                    adjacency[a].push(i);
                    adjacency[b].push(i);
                }
                Entity::Circle { c, r, .. } => {
                    let pts: Vec<[f64; 2]> = (0..CIRCLE_STEPS)
                        .map(|k| {
                            let t = k as f64 / CIRCLE_STEPS as f64 * std::f64::consts::TAU;
                            [
                                self.points[c][0] + r * t.cos(),
                                self.points[c][1] + r * t.sin(),
                            ]
                        })
                        .collect();
                    loops.push((pts, vec![i; CIRCLE_STEPS]));
                }
            }
        }
        if let Some(p) = adjacency.iter().position(|a| !a.is_empty() && a.len() != 2) {
            return Err(if adjacency[p].len() == 1 {
                format!("Contour ouvert au point P{p}. Fermez le contour ou passez la ligne en construction.")
            } else {
                format!("Plus de deux entités se rejoignent au point P{p}.")
            });
        }
        let mut used = vec![false; self.entities.len()];
        for start in 0..self.entities.len() {
            if used[start] || self.entities[start].construction() {
                continue;
            }
            let (a0, _) = match self.entities[start] {
                Entity::Line { a, b, .. } | Entity::Arc { a, b, .. } => (a, b),
                _ => continue,
            };
            let mut pts = Vec::new();
            let mut ids = Vec::new();
            let mut entity = start;
            let mut at = a0;
            loop {
                used[entity] = true;
                let (a, b) = match self.entities[entity] {
                    Entity::Line { a, b, .. } | Entity::Arc { a, b, .. } => (a, b),
                    _ => unreachable!(),
                };
                let forward = at == a;
                let next = if forward { b } else { a };
                let seg: Vec<[f64; 2]> = match self.entities[entity] {
                    Entity::Line { .. } => vec![self.points[at]],
                    Entity::Arc { c, a, b, .. } => {
                        let mut s =
                            arc_points(self.points[c], self.points[a], self.points[b], true);
                        s.pop();
                        if !forward {
                            s.reverse();
                            s.pop();
                            s.insert(0, self.points[at]);
                        }
                        s
                    }
                    _ => unreachable!(),
                };
                for p in seg {
                    pts.push(p);
                    ids.push(entity);
                }
                at = next;
                let Some(&following) = adjacency[at].iter().find(|&&e| e != entity) else {
                    break;
                };
                if following == start {
                    break;
                }
                entity = following;
            }
            loops.push((pts, ids));
        }
        if loops.is_empty() {
            return Err("L’esquisse ne contient aucun contour fermé.".into());
        }
        for (pts, _) in &loops {
            crate::geometry::triangulate(pts).map_err(|e| format!("Contour invalide : {e}"))?;
        }
        for i in 0..loops.len() {
            for j in i + 1..loops.len() {
                if contours_cross(&loops[i].0, &loops[j].0) {
                    return Err("Deux contours se croisent. Séparez-les ou fusionnez-les en un seul contour.".into());
                }
            }
        }
        let contours = loops
            .iter()
            .enumerate()
            .map(|(i, (pts, ids))| Contour {
                points: pts.clone(),
                entity: ids.clone(),
                depth: loops
                    .iter()
                    .enumerate()
                    .filter(|(j, other)| *j != i && inside(pts[0], &other.0))
                    .count(),
            })
            .collect();
        Ok(contours)
    }
    /// Construction lines usable as revolution axes.
    pub fn axes(&self) -> Vec<usize> {
        self.entities
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                matches!(
                    e,
                    Entity::Line {
                        construction: true,
                        ..
                    }
                )
            })
            .map(|(i, _)| i)
            .collect()
    }
}
fn sub(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    [a[0] - b[0], a[1] - b[1]]
}
pub fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}
/// Signed distance from p to the infinite line ab.
pub fn line_distance(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let d = sub(b, a);
    let l = d[0].hypot(d[1]).max(1e-9);
    (d[0] * (p[1] - a[1]) - d[1] * (p[0] - a[0])) / l
}
pub fn segment_distance(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let d = sub(b, a);
    let t = (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1])
        / (d[0] * d[0] + d[1] * d[1]).max(1e-18))
    .clamp(0.0, 1.0);
    dist(p, [a[0] + d[0] * t, a[1] + d[1] * t])
}
/// Counter-clockwise arc samples from a to b (inclusive).
pub fn arc_points(c: [f64; 2], a: [f64; 2], b: [f64; 2], fine: bool) -> Vec<[f64; 2]> {
    let r = dist(c, a);
    let t0 = (a[1] - c[1]).atan2(a[0] - c[0]);
    let mut t1 = (b[1] - c[1]).atan2(b[0] - c[0]);
    while t1 <= t0 + 1e-9 {
        t1 += std::f64::consts::TAU;
    }
    let sweep = t1 - t0;
    let steps =
        ((sweep / std::f64::consts::TAU * if fine { 96.0 } else { 64.0 }).ceil() as usize).max(2);
    let mut out: Vec<[f64; 2]> = (0..=steps)
        .map(|i| {
            let t = t0 + sweep * i as f64 / steps as f64;
            [c[0] + r * t.cos(), c[1] + r * t.sin()]
        })
        .collect();
    *out.last_mut().unwrap() = b;
    out[0] = a;
    out
}
pub fn inside(p: [f64; 2], poly: &[[f64; 2]]) -> bool {
    let mut c = false;
    let n = poly.len();
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            c = !c;
        }
    }
    c
}
fn contours_cross(a: &[[f64; 2]], b: &[[f64; 2]]) -> bool {
    let cross = |o: [f64; 2], p: [f64; 2], q: [f64; 2]| {
        (p[0] - o[0]) * (q[1] - o[1]) - (p[1] - o[1]) * (q[0] - o[0])
    };
    for i in 0..a.len() {
        let (p1, p2) = (a[i], a[(i + 1) % a.len()]);
        for j in 0..b.len() {
            let (q1, q2) = (b[j], b[(j + 1) % b.len()]);
            let d1 = cross(q1, q2, p1);
            let d2 = cross(q1, q2, p2);
            let d3 = cross(p1, p2, q1);
            let d4 = cross(p1, p2, q2);
            if d1 * d2 < -1e-12 && d3 * d4 < -1e-12 {
                return true;
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    fn rectangle(s: &mut ProfileSketch, w: f64, h: f64) -> [usize; 4] {
        let p = [
            s.add_point([1.0, 1.0]),
            s.add_point([w + 3.0, 2.0]),
            s.add_point([w, h + 1.0]),
            s.add_point([-1.0, h]),
        ];
        let mut e = [0; 4];
        for i in 0..4 {
            e[i] = s.add(Entity::Line {
                a: p[i],
                b: p[(i + 1) % 4],
                construction: false,
            });
        }
        s.rels.extend([
            Rel::Horizontal(e[0]),
            Rel::Horizontal(e[2]),
            Rel::Vertical(e[1]),
            Rel::Vertical(e[3]),
        ]);
        e
    }
    #[test]
    fn rectangle_becomes_fully_defined_with_two_dimensions_and_origin() {
        let mut s = ProfileSketch::on(SketchPlane::top());
        let e = rectangle(&mut s, 40.0, 20.0);
        s.solve().unwrap();
        assert_eq!(s.status().dof, 4);
        s.rels.push(Rel::Coincident(0, 1));
        s.dims.push(Dim {
            kind: DimKind::Length(e[0]),
            value: 50.0,
            offset: [0.0; 2],
            flip: false,
        });
        s.dims.push(Dim {
            kind: DimKind::Length(e[1]),
            value: 30.0,
            offset: [0.0; 2],
            flip: false,
        });
        s.solve().unwrap();
        let st = s.status();
        assert!(st.fully_defined(), "dof {}", st.dof);
        assert!(st.defined_points.iter().all(|d| *d));
        assert!((s.measure(DimKind::Length(e[0])) - 50.0).abs() < 1e-5);
        let c = s.contours().unwrap();
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].points.len(), 4);
    }
    #[test]
    fn circle_inside_rectangle_is_a_hole_and_diameter_drives_radius() {
        let mut s = ProfileSketch::on(SketchPlane::top());
        rectangle(&mut s, 40.0, 20.0);
        let c = s.add_point([20.0, 10.0]);
        let circle = s.add(Entity::Circle {
            c,
            r: 3.0,
            construction: false,
        });
        s.dims.push(Dim {
            kind: DimKind::Diameter(circle),
            value: 8.0,
            offset: [0.0; 2],
            flip: false,
        });
        s.solve().unwrap();
        assert!((s.radius(circle) - 4.0).abs() < 1e-6);
        let mut contours = s.contours().unwrap();
        contours.sort_by_key(|c| c.depth);
        assert_eq!(
            contours.iter().map(|c| c.depth).collect::<Vec<_>>(),
            vec![0, 1]
        );
    }
    #[test]
    fn open_contour_is_reported() {
        let mut s = ProfileSketch::on(SketchPlane::top());
        let a = s.add_point([0.0, 5.0]);
        let b = s.add_point([10.0, 5.0]);
        s.add(Entity::Line {
            a,
            b,
            construction: false,
        });
        assert!(s.contours().unwrap_err().contains("ouvert"));
    }
    #[test]
    fn deleting_an_entity_renumbers_references() {
        let mut s = ProfileSketch::on(SketchPlane::top());
        let e = rectangle(&mut s, 40.0, 20.0);
        s.dims.push(Dim {
            kind: DimKind::Length(e[2]),
            value: 40.0,
            offset: [0.0; 2],
            flip: false,
        });
        s.delete_entity(e[0]);
        assert_eq!(s.entities.len(), 3);
        assert_eq!(s.dims[0].kind, DimKind::Length(1));
        assert!(s.validate().is_ok());
    }
    #[test]
    fn arc_contour_with_lines() {
        let mut s = ProfileSketch::on(SketchPlane::top());
        let a = s.add_point([10.0, 0.0]);
        let b = s.add_point([-10.0, 0.0]);
        let c = s.add_point([0.0, 0.0]);
        s.add(Entity::Arc {
            c,
            a,
            b,
            construction: false,
        });
        s.add(Entity::Line {
            a: b,
            b: a,
            construction: false,
        });
        let contours = s.contours().unwrap();
        assert_eq!(contours.len(), 1);
        let area: f64 = {
            let p = &contours[0].points;
            (0..p.len())
                .map(|i| p[i][0] * p[(i + 1) % p.len()][1] - p[(i + 1) % p.len()][0] * p[i][1])
                .sum::<f64>()
                / 2.0
        };
        assert!((area.abs() - std::f64::consts::PI * 50.0).abs() < 0.5);
    }
}
