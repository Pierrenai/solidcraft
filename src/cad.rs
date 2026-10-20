//! Part editing in the manner of SolidWorks: FeatureManager tree, PropertyManager with OK and
//! Cancel, pre-selection and selection of faces, edges and planes, and the feature commands.
use crate::{
    sketcher::{self, Frame, SketchEdit, Tool},
    viewport::{Pick, Style, Tint, Viewport},
};
use eframe::egui::{self, Color32, Pos2, RichText, Stroke, Vec2};
use solidcraft::{
    features::{FinishKind, HoleKind},
    geometry::{Mesh, Vec3},
    model::{Body, Shape},
    part::{
        self, EdgeRef, EndCondition, Extrude, FeatureKind, Finish, HoleWizard, Part, Pattern,
        PatternKind, RefPlane, Regen, Revolve,
    },
    psketch::{self, Entity, ProfileSketch, SketchPlane},
    topology::{self, Topology},
};

pub mod panels;

pub const SEL: Color32 = Color32::from_rgb(30, 130, 255);
pub const PRE: Color32 = Color32::from_rgb(255, 140, 20);
pub const PREVIEW: Color32 = Color32::from_rgb(250, 205, 40);
const MUTED: Color32 = Color32::from_rgb(95, 104, 116);
const ERROR: Color32 = Color32::from_rgb(200, 40, 40);

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PlaneRef {
    Front,
    Top,
    Right,
    Feature(u32),
}
impl PlaneRef {
    pub fn label(self, part: &Part) -> String {
        match self {
            Self::Front => "Plan de face".into(),
            Self::Top => "Plan de dessus".into(),
            Self::Right => "Plan de droite".into(),
            Self::Feature(id) => part.feature(id).map(|f| f.name.clone()).unwrap_or_default(),
        }
    }
}
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Sel {
    Face(u32),
    Edge([u32; 2], [f64; 3]),
    Plane(PlaneRef),
    Feature(u32),
    Body(usize),
}
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Cmd {
    Extrude { cut: bool },
    Revolve { cut: bool },
    Finish(FinishKind),
    Hole,
    LinearPattern,
    CircularPattern,
    Mirror,
    Plane,
}
impl Cmd {
    pub fn label(self) -> &'static str {
        match self {
            Self::Extrude { cut: false } => "Bossage extrudé",
            Self::Extrude { cut: true } => "Enlèvement de matière extrudé",
            Self::Revolve { cut: false } => "Bossage avec révolution",
            Self::Revolve { cut: true } => "Enlèvement de matière avec révolution",
            Self::Finish(FinishKind::Fillet) => "Congé",
            Self::Finish(FinishKind::Chamfer) => "Chanfrein",
            Self::Hole => "Assistance pour le perçage",
            Self::LinearPattern => "Répétition linéaire",
            Self::CircularPattern => "Répétition circulaire",
            Self::Mirror => "Symétrie",
            Self::Plane => "Plan",
        }
    }
}
pub enum Mode {
    Idle,
    PickPlane(Option<Cmd>),
    Sketch(Box<SketchEdit>, Option<Cmd>),
    Feature { id: u32, cmd: Cmd },
    Measure,
}
/// Result of the editor for the application.
pub enum Action {
    /// Replace the part (an undo step).
    Commit(Part, String),
}
pub struct PartEditor {
    pub body: Option<usize>,
    pub draft: Option<Part>,
    pub mode: Mode,
    pub sel: Vec<Sel>,
    pub hover: Option<Sel>,
    pub tree_hover: Option<u32>,
    pub regen: Regen,
    pub topo: Topology,
    shown: Option<(Part, Option<usize>)>,
    pub version: u64,
    pub message: String,
    context_bar: Option<Pos2>,
    renaming: Option<(u32, String)>,
    pub show_planes: bool,
    pub planes_size: f64,
    hole_face: bool,
    drag_depth: Option<f64>,
    pending: Vec<Action>,
    pub space_menu: bool,
    hover_handle: bool,
    pub tree_plane: Option<PlaneRef>,
    expanded: std::collections::HashSet<u32>,
}
impl Default for PartEditor {
    fn default() -> Self {
        Self {
            body: None,
            draft: None,
            mode: Mode::Idle,
            sel: Vec::new(),
            hover: None,
            tree_hover: None,
            regen: Regen::default(),
            topo: Topology::default(),
            shown: None,
            version: 1,
            message: String::new(),
            context_bar: None,
            renaming: None,
            show_planes: false,
            planes_size: 40.0,
            hole_face: false,
            drag_depth: None,
            pending: Vec::new(),
            space_menu: false,
            hover_handle: false,
            tree_plane: None,
            expanded: Default::default(),
        }
    }
}
fn part_of(body: &Body) -> Option<&Part> {
    match &body.shape {
        Shape::Part(p) => Some(p),
        _ => None,
    }
}
pub fn std_plane(r: PlaneRef) -> SketchPlane {
    match r {
        PlaneRef::Front => SketchPlane::front(),
        PlaneRef::Top => SketchPlane::top(),
        PlaneRef::Right => SketchPlane::right(),
        PlaneRef::Feature(_) => SketchPlane::top(),
    }
}

impl PartEditor {
    pub fn active(&self) -> bool {
        !matches!(self.mode, Mode::Idle | Mode::Measure)
    }
    pub fn sketching(&self) -> bool {
        matches!(self.mode, Mode::Sketch(..))
    }
    pub fn take_actions(&mut self) -> Vec<Action> {
        std::mem::take(&mut self.pending)
    }
    /// The part currently displayed: the draft while a command is open, else the saved part.
    pub fn part<'a>(&'a self, bodies: &'a [Body]) -> Option<&'a Part> {
        self.draft
            .as_ref()
            .or_else(|| self.body.and_then(|i| bodies.get(i)).and_then(part_of))
    }
    fn xf(&self, bodies: &[Body]) -> ([f64; 3], [f64; 3]) {
        self.body
            .and_then(|i| bodies.get(i))
            .map(|b| (b.position, b.rotation))
            .unwrap_or(([0.0; 3], [0.0; 3]))
    }
    fn world(&self, bodies: &[Body], p: Vec3) -> Vec3 {
        let (pos, rot) = self.xf(bodies);
        p.rotate(rot) + Vec3::from_array(pos)
    }
    fn local(&self, bodies: &[Body], p: Vec3) -> Vec3 {
        let (pos, rot) = self.xf(bodies);
        sketcher::unrotate(p - Vec3::from_array(pos), rot)
    }
    fn local_dir(&self, bodies: &[Body], v: Vec3) -> Vec3 {
        let (_, rot) = self.xf(bodies);
        sketcher::unrotate(v, rot)
    }
    /// Rebuilds the displayed part when it changed. Returns true when the mesh changed.
    pub fn sync(&mut self, bodies: &[Body]) -> bool {
        if self
            .body
            .is_none_or(|i| bodies.get(i).and_then(part_of).is_none())
        {
            self.body = bodies.iter().position(|b| part_of(b).is_some());
            self.draft = None;
            self.mode = Mode::Idle;
        }
        let Some(part) = self.part(bodies).cloned() else {
            if self.shown.is_some() {
                self.shown = None;
                self.regen = Regen::default();
                self.topo = Topology::default();
                self.version += 1;
                return true;
            }
            return false;
        };
        // While a sketch is edited, show the part as it was at that sketch (rolled back).
        let rollback = match &self.mode {
            Mode::Sketch(edit, _) => part.index(edit.feature).map(|i| i + 1),
            _ => part.rollback,
        };
        if self
            .shown
            .as_ref()
            .is_some_and(|(p, r)| *p == part && *r == rollback)
        {
            return false;
        }
        let mut view = part.clone();
        view.rollback = rollback;
        let regen = view.regenerate();
        let mesh_changed = self.shown.is_none()
            || regen.mesh.polygons.len() != self.regen.mesh.polygons.len()
            || regen.mesh.volume() != self.regen.mesh.volume();
        if mesh_changed {
            self.topo = topology::analyse(&regen.mesh);
            self.version += 1;
        }
        self.regen = regen;
        self.shown = Some((part, rollback));
        if let Some((a, b)) = self.regen.mesh.bounds() {
            let ext = (b - a).length().max(1.0);
            let reach = a.length().max(b.length());
            self.planes_size = (ext * 0.4).max(reach * 0.75).max(20.0);
        } else {
            self.planes_size = 40.0;
        }
        mesh_changed
    }
    pub fn mesh(&self) -> &Mesh {
        &self.regen.mesh
    }
    fn draft_mut(&mut self, bodies: &[Body]) -> Option<&mut Part> {
        if self.draft.is_none() {
            self.draft = self
                .body
                .and_then(|i| bodies.get(i))
                .and_then(part_of)
                .cloned();
        }
        self.draft.as_mut()
    }
    fn commit(&mut self, label: &str) {
        if let Some(d) = self.draft.take() {
            self.pending.push(Action::Commit(d, label.into()));
        }
    }
    fn plane_of(&self, r: PlaneRef) -> SketchPlane {
        match r {
            PlaneRef::Feature(id) => {
                let mut p = self
                    .regen
                    .planes
                    .get(&id)
                    .copied()
                    .unwrap_or_else(SketchPlane::top);
                p.face = None;
                p
            }
            other => std_plane(other),
        }
    }
    /// Plane through a planar face of the part, with the part origin projected on it.
    fn face_plane(&self, tag: u32) -> Option<SketchPlane> {
        let (n, w) = self.topo.face(tag)?.plane?;
        Some(SketchPlane::from_face(n * w, n, Some(tag)))
    }
    fn selected_plane(&self) -> Option<SketchPlane> {
        self.sel.iter().find_map(|s| match *s {
            Sel::Plane(r) => Some(self.plane_of(r)),
            Sel::Face(t) => self.face_plane(t),
            _ => None,
        })
    }
    pub fn cancel(&mut self) {
        if let Mode::Sketch(edit, _) = &self.mode {
            if edit.dim_edit.is_some() {
                return;
            }
        }
        self.draft = None;
        self.mode = Mode::Idle;
        self.hole_face = false;
        self.message.clear();
    }
    // ----- Commands -------------------------------------------------------------------------
    pub fn start_sketch(
        &mut self,
        bodies: &[Body],
        view: &mut Viewport,
        ctx: &egui::Context,
        then: Option<Cmd>,
    ) {
        if self.sketching() {
            self.exit_sketch(bodies, view, ctx, true);
            return;
        }
        // Edit the selected sketch, else sketch on the selected plane or face.
        if let Some(Sel::Feature(id)) = self.sel.first().copied() {
            if matches!(
                self.part(bodies)
                    .and_then(|p| p.feature(id))
                    .map(|f| &f.kind),
                Some(FeatureKind::Sketch(_))
            ) {
                self.edit_sketch(bodies, id, view, ctx);
                return;
            }
        }
        match self.selected_plane() {
            Some(plane) => self.new_sketch(bodies, plane, view, ctx, then),
            None => {
                self.mode = Mode::PickPlane(then);
                self.message = "Sélectionnez un plan ou une face plane pour l’esquisse.".into();
            }
        }
    }
    fn new_sketch(
        &mut self,
        bodies: &[Body],
        plane: SketchPlane,
        view: &mut Viewport,
        ctx: &egui::Context,
        then: Option<Cmd>,
    ) {
        self.draft = None;
        let Some(d) = self.draft_mut(bodies) else {
            return;
        };
        let sketch = ProfileSketch::on(plane);
        let id = d.push(FeatureKind::Sketch(sketch.clone()));
        let edit = SketchEdit::new(id, true, &sketch);
        self.mode = Mode::Sketch(Box::new(edit), then);
        self.sel.clear();
        self.message.clear();
        let (_, rot) = self.xf(bodies);
        view.normal_to(plane.u().rotate(rot), plane.v().rotate(rot), ctx);
    }
    pub fn edit_sketch(
        &mut self,
        bodies: &[Body],
        id: u32,
        view: &mut Viewport,
        ctx: &egui::Context,
    ) {
        self.draft = None;
        let Some(d) = self.draft_mut(bodies) else {
            return;
        };
        let Some(FeatureKind::Sketch(s)) = d.feature(id).map(|f| f.kind.clone()) else {
            return;
        };
        self.mode = Mode::Sketch(Box::new(SketchEdit::new(id, false, &s)), None);
        self.sel.clear();
        let plane = self
            .regen
            .planes
            .get(&id)
            .copied()
            .or(s.plane)
            .unwrap_or_else(SketchPlane::top);
        let (_, rot) = self.xf(bodies);
        view.normal_to(plane.u().rotate(rot), plane.v().rotate(rot), ctx);
    }
    /// Leaves the sketch, keeping (true) or discarding the changes.
    pub fn exit_sketch(
        &mut self,
        bodies: &[Body],
        view: &mut Viewport,
        ctx: &egui::Context,
        keep: bool,
    ) {
        let Mode::Sketch(edit, then) = std::mem::replace(&mut self.mode, Mode::Idle) else {
            return;
        };
        if !keep {
            self.draft = None;
            return;
        }
        let id = edit.feature;
        let empty = matches!(self.draft.as_ref().and_then(|d| d.feature(id)).map(|f| &f.kind), Some(FeatureKind::Sketch(s)) if s.entities.is_empty());
        if empty && edit.new {
            self.draft = None;
            self.message = "Esquisse vide abandonnée.".into();
            return;
        }
        let name = self
            .draft
            .as_ref()
            .and_then(|d| d.feature(id))
            .map(|f| f.name.clone())
            .unwrap_or_default();
        self.commit(&format!("Esquisse « {name} »"));
        self.sel = vec![Sel::Feature(id)];
        if let Some(cmd) = then {
            // Use the committed part as the base of the next command.
            if let Some(Action::Commit(p, _)) = self.pending.last() {
                self.draft = Some(p.clone());
            }
            self.run(cmd, bodies, view, ctx);
        }
    }
    /// Latest sketch not yet used by a feature.
    fn free_sketch(&self, part: &Part) -> Option<u32> {
        part.features
            .iter()
            .rev()
            .find(|f| {
                matches!(f.kind, FeatureKind::Sketch(_))
                    && !part.features.iter().any(|g| g.kind.sketch() == Some(f.id))
            })
            .map(|f| f.id)
    }
    pub fn run(&mut self, cmd: Cmd, bodies: &[Body], view: &mut Viewport, ctx: &egui::Context) {
        if self.sketching() {
            // Feature commands from inside a sketch close it and use it, as in SolidWorks.
            if let Mode::Sketch(_, then) = &mut self.mode {
                *then = Some(cmd);
            }
            self.exit_sketch(bodies, view, ctx, true);
            return;
        }
        if matches!(self.mode, Mode::Feature { .. }) {
            self.cancel();
        }
        if self.draft.is_none() {
            self.draft_mut(bodies);
        }
        let Some(part) = self.draft.clone() else {
            self.message = "Créez d’abord une pièce (Fichier › Nouvelle pièce).".into();
            return;
        };
        let selected_sketch = self.sel.iter().find_map(|s| match s {
            Sel::Feature(id)
                if matches!(
                    part.feature(*id).map(|f| &f.kind),
                    Some(FeatureKind::Sketch(_))
                ) =>
            {
                Some(*id)
            }
            _ => None,
        });
        let seeds: Vec<u32> = {
            let mut v: Vec<u32> = self
                .sel
                .iter()
                .filter_map(|s| match s {
                    Sel::Feature(id) => Some(*id),
                    Sel::Face(t) => Some(part::feature_of(*t)),
                    _ => None,
                })
                .filter(|id| {
                    part.feature(*id).is_some_and(|f| {
                        !matches!(f.kind, FeatureKind::Sketch(_) | FeatureKind::Plane(_))
                    })
                })
                .collect();
            v.dedup();
            v
        };
        let kind = match cmd {
            Cmd::Extrude { cut } | Cmd::Revolve { cut } => {
                let Some(sketch) = selected_sketch.or_else(|| self.free_sketch(&part)) else {
                    self.draft = None;
                    self.mode = Mode::PickPlane(Some(cmd));
                    self.message = format!(
                        "{} : sélectionnez d’abord un plan ou une face pour l’esquisse.",
                        cmd.label()
                    );
                    return;
                };
                if matches!(cmd, Cmd::Extrude { .. }) {
                    FeatureKind::Extrude(Extrude {
                        sketch,
                        end: EndCondition::Blind,
                        depth: 10.0,
                        reverse: false,
                        cut,
                    })
                } else {
                    FeatureKind::Revolve(Revolve {
                        sketch,
                        axis: None,
                        angle: 360.0,
                        reverse: false,
                        cut,
                    })
                }
            }
            Cmd::Finish(kind) => {
                let mut edges = Vec::new();
                for s in &self.sel {
                    match *s {
                        Sel::Edge(faces, point) => edges.push(EdgeRef { faces, point }),
                        Sel::Face(t) => edges.extend(self.face_edges(t)),
                        _ => {}
                    }
                }
                FeatureKind::Finish(Finish {
                    kind,
                    size: if kind == FinishKind::Fillet { 2.0 } else { 1.0 },
                    edges,
                })
            }
            Cmd::Hole => {
                let face = self.sel.iter().find_map(|s| {
                    if let Sel::Face(t) = s {
                        self.face_plane(*t)
                    } else {
                        None
                    }
                });
                self.hole_face = face.is_some();
                let (n, d, cb, cbd, _, _) = part::METRIC[4];
                FeatureKind::Hole(HoleWizard {
                    plane: face.unwrap_or_else(SketchPlane::top),
                    points: Vec::new(),
                    kind: HoleKind::Simple,
                    diameter: d,
                    depth: 10.0,
                    through: true,
                    head_diameter: cb,
                    head_depth: cbd,
                    standard: format!("{n} – passage normal"),
                })
            }
            Cmd::LinearPattern | Cmd::CircularPattern | Cmd::Mirror => {
                let seeds = if seeds.is_empty() {
                    part.features
                        .iter()
                        .rev()
                        .find(|f| !matches!(f.kind, FeatureKind::Sketch(_) | FeatureKind::Plane(_)))
                        .map(|f| vec![f.id])
                        .unwrap_or_default()
                } else {
                    seeds
                };
                let kind = match cmd {
                    Cmd::LinearPattern => PatternKind::Linear {
                        direction: self.selected_direction(bodies).unwrap_or([1.0, 0.0, 0.0]),
                        spacing: 20.0,
                        count: 3,
                    },
                    Cmd::CircularPattern => PatternKind::Circular {
                        origin: [0.0; 3],
                        axis: self.selected_direction(bodies).unwrap_or([0.0, 0.0, 1.0]),
                        angle: 360.0,
                        count: 6,
                    },
                    _ => PatternKind::Mirror {
                        plane: self.selected_plane().unwrap_or_else(SketchPlane::right),
                    },
                };
                FeatureKind::Pattern(Pattern { kind, seeds })
            }
            Cmd::Plane => FeatureKind::Plane(RefPlane {
                base: self.selected_plane().unwrap_or_else(SketchPlane::top),
                offset: 10.0,
            }),
        };
        let d = self.draft.as_mut().unwrap();
        let id = d.push(kind);
        self.mode = Mode::Feature { id, cmd };
        self.message.clear();
        self.sel
            .retain(|s| matches!(s, Sel::Edge(..)) && matches!(cmd, Cmd::Finish(_)));
    }
    pub fn edit_feature(
        &mut self,
        bodies: &[Body],
        id: u32,
        view: &mut Viewport,
        ctx: &egui::Context,
    ) {
        let Some(part) = self.body.and_then(|i| bodies.get(i)).and_then(part_of) else {
            return;
        };
        let Some(f) = part.feature(id) else {
            return;
        };
        let cmd = match &f.kind {
            FeatureKind::Sketch(_) => {
                self.edit_sketch(bodies, id, view, ctx);
                return;
            }
            FeatureKind::Extrude(e) => Cmd::Extrude { cut: e.cut },
            FeatureKind::Revolve(r) => Cmd::Revolve { cut: r.cut },
            FeatureKind::Finish(f) => Cmd::Finish(f.kind),
            FeatureKind::Hole(_) => Cmd::Hole,
            FeatureKind::Pattern(p) => match p.kind {
                PatternKind::Linear { .. } => Cmd::LinearPattern,
                PatternKind::Circular { .. } => Cmd::CircularPattern,
                PatternKind::Mirror { .. } => Cmd::Mirror,
            },
            FeatureKind::Plane(_) => Cmd::Plane,
        };
        self.draft = Some(part.clone());
        self.hole_face = true;
        self.mode = Mode::Feature { id, cmd };
        self.sel.clear();
        self.message.clear();
    }
    fn face_edges(&self, tag: u32) -> Vec<EdgeRef> {
        self.topo
            .edges
            .iter()
            .filter(|e| e.faces.contains(&tag) && e.line.is_some())
            .filter(|e| {
                e.faces
                    .iter()
                    .all(|f| self.topo.face(*f).is_some_and(|f| f.plane.is_some()))
            })
            .map(|e| EdgeRef {
                faces: e.faces,
                point: e.midpoint().array(),
            })
            .collect()
    }
    fn selected_direction(&self, bodies: &[Body]) -> Option<[f64; 3]> {
        let _ = bodies;
        self.sel.iter().find_map(|s| match *s {
            Sel::Edge(faces, p) => self
                .find_edge(faces, p)
                .and_then(|e| e.line)
                .map(|[a, b]| (b - a).normalized().array()),
            Sel::Face(t) => self
                .topo
                .face(t)
                .and_then(|f| f.plane)
                .map(|(n, _)| n.array()),
            _ => None,
        })
    }
    fn find_edge(&self, faces: [u32; 2], p: [f64; 3]) -> Option<&topology::Edge> {
        let p = Vec3::from_array(p);
        self.topo
            .edges
            .iter()
            .filter(|e| e.faces == faces)
            .min_by(|a, b| a.distance_to(p).total_cmp(&b.distance_to(p)))
    }
    pub fn ok(&mut self, bodies: &[Body], view: &mut Viewport, ctx: &egui::Context) {
        match &self.mode {
            Mode::Sketch(..) => self.exit_sketch(bodies, view, ctx, true),
            Mode::Feature { id, .. } => {
                let id = *id;
                if let Some(e) = self.regen.error(id) {
                    self.message = format!("Impossible de valider : {e}");
                    return;
                }
                let name = self
                    .draft
                    .as_ref()
                    .and_then(|d| d.feature(id))
                    .map(|f| f.name.clone())
                    .unwrap_or_default();
                self.commit(&format!("« {name} »"));
                self.mode = Mode::Idle;
                self.hole_face = false;
                self.sel = vec![Sel::Feature(id)];
            }
            Mode::PickPlane(_) | Mode::Measure => self.cancel(),
            Mode::Idle => {}
        }
    }
    pub fn delete_selected(&mut self, bodies: &[Body]) {
        let ids: Vec<u32> = self
            .sel
            .iter()
            .filter_map(|s| {
                if let Sel::Feature(id) = s {
                    Some(*id)
                } else {
                    None
                }
            })
            .collect();
        if ids.is_empty() {
            return;
        }
        self.draft = None;
        if let Some(d) = self.draft_mut(bodies) {
            for id in &ids {
                d.delete(*id);
            }
        }
        self.commit("Suppression");
        self.sel.clear();
    }
    // ----- Viewport interaction ---------------------------------------------------------------
    pub fn tints(&self) -> Vec<Tint> {
        let Some(b) = self.body else {
            return vec![];
        };
        let mut t = Vec::new();
        if let Some(Sel::Face(tag)) = self.hover {
            t.push(Tint {
                pick: Pick::Face(b, tag),
                color: PRE,
                amount: 0.45,
            });
        }
        if let Some(id) = self.tree_hover {
            t.push(Tint {
                pick: Pick::Feature(b, id),
                color: PRE,
                amount: 0.35,
            });
        }
        if let Some(Sel::Body(i)) = self.hover {
            t.push(Tint {
                pick: Pick::Body(i),
                color: PRE,
                amount: 0.3,
            });
        }
        for s in &self.sel {
            match *s {
                Sel::Face(tag) => t.push(Tint {
                    pick: Pick::Face(b, tag),
                    color: SEL,
                    amount: 0.5,
                }),
                Sel::Feature(id) => t.push(Tint {
                    pick: Pick::Feature(b, id),
                    color: SEL,
                    amount: 0.4,
                }),
                Sel::Body(i) => t.push(Tint {
                    pick: Pick::Body(i),
                    color: SEL,
                    amount: 0.35,
                }),
                _ => {}
            }
        }
        if let Mode::Feature { id, .. } = self.mode {
            t.push(Tint {
                pick: Pick::Feature(b, id),
                color: PREVIEW,
                amount: 0.55,
            });
        }
        if let Mode::Feature {
            cmd: Cmd::LinearPattern | Cmd::CircularPattern | Cmd::Mirror,
            id,
            ..
        } = self.mode
        {
            if let Some(FeatureKind::Pattern(p)) = self
                .draft
                .as_ref()
                .and_then(|d| d.feature(id))
                .map(|f| &f.kind)
            {
                for s in &p.seeds {
                    t.push(Tint {
                        pick: Pick::Feature(b, *s),
                        color: SEL,
                        amount: 0.45,
                    });
                }
            }
        }
        t
    }
    fn plane_quads(&self, bodies: &[Body]) -> Vec<(PlaneRef, [Vec3; 4], String)> {
        let mut out = Vec::new();
        let part = self.part(bodies);
        let s = self.planes_size;
        let picking = matches!(self.mode, Mode::PickPlane(_))
            || self.show_planes
            || part.is_some_and(|p| p.features.is_empty());
        let mut refs: Vec<PlaneRef> = Vec::new();
        for r in [PlaneRef::Front, PlaneRef::Top, PlaneRef::Right] {
            if picking
                || self.sel.contains(&Sel::Plane(r))
                || self.hover == Some(Sel::Plane(r))
                || self.tree_plane == Some(r)
            {
                refs.push(r);
            }
        }
        if let Some(p) = part {
            for f in &p.features {
                if let FeatureKind::Plane(_) = f.kind {
                    if !f.suppressed && self.regen.planes.contains_key(&f.id) {
                        refs.push(PlaneRef::Feature(f.id));
                    }
                }
            }
        }
        for r in refs {
            let plane = self.plane_of(r);
            let size = if matches!(r, PlaneRef::Feature(_)) {
                s * 0.8
            } else {
                s
            };
            let c = plane.origin();
            let (u, v) = (plane.u() * size, plane.v() * size);
            let quad = [c - u - v, c + u - v, c + u + v, c - u + v].map(|p| self.world(bodies, p));
            out.push((r, quad, part.map(|p| r.label(p)).unwrap_or_default()));
        }
        out
    }
    fn pick(&self, bodies: &[Body], view: &Viewport, pos: Pos2, allow_edges: bool) -> Option<Sel> {
        if allow_edges {
            let mut best: Option<(f32, Sel)> = None;
            for e in &self.topo.edges {
                for s in &e.segments {
                    let (a, b) = (self.world(bodies, s[0]), self.world(bodies, s[1]));
                    let (pa, pb) = (view.project(a), view.project(b));
                    let d = sketcher::segment_distance_2d(pos, pa, pb);
                    if d < 7.0 && best.is_none_or(|x| d < x.0) {
                        let t = ((pos - pa).dot(pb - pa) / (pb - pa).length_sq().max(1e-6))
                            .clamp(0.0, 1.0) as f64;
                        if view.visible(a + (b - a) * t) {
                            best = Some((d, Sel::Edge(e.faces, e.midpoint().array())));
                        }
                    }
                }
            }
            if let Some((_, s)) = best {
                return Some(s);
            }
        }
        if let Some(hit) = view.hit(pos) {
            return Some(if Some(hit.body) == self.body {
                Sel::Face(hit.tag)
            } else {
                Sel::Body(hit.body)
            });
        }
        for (r, q, _) in self.plane_quads(bodies) {
            let poly: Vec<Pos2> = q.iter().map(|p| view.project(*p)).collect();
            let inside = {
                let mut c = false;
                for i in 0..4 {
                    let (a, b) = (poly[i], poly[(i + 1) % 4]);
                    if (a.y > pos.y) != (b.y > pos.y)
                        && pos.x < (b.x - a.x) * (pos.y - a.y) / (b.y - a.y) + a.x
                    {
                        c = !c;
                    }
                }
                c
            };
            if inside {
                return Some(Sel::Plane(r));
            }
        }
        None
    }
    fn sketch_frame(&self, bodies: &[Body], id: u32) -> Option<Frame> {
        let part = self.part(bodies)?;
        let FeatureKind::Sketch(s) = &part.feature(id)?.kind else {
            return None;
        };
        let plane = self.regen.planes.get(&id).copied().or(s.plane)?;
        let (pos, rot) = self.xf(bodies);
        Some(Frame { plane, pos, rot })
    }
    /// Whether a primary drag should orbit the view this frame.
    pub fn left_orbit(&self, view: &Viewport) -> bool {
        match &self.mode {
            Mode::Sketch(edit, _) => {
                edit.tool == Tool::Select && edit.hover.is_none() && !edit.dragging()
            }
            Mode::Feature {
                cmd: Cmd::Extrude { .. },
                ..
            } => self.drag_depth.is_none() && !self.near_handle(view),
            _ => true,
        }
    }
    fn near_handle(&self, view: &Viewport) -> bool {
        let _ = view;
        self.hover_handle
    }
    /// Handles hover, clicks and drags in the 3D view and draws the overlays.
    pub fn viewport(
        &mut self,
        ui: &mut egui::Ui,
        bodies: &[Body],
        view: &mut Viewport,
        response: &egui::Response,
    ) {
        let ctx = ui.ctx().clone();
        let painter = ui.painter_at(view.rect);
        let pos = response.hover_pos();
        // Planes.
        let quads = self.plane_quads(bodies);
        // Sketch mode.
        if let Mode::Sketch(edit, _) = &mut self.mode {
            let id = edit.feature;
            let frame = self
                .draft
                .as_ref()
                .and_then(|d| d.feature(id))
                .and_then(|f| {
                    if let FeatureKind::Sketch(s) = &f.kind {
                        s.plane
                    } else {
                        None
                    }
                });
            let plane = self.regen.planes.get(&id).copied().or(frame);
            if let (Some(plane), Some(draft)) = (plane, self.draft.as_mut()) {
                let (pos3, rot) = self
                    .body
                    .and_then(|i| bodies.get(i))
                    .map(|b| (b.position, b.rotation))
                    .unwrap_or(([0.0; 3], [0.0; 3]));
                let frame = Frame {
                    plane,
                    pos: pos3,
                    rot,
                };
                if let Some(FeatureKind::Sketch(sketch)) =
                    draft.feature_mut(id).map(|f| &mut f.kind)
                {
                    edit.interact(sketch, &frame, view, response, ui);
                    edit.dim_popup(&ctx, sketch);
                    // Faint sketch plane grid.
                    let ext = (self.planes_size * 1.2).max(30.0);
                    let step = {
                        let raw = 40.0 / view.scale;
                        let e = raw.log10().floor();
                        let b = 10f64.powf(e);
                        if raw / b < 2.0 {
                            2.0 * b
                        } else if raw / b < 5.0 {
                            5.0 * b
                        } else {
                            10.0 * b
                        }
                    };
                    let n = ((ext / step).ceil() as i32).min(60);
                    let grid = Stroke::new(0.6, Color32::from_rgba_unmultiplied(60, 110, 200, 30));
                    for k in -n..=n {
                        let t = k as f64 * step;
                        let lim = n as f64 * step;
                        painter.line_segment(
                            [frame.screen(view, [t, -lim]), frame.screen(view, [t, lim])],
                            grid,
                        );
                        painter.line_segment(
                            [frame.screen(view, [-lim, t]), frame.screen(view, [lim, t])],
                            grid,
                        );
                    }
                    edit.paint(sketch, &frame, view, &painter, true);
                }
            }
            self.paint_other_sketches(bodies, view, &painter);
            return;
        }
        // Hover.
        let allow_edges = !matches!(
            self.mode,
            Mode::PickPlane(_) | Mode::Feature { cmd: Cmd::Hole, .. }
        );
        self.hover = if response.dragged() {
            None
        } else {
            pos.and_then(|p| self.pick(bodies, view, p, allow_edges))
        };
        if matches!(self.mode, Mode::PickPlane(_)) {
            if let Some(Sel::Face(t)) = self.hover {
                if self.topo.face(t).is_some_and(|f| f.plane.is_none()) {
                    self.hover = None;
                }
            }
        }
        // Planes.
        for (r, q, label) in &quads {
            let pts: Vec<Pos2> = q.iter().map(|p| view.project(*p)).collect();
            let state = if self.sel.contains(&Sel::Plane(*r)) {
                SEL
            } else if self.hover == Some(Sel::Plane(*r)) {
                PRE
            } else {
                Color32::from_rgb(90, 130, 190)
            };
            // Planes are drawn behind the part: filled only when nothing hides them.
            let unobstructed = self.regen.mesh.polygons.is_empty()
                || q.iter()
                    .chain(std::iter::once(&((q[0] + q[2]) * 0.5)))
                    .all(|p| view.visible(*p));
            if unobstructed {
                painter.add(egui::Shape::convex_polygon(
                    pts.clone(),
                    Color32::from_rgba_unmultiplied(state.r(), state.g(), state.b(), 22),
                    Stroke::NONE,
                ));
            }
            let mut closed: Vec<Vec3> = q.to_vec();
            closed.push(q[0]);
            view.line3(
                &painter,
                &closed,
                Stroke::new(
                    if state == Color32::from_rgb(90, 130, 190) {
                        1.0
                    } else {
                        2.0
                    },
                    state,
                ),
                false,
            );
            painter.text(
                pts[3] + Vec2::new(4.0, 4.0),
                egui::Align2::LEFT_TOP,
                label,
                egui::FontId::proportional(11.0),
                state,
            );
        }
        // Origin.
        if self.body.is_some() {
            let o = view.project(self.world(bodies, Vec3::default()));
            let blue = Color32::from_rgb(40, 80, 170);
            painter.circle_stroke(o, 4.0, Stroke::new(1.2, blue));
            painter.line_segment(
                [o + Vec2::new(-7.0, 0.0), o + Vec2::new(7.0, 0.0)],
                Stroke::new(1.0, blue),
            );
            painter.line_segment(
                [o + Vec2::new(0.0, -7.0), o + Vec2::new(0.0, 7.0)],
                Stroke::new(1.0, blue),
            );
        }
        self.paint_other_sketches(bodies, view, &painter);
        // Edges: wireframe, selection and pre-selection.
        if view.style == Style::Wireframe {
            for e in &self.topo.edges {
                for s in &e.segments {
                    painter.line_segment(
                        [
                            view.project(self.world(bodies, s[0])),
                            view.project(self.world(bodies, s[1])),
                        ],
                        Stroke::new(1.0, Color32::from_rgb(30, 30, 35)),
                    );
                }
            }
        }
        let mut edge_marks: Vec<(Sel, Color32, f32)> = self
            .sel
            .iter()
            .filter(|s| matches!(s, Sel::Edge(..)))
            .map(|s| (*s, SEL, 3.0))
            .collect();
        if let Mode::Feature {
            id,
            cmd: Cmd::Finish(_),
            ..
        } = self.mode
        {
            if let Some(FeatureKind::Finish(f)) = self
                .draft
                .as_ref()
                .and_then(|d| d.feature(id))
                .map(|f| &f.kind)
            {
                // Edges of the feature, located on the geometry before the feature.
                for r in &f.edges {
                    edge_marks.push((Sel::Edge(r.faces, r.point), SEL, 3.0));
                }
            }
        }
        if let Some(h @ Sel::Edge(..)) = self.hover {
            edge_marks.push((h, PRE, 3.5));
        }
        for (s, color, width) in edge_marks {
            if let Sel::Edge(faces, p) = s {
                let segs: Vec<[Vec3; 2]> = self
                    .find_edge(faces, p)
                    .map(|e| e.segments.clone())
                    .unwrap_or_else(|| self.edge_before(bodies, faces, p));
                for seg in segs {
                    view.line3(
                        &painter,
                        &[self.world(bodies, seg[0]), self.world(bodies, seg[1])],
                        Stroke::new(width, color),
                        true,
                    );
                }
            }
        }
        if let Some(Sel::Face(t)) = self.hover {
            if let Some(f) = self.topo.face(t) {
                if let Some(p) = pos {
                    let text = match f.plane {
                        Some(_) => format!("Face plane · {:.1} mm²", f.area),
                        None => format!("Face · {:.1} mm²", f.area),
                    };
                    let owner = self
                        .part(bodies)
                        .and_then(|pt| pt.feature(part::feature_of(t)))
                        .map(|f| f.name.clone())
                        .unwrap_or_default();
                    painter.text(
                        p + Vec2::new(16.0, 18.0),
                        egui::Align2::LEFT_TOP,
                        format!("{text}\n{owner}"),
                        egui::FontId::proportional(11.0),
                        Color32::from_rgb(70, 70, 80),
                    );
                }
            }
        }
        self.feature_overlays(ui, bodies, view, response, &painter);
        // Clicks.
        if response.clicked_by(egui::PointerButton::Primary) {
            let add = ui.input(|i| i.modifiers.ctrl || i.modifiers.shift);
            self.click(bodies, view, &ctx, add, pos.unwrap_or_default());
        }
        if response.double_clicked() {
            if let Some(Sel::Face(t)) = self.hover {
                let id = part::feature_of(t);
                if matches!(self.mode, Mode::Idle) && id != 0 {
                    self.edit_feature(bodies, id, view, &ctx);
                }
            }
        }
        if response.secondary_clicked() {
            if let Some(h) = self.hover {
                if !self.sel.contains(&h) {
                    self.sel = vec![h];
                }
            }
        }
        // Context toolbar near the last selection.
        if let Some(at) = self.context_bar {
            if pos.is_some_and(|p| p.distance(at) > 220.0)
                || !matches!(self.mode, Mode::Idle)
                || self.sel.is_empty()
            {
                self.context_bar = None;
            } else {
                self.context_toolbar(&ctx, bodies, view, at);
            }
        }
    }
    fn edge_before(&self, bodies: &[Body], faces: [u32; 2], p: [f64; 3]) -> Vec<[Vec3; 2]> {
        // The edge of a fillet being edited no longer exists in the preview: look before it.
        let Some(part) = self.part(bodies) else {
            return vec![];
        };
        let Mode::Feature { id, .. } = self.mode else {
            return vec![];
        };
        let mut before = part.clone();
        before.rollback = part.index(id);
        let t = topology::analyse(&before.regenerate().mesh);
        let p = Vec3::from_array(p);
        t.edges
            .iter()
            .filter(|e| e.faces == faces)
            .min_by(|a, b| a.distance_to(p).total_cmp(&b.distance_to(p)))
            .map(|e| e.segments.clone())
            .unwrap_or_default()
    }
    fn paint_other_sketches(&mut self, bodies: &[Body], view: &Viewport, painter: &egui::Painter) {
        let Some(part) = self.part(bodies) else {
            return;
        };
        let editing = if let Mode::Sketch(e, _) = &self.mode {
            Some(e.feature)
        } else {
            None
        };
        let visible: Vec<u32> = part
            .features
            .iter()
            .filter(|f| {
                matches!(f.kind, FeatureKind::Sketch(_)) && Some(f.id) != editing && !f.suppressed
            })
            .filter(|f| {
                !self.regen.consumed.contains(&f.id)
                    || self.sel.contains(&Sel::Feature(f.id))
                    || self.tree_hover == Some(f.id)
            })
            .filter(|f| self.regen.planes.contains_key(&f.id))
            .map(|f| f.id)
            .collect();
        for id in visible {
            let (Some(frame), Some(FeatureKind::Sketch(s))) = (
                self.sketch_frame(bodies, id),
                part.feature(id).map(|f| &f.kind),
            ) else {
                continue;
            };
            let mut ghost = SketchEdit::new(id, false, s);
            ghost.relations = false;
            ghost.paint(s, &frame, view, painter, false);
        }
    }
    fn click(
        &mut self,
        bodies: &[Body],
        view: &mut Viewport,
        ctx: &egui::Context,
        add: bool,
        pos: Pos2,
    ) {
        let hover = self.hover;
        match &mut self.mode {
            Mode::PickPlane(then) => {
                let then = *then;
                let plane = match hover {
                    Some(Sel::Plane(r)) => Some(self.plane_of(r)),
                    Some(Sel::Face(t)) => self.face_plane(t),
                    _ => None,
                };
                if let Some(p) = plane {
                    self.new_sketch(bodies, p, view, ctx, then);
                }
            }
            Mode::Feature { id, cmd, .. } => {
                let (id, cmd) = (*id, *cmd);
                self.feature_click(bodies, view, id, cmd, hover, pos);
            }
            Mode::Idle | Mode::Measure => match hover {
                Some(h) => {
                    if add {
                        if let Some(i) = self.sel.iter().position(|s| *s == h) {
                            self.sel.remove(i);
                        } else {
                            self.sel.push(h);
                        }
                    } else {
                        self.sel = vec![h];
                    }
                    if matches!(self.mode, Mode::Idle) && !matches!(h, Sel::Body(_)) {
                        self.context_bar = Some(pos);
                    }
                }
                None => {
                    if !add {
                        self.sel.clear();
                    }
                    self.context_bar = None;
                }
            },
            Mode::Sketch(..) => {}
        }
    }
    fn feature_click(
        &mut self,
        bodies: &[Body],
        view: &Viewport,
        id: u32,
        cmd: Cmd,
        hover: Option<Sel>,
        pos: Pos2,
    ) {
        let face_plane = |t: u32| self.face_plane(t);
        let hit_local = view.hit(pos).map(|h| {
            (
                self.local(bodies, h.point),
                self.local_dir(bodies, h.normal),
            )
        });
        let edge_dir = |s: &Sel| match *s {
            Sel::Edge(f, p) => self
                .find_edge(f, p)
                .and_then(|e| e.line)
                .map(|[a, b]| (b - a).normalized().array()),
            _ => None,
        };
        let new_dir = hover.as_ref().and_then(edge_dir);
        let face_edges = if let Some(Sel::Face(t)) = hover {
            self.face_edges(t)
        } else {
            vec![]
        };
        let plane_hit = match hover {
            Some(Sel::Face(t)) => face_plane(t),
            Some(Sel::Plane(r)) => Some(self.plane_of(r)),
            _ => None,
        };
        let hole_face = self.hole_face;
        let Some(d) = self.draft.as_mut() else {
            return;
        };
        let Some(f) = d.feature_mut(id) else {
            return;
        };
        match (&mut f.kind, cmd) {
            (FeatureKind::Finish(fi), _) => match hover {
                Some(Sel::Edge(faces, point)) => {
                    if let Some(i) = fi.edges.iter().position(|e| e.faces == faces) {
                        fi.edges.remove(i);
                    } else {
                        fi.edges.push(EdgeRef { faces, point });
                    }
                }
                Some(Sel::Face(_)) => {
                    for e in face_edges {
                        if !fi.edges.iter().any(|x| x.faces == e.faces) {
                            fi.edges.push(e);
                        }
                    }
                }
                _ => {}
            },
            (FeatureKind::Hole(h), _) => {
                if !hole_face {
                    if let Some(p) = plane_hit.filter(|p| p.face.is_some()) {
                        h.plane = p;
                        self.hole_face = true;
                        if let Some((pt, _)) = hit_local {
                            let q = p.local(pt);
                            h.points.push(q.map(|v| (v * 2.0).round() / 2.0));
                        }
                    }
                } else if let (Some(Sel::Face(t)), Some((pt, _))) = (hover, hit_local) {
                    if h.plane.face == Some(t) || h.plane.face.is_none() {
                        let q = h.plane.local(pt);
                        let step = if view.scale > 8.0 {
                            0.1
                        } else if view.scale > 2.0 {
                            0.5
                        } else {
                            1.0
                        };
                        h.points.push(q.map(|v| (v / step).round() * step));
                    }
                }
            }
            (FeatureKind::Pattern(p), _) => match hover {
                Some(Sel::Face(t)) => {
                    let owner = part::feature_of(t);
                    if owner != id {
                        if let Some(i) = p.seeds.iter().position(|s| *s == owner) {
                            p.seeds.remove(i);
                        } else {
                            p.seeds.push(owner);
                        }
                    }
                    if let PatternKind::Mirror { plane } = &mut p.kind {
                        if p.seeds.is_empty() {
                            if let Some(fp) = plane_hit {
                                *plane = fp;
                            }
                        }
                    }
                }
                Some(Sel::Edge(..)) => {
                    if let Some(dir) = new_dir {
                        match &mut p.kind {
                            PatternKind::Linear { direction, .. } => *direction = dir,
                            PatternKind::Circular { axis, origin, .. } => {
                                *axis = dir;
                                if let Some(Sel::Edge(fc, pt)) = hover {
                                    let _ = fc;
                                    *origin = pt;
                                }
                            }
                            _ => {}
                        }
                    }
                }
                Some(Sel::Plane(_)) => {
                    if let (PatternKind::Mirror { plane }, Some(fp)) = (&mut p.kind, plane_hit) {
                        *plane = fp;
                    }
                }
                _ => {}
            },
            (FeatureKind::Plane(rp), _) => {
                if let Some(p) = plane_hit {
                    rp.base = p;
                }
            }
            (FeatureKind::Extrude(_) | FeatureKind::Revolve(_), _) => {}
            _ => {}
        }
    }
    fn context_toolbar(
        &mut self,
        ctx: &egui::Context,
        bodies: &[Body],
        view: &mut Viewport,
        at: Pos2,
    ) {
        let first = self.sel.last().copied();
        let mut close = false;
        egui::Area::new(egui::Id::new("context-toolbar"))
            .fixed_pos(at + Vec2::new(14.0, -46.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).inner_margin(4.0).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        let plane_like = matches!(first, Some(Sel::Plane(_))) || matches!(first, Some(Sel::Face(t)) if self.topo.face(t).is_some_and(|f| f.plane.is_some()));
                        if plane_like {
                            if ui.button("✏ Esquisse").on_hover_text("Esquisse sur cette face / ce plan").clicked() {
                                self.start_sketch(bodies, view, ctx, None);
                                close = true;
                            }
                            if ui.button("Normal à").on_hover_text("Regarder perpendiculairement (Ctrl+8)").clicked() {
                                self.normal_to(bodies, view, ctx);
                                close = true;
                            }
                        }
                        if let Some(Sel::Face(t)) = first {
                            let id = part::feature_of(t);
                            if id != 0 && ui.button("⚙ Éditer la fonction").clicked() {
                                self.edit_feature(bodies, id, view, ctx);
                                close = true;
                            }
                            if plane_like && ui.button("Perçage").clicked() {
                                self.run(Cmd::Hole, bodies, view, ctx);
                                close = true;
                            }
                        }
                        if matches!(first, Some(Sel::Edge(..)) | Some(Sel::Face(_))) {
                            if ui.button("Congé").clicked() {
                                self.run(Cmd::Finish(FinishKind::Fillet), bodies, view, ctx);
                                close = true;
                            }
                            if ui.button("Chanfrein").clicked() {
                                self.run(Cmd::Finish(FinishKind::Chamfer), bodies, view, ctx);
                                close = true;
                            }
                        }
                        if ui.button("Mesurer").on_hover_text("Mesurer").clicked() {
                            self.mode = Mode::Measure;
                            close = true;
                        }
                    });
                });
            });
        if close {
            self.context_bar = None;
        }
    }
    pub fn normal_to(&mut self, bodies: &[Body], view: &mut Viewport, ctx: &egui::Context) {
        let plane = match &self.mode {
            Mode::Sketch(edit, _) => self.sketch_frame(bodies, edit.feature).map(|f| f.plane),
            _ => self.selected_plane(),
        };
        if let Some(p) = plane {
            let (_, rot) = self.xf(bodies);
            view.normal_to(p.u().rotate(rot), p.v().rotate(rot), ctx);
        } else {
            self.message = "Normal à : sélectionnez une face plane ou un plan.".into();
        }
    }
    fn feature_overlays(
        &mut self,
        ui: &egui::Ui,
        bodies: &[Body],
        view: &Viewport,
        response: &egui::Response,
        painter: &egui::Painter,
    ) {
        self.hover_handle = false;
        let Mode::Feature { id, .. } = self.mode else {
            if matches!(self.mode, Mode::Measure) {
                self.measure_overlay(bodies, view, painter);
            }
            return;
        };
        let Some(d) = self.draft.as_ref() else {
            return;
        };
        let Some(f) = d.feature(id) else {
            return;
        };
        match &f.kind {
            FeatureKind::Extrude(e) => {
                // Depth handle: an arrow from the profile centre along the extrusion.
                let Some(plane) = self.regen.planes.get(&e.sketch).copied() else {
                    return;
                };
                let Some(FeatureKind::Sketch(s)) = d.feature(e.sketch).map(|f| &f.kind) else {
                    return;
                };
                let Ok(contours) = s.contours() else {
                    return;
                };
                let pts: Vec<[f64; 2]> = contours
                    .iter()
                    .filter(|c| c.depth == 0)
                    .flat_map(|c| c.points.clone())
                    .collect();
                if pts.is_empty() {
                    return;
                }
                let c = pts
                    .iter()
                    .fold([0.0, 0.0], |a, p| [a[0] + p[0], a[1] + p[1]]);
                let c = [c[0] / pts.len() as f64, c[1] / pts.len() as f64];
                let n = plane.normal();
                let mut dir = if e.cut { -n } else { n };
                if e.reverse {
                    dir = -dir;
                }
                let depth = if e.end == EndCondition::ThroughAll {
                    self.planes_size
                } else if e.end == EndCondition::MidPlane {
                    e.depth / 2.0
                } else {
                    e.depth
                };
                let base = self.world(bodies, plane.world(c));
                let tip = self.world(bodies, plane.world(c) + dir * depth);
                let (pb, pt) = (view.project(base), view.project(tip));
                let near = response.hover_pos().is_some_and(|p| p.distance(pt) < 12.0);
                self.hover_handle = near || self.drag_depth.is_some();
                let color = if self.hover_handle {
                    PRE
                } else {
                    Color32::from_rgb(40, 100, 220)
                };
                sketcher::arrow(painter, pb, pt, Stroke::new(2.5, color));
                painter.circle_filled(pt, 5.0, color);
                painter.text(
                    pt + Vec2::new(10.0, -10.0),
                    egui::Align2::LEFT_BOTTOM,
                    format!("{} mm", psketch::fmt((e.depth * 100.0).round() / 100.0)),
                    egui::FontId::proportional(13.0),
                    color,
                );
                if e.end != EndCondition::ThroughAll {
                    if response.drag_started_by(egui::PointerButton::Primary) && near {
                        self.drag_depth = Some(e.depth);
                    }
                    if let Some(start) = self.drag_depth {
                        if response.dragged_by(egui::PointerButton::Primary) {
                            let screen_dir = (pt - pb).normalized();
                            let total = response
                                .interact_pointer_pos()
                                .map(|p| (p - pb).dot(screen_dir))
                                .unwrap_or(0.0);
                            let per_mm = (pt - pb).length() / depth.max(1e-6) as f32;
                            let mut v = (total / per_mm.max(1e-6)) as f64
                                * if e.end == EndCondition::MidPlane {
                                    2.0
                                } else {
                                    1.0
                                };
                            let step = if view.scale > 8.0 {
                                0.1
                            } else if view.scale > 2.0 {
                                0.5
                            } else {
                                1.0
                            };
                            v = ((v / step).round() * step).max(step);
                            let _ = start;
                            if let Some(FeatureKind::Extrude(e)) = self
                                .draft
                                .as_mut()
                                .and_then(|d| d.feature_mut(id))
                                .map(|f| &mut f.kind)
                            {
                                e.depth = v;
                            }
                        }
                        if response.drag_stopped() {
                            self.drag_depth = None;
                        }
                    }
                }
                let _ = ui;
            }
            FeatureKind::Hole(h) => {
                if self.hole_face {
                    let plane = self.regen.planes.get(&id).copied().unwrap_or(h.plane);
                    for (k, p) in h.points.iter().enumerate() {
                        let c = view.project(self.world(bodies, plane.world(*p)));
                        painter.circle_stroke(c, 6.0, Stroke::new(1.5, SEL));
                        painter.line_segment(
                            [c + Vec2::new(-9.0, 0.0), c + Vec2::new(9.0, 0.0)],
                            Stroke::new(1.0, SEL),
                        );
                        painter.line_segment(
                            [c + Vec2::new(0.0, -9.0), c + Vec2::new(0.0, 9.0)],
                            Stroke::new(1.0, SEL),
                        );
                        painter.text(
                            c + Vec2::new(8.0, -8.0),
                            egui::Align2::LEFT_BOTTOM,
                            format!("{}", k + 1),
                            egui::FontId::proportional(10.0),
                            SEL,
                        );
                    }
                }
            }
            FeatureKind::Pattern(p) => {
                let (o, dir) = match &p.kind {
                    PatternKind::Linear { direction, .. } => {
                        (Vec3::default(), Vec3::from_array(*direction))
                    }
                    PatternKind::Circular { origin, axis, .. } => {
                        (Vec3::from_array(*origin), Vec3::from_array(*axis))
                    }
                    PatternKind::Mirror { .. } => return,
                };
                let a = view.project(self.world(bodies, o));
                let b =
                    view.project(self.world(bodies, o + dir.normalized() * self.planes_size * 0.6));
                sketcher::arrow(
                    painter,
                    a,
                    b,
                    Stroke::new(2.0, Color32::from_rgb(200, 60, 160)),
                );
            }
            _ => {}
        }
    }
    fn measure_overlay(&self, bodies: &[Body], view: &Viewport, painter: &egui::Painter) {
        let pts: Vec<Vec3> = self.sel.iter().filter_map(|s| self.sel_point(s)).collect();
        if pts.len() == 2 {
            let (a, b) = (
                view.project(self.world(bodies, pts[0])),
                view.project(self.world(bodies, pts[1])),
            );
            sketcher::dashed(
                painter,
                &[a, b],
                Stroke::new(1.5, Color32::from_rgb(200, 60, 160)),
            );
        }
    }
    fn sel_point(&self, s: &Sel) -> Option<Vec3> {
        match *s {
            Sel::Face(t) => self.topo.face(t).map(|f| f.centroid),
            Sel::Edge(f, p) => self.find_edge(f, p).map(|e| e.midpoint()),
            _ => None,
        }
    }
    pub fn measure_text(&self) -> Vec<String> {
        let mut out = Vec::new();
        for s in &self.sel {
            match *s {
                Sel::Face(t) => {
                    if let Some(f) = self.topo.face(t) {
                        out.push(match f.plane {
                            Some((n, _)) => format!(
                                "Face plane : aire {:.2} mm², normale ({:.3}; {:.3}; {:.3})",
                                f.area, n.x, n.y, n.z
                            ),
                            None => format!("Face : aire {:.2} mm²", f.area),
                        });
                    }
                }
                Sel::Edge(f, p) => {
                    if let Some(e) = self.find_edge(f, p) {
                        out.push(match e.line {
                            Some([a, b]) => format!("Arête droite : longueur {:.3} mm\nde ({:.2}; {:.2}; {:.2}) à ({:.2}; {:.2}; {:.2})", e.length, a.x, a.y, a.z, b.x, b.y, b.z),
                            None => {
                                // Radius of a circular edge from its points.
                                let pts: Vec<Vec3> = e.segments.iter().map(|s| s[0]).collect();
                                let c = pts.iter().fold(Vec3::default(), |a, p| a + *p) / pts.len().max(1) as f64;
                                let r = pts.iter().map(|p| (*p - c).length()).sum::<f64>() / pts.len().max(1) as f64;
                                format!("Arête courbe : longueur {:.3} mm (cercle : Ø {:.3} mm)", e.length, 2.0 * r)
                            }
                        });
                    }
                }
                _ => {}
            }
        }
        if self.sel.len() == 2 {
            let planes: Vec<Option<(Vec3, f64)>> = self
                .sel
                .iter()
                .map(|s| {
                    if let Sel::Face(t) = s {
                        self.topo.face(*t).and_then(|f| f.plane)
                    } else {
                        None
                    }
                })
                .collect();
            if let (Some((n1, w1)), Some((n2, w2))) = (planes[0], planes[1]) {
                if n1.dot(n2).abs() > 0.99999 {
                    out.push(format!(
                        "Distance entre les faces parallèles : {:.3} mm",
                        (w1 - n2.dot(n1) * w2).abs()
                    ));
                } else {
                    out.push(format!(
                        "Angle entre les faces : {:.2}°",
                        n1.dot(n2).clamp(-1.0, 1.0).acos().to_degrees()
                    ));
                }
            } else if let (Some(a), Some(b)) =
                (self.sel_point(&self.sel[0]), self.sel_point(&self.sel[1]))
            {
                let d = b - a;
                out.push(format!(
                    "Distance entre centres : {:.3} mm  (dX {:.3}  dY {:.3}  dZ {:.3})",
                    d.length(),
                    d.x,
                    d.y,
                    d.z
                ));
            }
        }
        if out.is_empty() {
            out.push("Sélectionnez une ou deux faces ou arêtes.".into());
        }
        out
    }
}
