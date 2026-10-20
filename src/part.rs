//! Feature-based part: an ordered tree of sketches and features rebuilt from the top, as in
//! SolidWorks or Creo. Every face carries the id of the feature that created it, so sketches
//! placed on faces and fillets placed on edges follow the geometry when parameters change.
use crate::{
    features::{FinishKind, HoleKind},
    geometry::{self, BooleanOp, Mesh, Polygon, Vec3},
    psketch::{Contour, ProfileSketch, SketchPlane},
    topology::{self, Topology},
};
use serde::{Deserialize, Serialize};
use std::{
    cell::RefCell,
    collections::{HashMap, HashSet},
};

pub fn tag(feature: u32, local: u32) -> u32 {
    (feature << 16) | (local & 0xffff)
}
pub fn feature_of(tag: u32) -> u32 {
    tag >> 16
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum EndCondition {
    Blind,
    ThroughAll,
    MidPlane,
}
impl EndCondition {
    pub fn label(self) -> &'static str {
        match self {
            Self::Blind => "Borgne",
            Self::ThroughAll => "À travers tout",
            Self::MidPlane => "Plan milieu",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Extrude {
    pub sketch: u32,
    pub end: EndCondition,
    pub depth: f64,
    pub reverse: bool,
    pub cut: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Revolve {
    pub sketch: u32,
    /// Construction line of the sketch used as axis; the first one when not set.
    pub axis: Option<usize>,
    pub angle: f64,
    pub reverse: bool,
    pub cut: bool,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EdgeRef {
    pub faces: [u32; 2],
    pub point: [f64; 3],
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Finish {
    pub kind: FinishKind,
    pub size: f64,
    pub edges: Vec<EdgeRef>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HoleWizard {
    pub plane: SketchPlane,
    pub points: Vec<[f64; 2]>,
    pub kind: HoleKind,
    /// Drilled diameter (tap drill for a threaded hole).
    pub diameter: f64,
    pub depth: f64,
    pub through: bool,
    pub head_diameter: f64,
    pub head_depth: f64,
    #[serde(default)]
    pub standard: String,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PatternKind {
    Linear {
        direction: [f64; 3],
        spacing: f64,
        count: usize,
    },
    Circular {
        origin: [f64; 3],
        axis: [f64; 3],
        angle: f64,
        count: usize,
    },
    Mirror {
        plane: SketchPlane,
    },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pattern {
    pub kind: PatternKind,
    pub seeds: Vec<u32>,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RefPlane {
    pub base: SketchPlane,
    pub offset: f64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum FeatureKind {
    Sketch(ProfileSketch),
    Extrude(Extrude),
    Revolve(Revolve),
    Finish(Finish),
    Hole(HoleWizard),
    Pattern(Pattern),
    Plane(RefPlane),
}
impl FeatureKind {
    pub fn prefix(&self) -> &'static str {
        match self {
            Self::Sketch(_) => "Esquisse",
            Self::Extrude(e) if e.cut => "Enlèv.-Extru.",
            Self::Extrude(_) => "Boss.-Extru.",
            Self::Revolve(r) if r.cut => "Enlèv.-Révol.",
            Self::Revolve(_) => "Boss.-Révol.",
            Self::Finish(f) if f.kind == FinishKind::Chamfer => "Chanfrein",
            Self::Finish(_) => "Congé",
            Self::Hole(_) => "Perçage",
            Self::Pattern(p) => match p.kind {
                PatternKind::Linear { .. } => "RépétL",
                PatternKind::Circular { .. } => "RépétC",
                PatternKind::Mirror { .. } => "Symétrie",
            },
            Self::Plane(_) => "Plan",
        }
    }
    pub fn sketch(&self) -> Option<u32> {
        match self {
            Self::Extrude(e) => Some(e.sketch),
            Self::Revolve(r) => Some(r.sketch),
            _ => None,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PartFeature {
    pub id: u32,
    pub name: String,
    #[serde(default)]
    pub suppressed: bool,
    pub kind: FeatureKind,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Material {
    pub name: String,
    /// g/cm³
    pub density: f64,
}
impl Default for Material {
    fn default() -> Self {
        Self {
            name: "PLA".into(),
            density: 1.24,
        }
    }
}
pub const MATERIALS: [(&str, f64); 8] = [
    ("PLA", 1.24),
    ("PETG", 1.27),
    ("ABS", 1.04),
    ("ASA", 1.07),
    ("Nylon PA12", 1.01),
    ("TPU", 1.21),
    ("Aluminium 6061", 2.70),
    ("Acier S235", 7.85),
];
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Part {
    pub features: Vec<PartFeature>,
    /// Number of features rebuilt (rollback bar); all when not set.
    #[serde(default)]
    pub rollback: Option<usize>,
    #[serde(default)]
    pub material: Material,
}

#[derive(Clone, Debug, Default)]
pub struct Regen {
    pub mesh: Mesh,
    pub errors: Vec<(u32, String)>,
    /// Resolved plane of each sketch, reference plane and hole feature.
    pub planes: HashMap<u32, SketchPlane>,
    /// Sketches absorbed by a feature.
    pub consumed: HashSet<u32>,
    tools: HashMap<u32, Vec<(Mesh, BooleanOp)>>,
}
impl Regen {
    pub fn error(&self, id: u32) -> Option<&str> {
        self.errors.iter().find(|e| e.0 == id).map(|e| e.1.as_str())
    }
}
#[derive(Clone, Default)]
struct State {
    mesh: Option<Mesh>,
    regen: Regen,
}
thread_local! {
    static CACHE: RefCell<Vec<(String, State)>> = const { RefCell::new(Vec::new()) };
}

impl Part {
    pub fn next_id(&self) -> u32 {
        self.features.iter().map(|f| f.id).max().unwrap_or(0) + 1
    }
    pub fn feature(&self, id: u32) -> Option<&PartFeature> {
        self.features.iter().find(|f| f.id == id)
    }
    pub fn feature_mut(&mut self, id: u32) -> Option<&mut PartFeature> {
        self.features.iter_mut().find(|f| f.id == id)
    }
    pub fn index(&self, id: u32) -> Option<usize> {
        self.features.iter().position(|f| f.id == id)
    }
    pub fn next_name(&self, kind: &FeatureKind) -> String {
        let prefix = kind.prefix();
        let n = self
            .features
            .iter()
            .filter_map(|f| f.name.strip_prefix(prefix)?.parse::<u32>().ok())
            .max()
            .unwrap_or(0);
        format!("{prefix}{}", n + 1)
    }
    /// Appends a feature (before the rollback bar) and returns its id.
    pub fn push(&mut self, kind: FeatureKind) -> u32 {
        let id = self.next_id();
        let name = self.next_name(&kind);
        let at = self
            .rollback
            .unwrap_or(self.features.len())
            .min(self.features.len());
        self.features.insert(
            at,
            PartFeature {
                id,
                name,
                suppressed: false,
                kind,
            },
        );
        if let Some(r) = &mut self.rollback {
            *r += 1;
        }
        id
    }
    /// Removes a feature and the sketch it absorbed when no other feature uses it.
    pub fn delete(&mut self, id: u32) {
        let sketch = self.feature(id).and_then(|f| f.kind.sketch());
        let before = self.features.len();
        self.features.retain(|f| f.id != id);
        if let Some(s) = sketch {
            if !self.features.iter().any(|f| f.kind.sketch() == Some(s)) {
                self.features.retain(|f| f.id != s);
            }
        }
        if let Some(r) = &mut self.rollback {
            *r = r.saturating_sub(before - self.features.len());
        }
        for f in &mut self.features {
            if let FeatureKind::Pattern(p) = &mut f.kind {
                p.seeds.retain(|s| *s != id);
            }
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.features.len() > 300 {
            return Err("Pièce trop complexe (300 fonctions au maximum).".into());
        }
        let mut ids = HashSet::new();
        for f in &self.features {
            if !ids.insert(f.id) || f.id == 0 || f.id > 0xffff || f.name.len() > 200 {
                return Err("Identifiants de fonctions invalides.".into());
            }
            let finite = |v: f64| v.is_finite() && v.abs() <= 1e5;
            let ok = match &f.kind {
                FeatureKind::Sketch(s) => {
                    s.validate()?;
                    true
                }
                FeatureKind::Extrude(e) => finite(e.depth) && e.depth >= 0.01,
                FeatureKind::Revolve(r) => r.angle.is_finite() && (0.1..=360.0).contains(&r.angle),
                FeatureKind::Finish(fi) => {
                    finite(fi.size) && fi.size >= 0.01 && fi.edges.len() <= 200
                }
                FeatureKind::Hole(h) => {
                    h.plane.is_valid()
                        && h.points.len() <= 200
                        && finite(h.diameter)
                        && h.diameter >= 0.1
                        && finite(h.depth)
                        && h.depth >= 0.1
                        && finite(h.head_diameter)
                        && finite(h.head_depth)
                }
                FeatureKind::Pattern(p) => match &p.kind {
                    PatternKind::Linear { spacing, count, .. } => {
                        finite(*spacing) && (2..=200).contains(count)
                    }
                    PatternKind::Circular { angle, count, .. } => {
                        finite(*angle) && (2..=200).contains(count)
                    }
                    PatternKind::Mirror { plane } => plane.is_valid(),
                },
                FeatureKind::Plane(p) => p.base.is_valid() && finite(p.offset),
            };
            if !ok {
                return Err(format!("Paramètres invalides dans « {} ».", f.name));
            }
        }
        if self.rollback.is_some_and(|r| r > self.features.len()) {
            return Err("Barre de reprise invalide.".into());
        }
        Ok(())
    }
    pub fn mesh(&self) -> Result<Mesh, String> {
        self.validate()?;
        Ok(self.regenerate().mesh)
    }
    /// Rebuilds the features up to the rollback bar. Failing features are skipped and reported,
    /// like features flagged in red in a feature tree.
    pub fn regenerate(&self) -> Regen {
        let end = self
            .rollback
            .unwrap_or(self.features.len())
            .min(self.features.len());
        let mut key = String::new();
        let mut keys = Vec::with_capacity(end);
        for f in &self.features[..end] {
            key.push_str(&serde_json::to_string(f).unwrap_or_default());
            key.push('|');
            keys.push(key.clone());
        }
        // Resume from the longest cached prefix.
        let (mut start, mut state) = (0, State::default());
        CACHE.with(|c| {
            let c = c.borrow();
            for i in (0..end).rev() {
                if let Some((_, s)) = c.iter().find(|(k, _)| *k == keys[i]) {
                    start = i + 1;
                    state = s.clone();
                    break;
                }
            }
        });
        for (i, key) in keys.iter().enumerate().take(end).skip(start) {
            let f = &self.features[i];
            if !f.suppressed {
                if let Err(e) = self.apply(f, &mut state) {
                    state.regen.errors.push((f.id, e));
                }
            }
            CACHE.with(|c| {
                let mut c = c.borrow_mut();
                c.push((key.clone(), state.clone()));
                if c.len() > 48 {
                    c.remove(0);
                }
            });
        }
        let mut regen = state.regen;
        regen.mesh = state.mesh.unwrap_or_default();
        regen
    }
    fn resolve_plane(
        &self,
        plane: &SketchPlane,
        state: &State,
        topo: &mut Option<Topology>,
    ) -> Result<SketchPlane, String> {
        let Some(face) = plane.face else {
            return Ok(*plane);
        };
        let mesh = state.mesh.as_ref().ok_or("Face de support introuvable.")?;
        let t = topo.get_or_insert_with(|| topology::analyse(mesh));
        let (n, w) = t
            .face(face)
            .and_then(|f| f.plane)
            .ok_or("La face de support a disparu ou n’est plus plane.")?;
        let o = plane.origin();
        let o = o - n * (o.dot(n) - w);
        let mut u = plane.u();
        u = (u - n * u.dot(n)).normalized();
        if u.length() < 0.5 {
            return Ok(SketchPlane::from_face(o, n, Some(face)));
        }
        let v = n.cross(u);
        let mut p = SketchPlane::new(o, u, v);
        p.face = Some(face);
        Ok(p)
    }
    fn sketch_input(
        &self,
        id: u32,
        state: &State,
    ) -> Result<(SketchPlane, &ProfileSketch, Vec<Contour>), String> {
        let f = self.feature(id).ok_or("Esquisse introuvable.")?;
        let FeatureKind::Sketch(s) = &f.kind else {
            return Err("La référence n’est pas une esquisse.".into());
        };
        if f.suppressed {
            return Err(format!("L’esquisse « {} » est désactivée.", f.name));
        }
        if let Some(e) = state.regen.error(id) {
            return Err(e.into());
        }
        let plane = *state
            .regen
            .planes
            .get(&id)
            .ok_or("L’esquisse doit précéder la fonction.")?;
        if !s.is_solved() {
            return Err(format!("L’esquisse « {} » n’est pas résolue.", f.name));
        }
        Ok((plane, s, s.contours()?))
    }
    fn size(&self, state: &State) -> f64 {
        state
            .mesh
            .as_ref()
            .and_then(|m| m.bounds())
            .map(|(a, b)| (b - a).length() + a.length().max(b.length()))
            .unwrap_or(0.0)
            * 2.0
            + 100.0
    }
    fn apply(&self, f: &PartFeature, state: &mut State) -> Result<(), String> {
        let id = f.id;
        let mut topo = None;
        match &f.kind {
            FeatureKind::Sketch(s) => {
                let plane = s.plane.ok_or("Esquisse sans plan.")?;
                let resolved = self.resolve_plane(&plane, state, &mut topo);
                state
                    .regen
                    .planes
                    .insert(id, *resolved.as_ref().unwrap_or(&plane));
                resolved?;
            }
            FeatureKind::Plane(p) => {
                let base = self.resolve_plane(&p.base, state, &mut topo)?;
                state.regen.planes.insert(id, base.offset(p.offset));
            }
            FeatureKind::Extrude(e) => {
                let (plane, _, contours) = self.sketch_input(e.sketch, state)?;
                state.regen.consumed.insert(e.sketch);
                let n = plane.normal();
                let mut dir = if e.cut { -n } else { n };
                if e.reverse {
                    dir = -dir;
                }
                // A tiny overlap with existing material keeps the booleans away from coplanar faces.
                let lap = if state.mesh.is_some() { 1e-3 } else { 0.0 };
                let (start, length) = match e.end {
                    EndCondition::Blind => (-lap, e.depth + lap),
                    EndCondition::MidPlane => (-e.depth / 2.0, e.depth),
                    EndCondition::ThroughAll => (-lap, self.size(state)),
                };
                let mut tool: Option<Mesh> = None;
                let mut sorted = contours;
                sorted.sort_by_key(|c| c.depth);
                for c in &sorted {
                    let sides: Vec<u32> = c.entity.iter().map(|e| tag(id, 2 + *e as u32)).collect();
                    let prism = geometry::extrude_tagged(
                        &c.points,
                        length,
                        [tag(id, 0), tag(id, 1)],
                        &sides,
                    )?;
                    let mirror = dir.dot(n) < 0.0;
                    let prism =
                        prism.mapped(|p| plane.world([p.x, p.y]) + dir * (p.z + start), mirror);
                    tool = Some(match tool {
                        None => prism,
                        Some(t) if c.depth % 2 == 0 => t.boolean(&prism, BooleanOp::Union)?,
                        Some(t) => t.boolean(&prism, BooleanOp::Subtract)?,
                    });
                }
                let tool = tool.ok_or("Aucun contour.")?;
                self.combine(
                    id,
                    state,
                    vec![(
                        tool,
                        if e.cut {
                            BooleanOp::Subtract
                        } else {
                            BooleanOp::Union
                        },
                    )],
                )?;
            }
            FeatureKind::Revolve(r) => {
                let (plane, sketch, contours) = self.sketch_input(r.sketch, state)?;
                state.regen.consumed.insert(r.sketch);
                let axis = r
                    .axis
                    .or_else(|| sketch.axes().first().copied())
                    .ok_or("Tracez une ligne de construction comme axe de révolution.")?;
                let crate::psketch::Entity::Line { a, b, .. } =
                    *sketch.entities.get(axis).ok_or("Axe introuvable.")?
                else {
                    return Err("L’axe doit être une ligne.".into());
                };
                let (pa, pb) = (sketch.points[a], sketch.points[b]);
                let len = crate::psketch::dist(pa, pb);
                if len < 1e-6 {
                    return Err("Axe de longueur nulle.".into());
                }
                let d = [(pb[0] - pa[0]) / len, (pb[1] - pa[1]) / len];
                let to_rz = |p: [f64; 2]| {
                    let q = [p[0] - pa[0], p[1] - pa[1]];
                    [d[0] * q[1] - d[1] * q[0], q[0] * d[0] + q[1] * d[1]]
                };
                let mut side = 0.0;
                for c in &contours {
                    for p in &c.points {
                        let rz = to_rz(*p);
                        if rz[0].abs() > 1e-6 {
                            if side * rz[0] < 0.0 {
                                return Err("Le profil traverse l’axe de révolution.".into());
                            }
                            side = rz[0].signum();
                        }
                    }
                }
                let flip = side < 0.0;
                let o = plane.world(pa);
                let axis3 = (plane.u() * d[0] + plane.v() * d[1]).normalized();
                let mut r0 = (plane.u() * -d[1] + plane.v() * d[0]).normalized();
                if flip {
                    r0 = -r0;
                }
                let mut t = axis3.cross(r0);
                if r.reverse {
                    t = -t;
                }
                let mut tool: Option<Mesh> = None;
                let mut sorted = contours;
                sorted.sort_by_key(|c| c.depth);
                for c in &sorted {
                    let pts: Vec<[f64; 2]> = c
                        .points
                        .iter()
                        .map(|p| {
                            let rz = to_rz(*p);
                            [rz[0].abs(), rz[1]]
                        })
                        .collect();
                    let solid = revolve_tagged(&pts, &c.entity, r.angle, id)?;
                    let solid = solid.mapped(|p| o + r0 * p.x + t * p.y + axis3 * p.z, r.reverse);
                    tool = Some(match tool {
                        None => solid,
                        Some(m) if c.depth % 2 == 0 => m.boolean(&solid, BooleanOp::Union)?,
                        Some(m) => m.boolean(&solid, BooleanOp::Subtract)?,
                    });
                }
                let tool = tool.ok_or("Aucun contour.")?;
                self.combine(
                    id,
                    state,
                    vec![(
                        tool,
                        if r.cut {
                            BooleanOp::Subtract
                        } else {
                            BooleanOp::Union
                        },
                    )],
                )?;
            }
            FeatureKind::Finish(fi) => {
                let mesh = state.mesh.as_ref().ok_or("Aucun corps à habiller.")?;
                if fi.edges.is_empty() {
                    return Err("Sélectionnez au moins une arête.".into());
                }
                let t = topology::analyse(mesh);
                let mut tools = Vec::new();
                for (i, r) in fi.edges.iter().enumerate() {
                    let p = Vec3::from_array(r.point);
                    let mut pair = r.faces;
                    pair.sort();
                    let edge = t
                        .edges
                        .iter()
                        .filter(|e| e.faces == pair)
                        .min_by(|a, b| a.distance_to(p).total_cmp(&b.distance_to(p)))
                        .ok_or("Une arête sélectionnée a disparu. Éditez la fonction pour la resélectionner.")?;
                    tools.push(finish_tool(
                        &t,
                        edge,
                        fi.kind,
                        fi.size,
                        tag(id, 2 + i as u32),
                        tag(id, 0),
                    )?);
                }
                self.combine(id, state, tools)?;
            }
            FeatureKind::Hole(h) => {
                let plane = self.resolve_plane(&h.plane, state, &mut topo)?;
                state.regen.planes.insert(id, plane);
                if h.points.is_empty() {
                    return Err("Placez au moins un perçage sur la face.".into());
                }
                let into = -plane.normal();
                let depth = if h.through { self.size(state) } else { h.depth };
                let mut tools = Vec::new();
                for (k, p) in h.points.iter().enumerate() {
                    let c = plane.world(*p);
                    let local =
                        hole_tool(h, depth)?.retagged(|t| tag(id, 2 + k as u32 * 16 + (t & 15)));
                    // (v, u, into) is right-handed because u × v = normal.
                    let m = local.mapped(
                        |q| c + plane.v() * q.x + plane.u() * q.y + into * q.z,
                        false,
                    );
                    tools.push((m, BooleanOp::Subtract));
                }
                self.combine(id, state, tools)?;
            }
            FeatureKind::Pattern(p) => {
                if p.seeds.is_empty() {
                    return Err("Choisissez les fonctions à répéter.".into());
                }
                let mut seeds = Vec::new();
                for s in &p.seeds {
                    let t = state
                        .regen
                        .tools
                        .get(s)
                        .ok_or("Une fonction à répéter est en erreur, désactivée ou placée après la répétition.")?;
                    seeds.extend(t.iter().cloned());
                }
                let mut out = Vec::new();
                let retag = |m: Mesh, k: usize| {
                    m.retagged(|t| tag(id, ((k as u32 & 0xff) << 8) | (t & 0xff)))
                };
                match &p.kind {
                    PatternKind::Linear {
                        direction,
                        spacing,
                        count,
                    } => {
                        let d = Vec3::from_array(*direction).normalized();
                        if d.length() < 0.5 {
                            return Err("Direction invalide.".into());
                        }
                        for k in 1..*count {
                            let shift = d * (*spacing * k as f64);
                            for (m, op) in &seeds {
                                out.push((retag(m.mapped(|q| q + shift, false), k), *op));
                            }
                        }
                    }
                    PatternKind::Circular {
                        origin,
                        axis,
                        angle,
                        count,
                    } => {
                        let a = Vec3::from_array(*axis).normalized();
                        if a.length() < 0.5 {
                            return Err("Axe invalide.".into());
                        }
                        let o = Vec3::from_array(*origin);
                        let full = (angle.abs() - 360.0).abs() < 1e-9;
                        let step = if full {
                            angle / *count as f64
                        } else {
                            angle / (*count as f64 - 1.0)
                        };
                        for k in 1..*count {
                            let th = (step * k as f64).to_radians();
                            let rot = |q: Vec3| {
                                let v = q - o;
                                o + v * th.cos()
                                    + a.cross(v) * th.sin()
                                    + a * (a.dot(v) * (1.0 - th.cos()))
                            };
                            for (m, op) in &seeds {
                                out.push((retag(m.mapped(rot, false), k), *op));
                            }
                        }
                    }
                    PatternKind::Mirror { plane } => {
                        let plane = self.resolve_plane(plane, state, &mut topo)?;
                        let (o, n) = (plane.origin(), plane.normal());
                        for (m, op) in &seeds {
                            out.push((
                                retag(m.mapped(|q| q - n * (2.0 * (q - o).dot(n)), true), 1),
                                *op,
                            ));
                        }
                    }
                }
                self.combine(id, state, out)?;
            }
        }
        Ok(())
    }
    fn combine(
        &self,
        id: u32,
        state: &mut State,
        tools: Vec<(Mesh, BooleanOp)>,
    ) -> Result<(), String> {
        let mut mesh = state.mesh.clone();
        for (tool, op) in &tools {
            mesh = Some(match (mesh, op) {
                (None, BooleanOp::Union) => tool.clone(),
                (None, _) => return Err("Il n’y a pas encore de matière à enlever.".into()),
                (Some(m), op) => m.boolean(tool, *op).map_err(|e| {
                    if e.contains("vide") {
                        "Cette fonction supprimerait toute la matière.".to_string()
                    } else {
                        e
                    }
                })?,
            });
        }
        if let (Some(before), Some(after)) = (&state.mesh, &mesh) {
            if (before.volume() - after.volume()).abs() < 1e-6 {
                return Err(if tools.iter().any(|t| t.1 == BooleanOp::Subtract) {
                    "La fonction ne coupe pas la pièce : vérifiez la direction.".into()
                } else {
                    "La fonction n’ajoute aucune matière.".into()
                });
            }
        }
        state.mesh = mesh;
        state.regen.tools.insert(id, tools);
        Ok(())
    }
}

/// Profile in the (r, z) half-plane revolved about Z. Faces tagged per sketch entity.
fn revolve_tagged(
    points: &[[f64; 2]],
    entity: &[usize],
    degrees: f64,
    id: u32,
) -> Result<Mesh, String> {
    let n = points.len();
    let area: f64 = (0..n)
        .map(|i| points[i][0] * points[(i + 1) % n][1] - points[(i + 1) % n][0] * points[i][1])
        .sum();
    // Segments with their entity, oriented counter-clockwise.
    let mut pts = points.to_vec();
    let mut seg: Vec<usize> = entity.to_vec();
    if area < 0.0 {
        pts.reverse();
        // Segment k now joins old points n-1-k and n-2-k, i.e. old segment n-2-k.
        seg = (0..n).map(|k| entity[(2 * n - 2 - k) % n]).collect();
    }
    let triangles = geometry::triangulate(&pts)?;
    let full = (degrees - 360.0).abs() < 1e-8;
    let steps = (96.0 * degrees / 360.0).ceil().max(3.0) as usize;
    let point = |i: usize, j: usize| {
        let a = degrees.to_radians() * i as f64 / steps as f64;
        Vec3::new(pts[j][0] * a.cos(), pts[j][0] * a.sin(), pts[j][1])
    };
    let mut polygons = Vec::new();
    for i in 0..steps {
        for (j, segment) in seg.iter().enumerate().take(n) {
            let k = (j + 1) % n;
            if let Some(p) = Polygon::new(vec![
                point(i, j),
                point(i + 1, j),
                point(i + 1, k),
                point(i, k),
            ]) {
                polygons.push(p.tagged(tag(id, 2 + *segment as u32)));
            }
        }
    }
    if !full {
        for t in triangles {
            if let Some(p) = Polygon::new(vec![point(0, t[0]), point(0, t[1]), point(0, t[2])]) {
                polygons.push(p.tagged(tag(id, 0)));
            }
            if let Some(p) = Polygon::new(vec![
                point(steps, t[2]),
                point(steps, t[1]),
                point(steps, t[0]),
            ]) {
                polygons.push(p.tagged(tag(id, 1)));
            }
        }
    }
    Ok(Mesh { polygons })
}

/// Cutting tool of a hole, axis +Z into the material, entry at z = 0.
fn hole_tool(h: &HoleWizard, depth: f64) -> Result<Mesh, String> {
    let r = h.diameter / 2.0;
    let mut tool = geometry::cylinder(r, depth + 1.0).transformed([0.0, 0.0, -1.0], [0.0; 3]);
    match h.kind {
        HoleKind::Counterbore => {
            if h.head_diameter <= h.diameter || h.head_depth <= 0.0 {
                return Err("Le lamage doit être plus large que le trou.".into());
            }
            let head = geometry::cylinder(h.head_diameter / 2.0, h.head_depth + 1.0)
                .transformed([0.0, 0.0, -1.0], [0.0; 3])
                .retagged(|t| t + 4);
            tool = tool.boolean(&head, BooleanOp::Union)?;
        }
        HoleKind::Countersink => {
            if h.head_diameter <= h.diameter {
                return Err("La fraisure doit être plus large que le trou.".into());
            }
            let rh = h.head_diameter / 2.0;
            let cone_depth = rh - r; // 90° countersink
            let profile = [
                [0.0, -1.0],
                [rh, -1.0],
                [rh, 0.0],
                [r, cone_depth],
                [0.0, cone_depth],
            ];
            let head = crate::features::revolve(&profile, 360.0)?.retagged(|_| 8);
            tool = tool.boolean(&head, BooleanOp::Union)?;
        }
        _ => {}
    }
    if !h.through && h.kind != HoleKind::Countersink {
        // Drill point at 118°.
        let tip = r / (59.0_f64).to_radians().tan();
        let cone = crate::features::revolve(
            &[[0.0, depth - 0.01], [r, depth - 0.01], [0.0, depth + tip]],
            360.0,
        )?
        .retagged(|_| 9);
        tool = tool.boolean(&cone, BooleanOp::Union)?;
    }
    Ok(tool)
}

/// Fillet or chamfer tool along a straight edge between two planar faces.
fn finish_tool(
    t: &Topology,
    edge: &topology::Edge,
    kind: FinishKind,
    size: f64,
    round_tag: u32,
    other_tag: u32,
) -> Result<(Mesh, BooleanOp), String> {
    let [a, b] = edge
        .line
        .ok_or("Congés et chanfreins : arêtes rectilignes uniquement dans cette version.")?;
    let n1 = t
        .face(edge.faces[0])
        .and_then(|f| f.plane)
        .ok_or("Les deux faces de l’arête doivent être planes.")?
        .0;
    let n2 = t
        .face(edge.faces[1])
        .and_then(|f| f.plane)
        .ok_or("Les deux faces de l’arête doivent être planes.")?
        .0;
    let k = n1.dot(n2);
    if !(-0.999..=0.999).contains(&k) {
        return Err("Arête entre faces tangentes : rien à arrondir.".into());
    }
    let e = (b - a).normalized();
    let (t1, t2) = (edge.inward[0], edge.inward[1]);
    let convex = t2.dot(n1) < 0.0;
    let s = if convex { 1.0 } else { -1.0 };
    // Corner at the origin of the section plane; material on the -n side when convex.
    let eps = 0.02;
    let mut section: Vec<Vec3> = Vec::new();
    let mut sides: Vec<u32> = Vec::new();
    match kind {
        FinishKind::Fillet => {
            let c = (n1 + n2) * (-s * size / (1.0 + k));
            let p1 = c + n1 * (s * size);
            let p2 = c + n2 * (s * size);
            // Arc from p2 to p1 around c, on the side of the corner.
            let d2 = (p2 - c).normalized();
            let d1 = (p1 - c).normalized();
            let angle = d2.dot(d1).clamp(-1.0, 1.0).acos();
            let axis = d2.cross(d1).normalized();
            let steps = ((angle / std::f64::consts::FRAC_PI_2) * 16.0)
                .ceil()
                .max(4.0) as usize;
            section.push(p1 + n1 * (s * eps));
            sides.push(other_tag);
            section.push((n1 + n2).normalized() * (s * eps * 2.0));
            sides.push(other_tag);
            section.push(p2 + n2 * (s * eps));
            sides.push(other_tag);
            for i in 0..=steps {
                let th = angle * i as f64 / steps as f64;
                let v = d2 * th.cos()
                    + axis.cross(d2) * th.sin()
                    + axis * (axis.dot(d2) * (1.0 - th.cos()));
                section.push(c + v * size);
                sides.push(if i < steps { round_tag } else { other_tag });
            }
        }
        FinishKind::Chamfer => {
            let p1 = t1 * size;
            let p2 = t2 * size;
            section.extend([
                p1 + n1 * (s * eps),
                (n1 + n2).normalized() * (s * eps * 2.0),
                p2 + n2 * (s * eps),
                p2,
                p1,
            ]);
            sides.extend([other_tag, other_tag, other_tag, round_tag, other_tag]);
        }
    }
    let x = n1.cross(e).normalized();
    let y = e.cross(x);
    let pts: Vec<[f64; 2]> = section.iter().map(|p| [p.dot(x), p.dot(y)]).collect();
    let len = (b - a).length() + 2.0 * eps;
    let prism = geometry::extrude_tagged(&pts, len, [other_tag, other_tag], &sides)?;
    let start = a - e * eps;
    let tool = prism.mapped(|p| start + x * p.x + y * p.y + e * p.z, false);
    Ok((
        tool,
        if convex {
            BooleanOp::Subtract
        } else {
            BooleanOp::Union
        },
    ))
}

/// Volume (mm³), area (mm²) and centre of mass of a closed mesh.
pub fn mass_properties(mesh: &Mesh) -> (f64, f64, Vec3) {
    let mut volume = 0.0;
    let mut area = 0.0;
    let mut moment = Vec3::default();
    for t in mesh.triangles() {
        let v = t[0].dot(t[1].cross(t[2])) / 6.0;
        volume += v;
        moment = moment + (t[0] + t[1] + t[2]) * (v / 4.0);
        area += (t[1] - t[0]).cross(t[2] - t[0]).length() / 2.0;
    }
    let centre = if volume.abs() > 1e-12 {
        moment / volume
    } else {
        Vec3::default()
    };
    (volume.abs(), area, centre)
}

/// ISO metric hole sizes: (name, clearance, counterbore Ø, counterbore depth, countersink Ø, tap drill).
pub const METRIC: [(&str, f64, f64, f64, f64, f64); 8] = [
    ("M2", 2.4, 4.4, 2.2, 4.4, 1.6),
    ("M2,5", 2.9, 5.5, 2.7, 5.5, 2.05),
    ("M3", 3.4, 6.5, 3.3, 6.3, 2.5),
    ("M4", 4.5, 8.0, 4.4, 8.4, 3.3),
    ("M5", 5.5, 10.0, 5.4, 10.4, 4.2),
    ("M6", 6.6, 11.0, 6.4, 12.4, 5.0),
    ("M8", 9.0, 15.0, 8.6, 16.4, 6.8),
    ("M10", 11.0, 18.0, 10.8, 20.4, 8.5),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::psketch::{Entity, Rel};
    fn rect_sketch(plane: SketchPlane, w: f64, h: f64) -> ProfileSketch {
        let mut s = ProfileSketch::on(plane);
        let p: Vec<usize> = [
            [-w / 2.0, -h / 2.0],
            [w / 2.0, -h / 2.0],
            [w / 2.0, h / 2.0],
            [-w / 2.0, h / 2.0],
        ]
        .into_iter()
        .map(|p| s.add_point(p))
        .collect();
        for i in 0..4 {
            s.add(Entity::Line {
                a: p[i],
                b: p[(i + 1) % 4],
                construction: false,
            });
        }
        s.rels.extend([
            Rel::Horizontal(0),
            Rel::Vertical(1),
            Rel::Horizontal(2),
            Rel::Vertical(3),
        ]);
        s
    }
    fn plate() -> (Part, u32) {
        let mut part = Part::default();
        let s = part.push(FeatureKind::Sketch(rect_sketch(
            SketchPlane::top(),
            80.0,
            50.0,
        )));
        let e = part.push(FeatureKind::Extrude(Extrude {
            sketch: s,
            end: EndCondition::Blind,
            depth: 10.0,
            reverse: false,
            cut: false,
        }));
        (part, e)
    }
    #[test]
    fn boss_extrude_volume_and_names() {
        let (part, _) = plate();
        let r = part.regenerate();
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        assert!((r.mesh.volume() - 80.0 * 50.0 * 10.0).abs() < 0.1);
        assert_eq!(part.features[0].name, "Esquisse1");
        assert_eq!(part.features[1].name, "Boss.-Extru.1");
    }
    #[test]
    fn sketch_on_top_face_follows_the_face_when_depth_changes() {
        let (mut part, boss) = plate();
        // Top cap of the boss: local tag 1.
        let mut plane = SketchPlane::from_face(
            Vec3::new(0.0, 0.0, 10.0),
            Vec3::new(0.0, 0.0, 1.0),
            Some(tag(boss, 1)),
        );
        plane.u = [1.0, 0.0, 0.0];
        plane.v = [0.0, 1.0, 0.0];
        let mut s = ProfileSketch::on(plane);
        let c = s.add_point([0.0, 0.0]);
        s.add(Entity::Circle {
            c,
            r: 5.0,
            construction: false,
        });
        let sk = part.push(FeatureKind::Sketch(s));
        part.push(FeatureKind::Extrude(Extrude {
            sketch: sk,
            end: EndCondition::Blind,
            depth: 4.0,
            reverse: false,
            cut: true,
        }));
        let r = part.regenerate();
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        let pocket = std::f64::consts::PI * 25.0 * 4.0;
        assert!((r.mesh.volume() - (40000.0 - pocket)).abs() < 2.0);
        if let FeatureKind::Extrude(e) = &mut part.feature_mut(boss).unwrap().kind {
            e.depth = 20.0;
        }
        let r = part.regenerate();
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        assert!((r.planes[&sk].origin[2] - 20.0).abs() < 1e-9);
        assert!((r.mesh.volume() - (80000.0 - pocket)).abs() < 2.0);
    }
    #[test]
    fn through_all_cut_and_cut_without_material() {
        let (mut part, _) = plate();
        let mut s = ProfileSketch::on(SketchPlane::top().offset(10.0));
        let c = s.add_point([10.0, 5.0]);
        s.add(Entity::Circle {
            c,
            r: 3.0,
            construction: false,
        });
        let sk = part.push(FeatureKind::Sketch(s));
        part.push(FeatureKind::Extrude(Extrude {
            sketch: sk,
            end: EndCondition::ThroughAll,
            depth: 1.0,
            reverse: false,
            cut: true,
        }));
        let r = part.regenerate();
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        assert!((r.mesh.volume() - (40000.0 - std::f64::consts::PI * 9.0 * 10.0)).abs() < 1.0);
        let mut empty = Part::default();
        let s = empty.push(FeatureKind::Sketch(rect_sketch(
            SketchPlane::top(),
            10.0,
            10.0,
        )));
        let cut = empty.push(FeatureKind::Extrude(Extrude {
            sketch: s,
            end: EndCondition::Blind,
            depth: 5.0,
            reverse: false,
            cut: true,
        }));
        assert!(empty.regenerate().error(cut).is_some());
    }
    #[test]
    fn fillet_and_chamfer_remove_expected_volume() {
        let (mut part, boss) = plate();
        let mesh = part.regenerate().mesh;
        let t = topology::analyse(&mesh);
        // Vertical edges of the plate.
        let edges: Vec<EdgeRef> = t
            .edges
            .iter()
            .filter(|e| {
                e.line
                    .is_some_and(|[a, b]| (b - a).normalized().z.abs() > 0.99)
            })
            .map(|e| EdgeRef {
                faces: e.faces,
                point: e.midpoint().array(),
            })
            .collect();
        assert_eq!(edges.len(), 4);
        let id = part.push(FeatureKind::Finish(Finish {
            kind: FinishKind::Fillet,
            size: 5.0,
            edges: edges.clone(),
        }));
        let r = part.regenerate();
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        let removed = 4.0 * (25.0 - std::f64::consts::PI * 25.0 / 4.0) * 10.0;
        assert!(
            (40000.0 - r.mesh.volume() - removed).abs() < 3.0,
            "{}",
            r.mesh.volume()
        );
        if let FeatureKind::Finish(f) = &mut part.feature_mut(id).unwrap().kind {
            f.kind = FinishKind::Chamfer;
        }
        let r = part.regenerate();
        assert!((40000.0 - r.mesh.volume() - 4.0 * 12.5 * 10.0).abs() < 3.0);
        // Edges follow a size change of the base.
        if let FeatureKind::Extrude(e) = &mut part.feature_mut(boss).unwrap().kind {
            e.depth = 20.0;
        }
        assert!(part.regenerate().errors.is_empty());
    }
    #[test]
    fn hole_wizard_and_linear_pattern() {
        let (mut part, boss) = plate();
        let mut plane = SketchPlane::new(
            Vec3::new(0.0, 0.0, 10.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        );
        plane.face = Some(tag(boss, 1));
        let hole = part.push(FeatureKind::Hole(HoleWizard {
            plane,
            points: vec![[-30.0, -15.0]],
            kind: HoleKind::Simple,
            diameter: 6.0,
            depth: 5.0,
            through: true,
            head_diameter: 10.0,
            head_depth: 3.0,
            standard: String::new(),
        }));
        part.push(FeatureKind::Pattern(Pattern {
            kind: PatternKind::Linear {
                direction: [1.0, 0.0, 0.0],
                spacing: 20.0,
                count: 4,
            },
            seeds: vec![hole],
        }));
        let r = part.regenerate();
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        let expected = 40000.0 - 4.0 * std::f64::consts::PI * 9.0 * 10.0;
        assert!(
            (r.mesh.volume() - expected).abs() < 2.0,
            "{}",
            r.mesh.volume()
        );
    }
    #[test]
    fn revolve_about_construction_axis() {
        let mut part = Part::default();
        let mut s = ProfileSketch::on(SketchPlane::front());
        let a0 = s.add_point([0.0, 0.0]);
        let a1 = s.add_point([0.0, 30.0]);
        s.add(Entity::Line {
            a: a0,
            b: a1,
            construction: true,
        });
        let q: Vec<usize> = [[10.0, 0.0], [20.0, 0.0], [20.0, 30.0], [10.0, 30.0]]
            .into_iter()
            .map(|p| s.add_point(p))
            .collect();
        for i in 0..4 {
            s.add(Entity::Line {
                a: q[i],
                b: q[(i + 1) % 4],
                construction: false,
            });
        }
        let sk = part.push(FeatureKind::Sketch(s));
        part.push(FeatureKind::Revolve(Revolve {
            sketch: sk,
            axis: None,
            angle: 360.0,
            reverse: false,
            cut: false,
        }));
        let r = part.regenerate();
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        let v = std::f64::consts::PI * (400.0 - 100.0) * 30.0;
        assert!((r.mesh.volume() - v).abs() / v < 0.01);
        let (min, max) = r.mesh.bounds().unwrap();
        assert!((max.z - 30.0).abs() < 1e-6 && min.z.abs() < 1e-6);
    }
    #[test]
    fn mirror_and_delete_with_absorbed_sketch() {
        let (mut part, boss) = plate();
        let mut s = ProfileSketch::on(SketchPlane::top().offset(10.0));
        let c = s.add_point([25.0, 0.0]);
        s.add(Entity::Circle {
            c,
            r: 5.0,
            construction: false,
        });
        let sk = part.push(FeatureKind::Sketch(s));
        let pin = part.push(FeatureKind::Extrude(Extrude {
            sketch: sk,
            end: EndCondition::Blind,
            depth: 10.0,
            reverse: false,
            cut: false,
        }));
        part.push(FeatureKind::Pattern(Pattern {
            kind: PatternKind::Mirror {
                plane: SketchPlane::right(),
            },
            seeds: vec![pin],
        }));
        let r = part.regenerate();
        assert!(r.errors.is_empty(), "{:?}", r.errors);
        let pin_v = std::f64::consts::PI * 25.0 * 10.0;
        assert!((r.mesh.volume() - (40000.0 + 2.0 * pin_v)).abs() < 2.0);
        part.delete(pin);
        assert!(part.feature(sk).is_none());
        assert!(part.feature(boss).is_some());
        let r = part.regenerate();
        assert!(!r.errors.is_empty(), "the mirror lost its seed");
    }
}
