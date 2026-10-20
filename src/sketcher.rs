//! Interactive sketcher: drawing tools with snapping and inferred relations, smart dimension,
//! dragging of under-defined geometry, and display of the sketch over the 3D view.
use crate::viewport::Viewport;
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};
use solidcraft::{
    geometry::Vec3,
    psketch::{self, Dim, DimKind, Entity, ProfileSketch, Rel, SketchPlane, Status},
};

pub const SKETCH_BLUE: Color32 = Color32::from_rgb(0, 78, 220);
pub const DEFINED: Color32 = Color32::from_rgb(20, 20, 24);
pub const HOVER: Color32 = Color32::from_rgb(255, 128, 0);
pub const SELECTED: Color32 = Color32::from_rgb(0, 170, 255);
pub const DIM: Color32 = Color32::from_rgb(40, 40, 46);
const RELATION: Color32 = Color32::from_rgb(40, 140, 60);

/// Sketch plane placed in the world (through the body transform).
#[derive(Clone, Copy)]
pub struct Frame {
    pub plane: SketchPlane,
    pub pos: [f64; 3],
    pub rot: [f64; 3],
}
pub fn unrotate(p: Vec3, r: [f64; 3]) -> Vec3 {
    p.rotate([0.0, 0.0, -r[2]])
        .rotate([0.0, -r[1], 0.0])
        .rotate([-r[0], 0.0, 0.0])
}
impl Frame {
    pub fn world(&self, p: [f64; 2]) -> Vec3 {
        self.plane.world(p).rotate(self.rot) + Vec3::from_array(self.pos)
    }
    pub fn origin(&self) -> Vec3 {
        self.world([0.0, 0.0])
    }
    pub fn u(&self) -> Vec3 {
        self.plane.u().rotate(self.rot)
    }
    pub fn v(&self) -> Vec3 {
        self.plane.v().rotate(self.rot)
    }
    pub fn normal(&self) -> Vec3 {
        self.plane.normal().rotate(self.rot)
    }
    pub fn local(&self, view: &Viewport, p: Pos2) -> Option<[f64; 2]> {
        let w = view.on_plane(p, self.origin(), self.normal())?;
        let d = w - self.origin();
        Some([d.dot(self.u()), d.dot(self.v())])
    }
    pub fn screen(&self, view: &Viewport, p: [f64; 2]) -> Pos2 {
        view.project(self.world(p))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Tool {
    Select,
    Line,
    Rectangle,
    CenterRectangle,
    Circle,
    Arc,
    Slot,
    Centerline,
    Dimension,
}
impl Tool {
    pub fn label(self) -> &'static str {
        match self {
            Self::Select => "Sélection",
            Self::Line => "Ligne",
            Self::Rectangle => "Rectangle",
            Self::CenterRectangle => "Rectangle centré",
            Self::Circle => "Cercle",
            Self::Arc => "Arc centré",
            Self::Slot => "Rainure droite",
            Self::Centerline => "Ligne de construction",
            Self::Dimension => "Cotation intelligente",
        }
    }
}
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Item {
    Point(usize),
    Entity(usize),
    Dim(usize),
}
#[derive(Clone, Copy, PartialEq, Debug)]
enum Target {
    None,
    Point(usize),
    Mid(usize),
    On(usize),
}
#[derive(Clone, Debug)]
struct Snap {
    p: [f64; 2],
    target: Target,
    h: bool,
    v: bool,
    guides: Vec<[[f64; 2]; 2]>,
}
pub struct DimEdit {
    pub index: usize,
    pub text: String,
    pub at: Pos2,
    pub fresh: bool,
    pub old: Dim,
    focus: bool,
}
pub struct SketchEdit {
    pub feature: u32,
    pub new: bool,
    pub tool: Tool,
    clicks: Vec<usize>,
    chain: Option<usize>,
    chain_start: Option<usize>,
    dim_pick: Option<Item>,
    pub dim_edit: Option<DimEdit>,
    pub sel: Vec<Item>,
    pub hover: Option<Item>,
    /// Dragged item, cursor at the previous frame, sketch when the drag started.
    drag: Option<(Item, [f64; 2], ProfileSketch)>,
    pub status: Status,
    status_dirty: bool,
    pub message: String,
    pub grid: bool,
    pub relations: bool,
    pub cursor: Option<[f64; 2]>,
    snap: Option<Snap>,
    labels: Vec<(usize, Rect)>,
}
fn nice_step(raw: f64) -> f64 {
    let e = raw.max(1e-6).log10().floor();
    let base = 10f64.powf(e);
    for m in [1.0, 2.0, 5.0, 10.0] {
        if base * m >= raw {
            return base * m;
        }
    }
    base * 10.0
}
/// Evaluates a number or a small arithmetic expression ("12,5", "40/2+3").
pub fn eval(text: &str) -> Option<f64> {
    let s: Vec<char> = text
        .replace(',', ".")
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect();
    fn expr(s: &[char], i: &mut usize) -> Option<f64> {
        let mut v = term(s, i)?;
        while *i < s.len() && (s[*i] == '+' || s[*i] == '-') {
            let op = s[*i];
            *i += 1;
            let r = term(s, i)?;
            v = if op == '+' { v + r } else { v - r };
        }
        Some(v)
    }
    fn term(s: &[char], i: &mut usize) -> Option<f64> {
        let mut v = factor(s, i)?;
        while *i < s.len() && (s[*i] == '*' || s[*i] == '/') {
            let op = s[*i];
            *i += 1;
            let r = factor(s, i)?;
            v = if op == '*' { v * r } else { v / r };
        }
        Some(v)
    }
    fn factor(s: &[char], i: &mut usize) -> Option<f64> {
        if *i < s.len() && s[*i] == '-' {
            *i += 1;
            return factor(s, i).map(|v| -v);
        }
        if *i < s.len() && s[*i] == '(' {
            *i += 1;
            let v = expr(s, i)?;
            if *i < s.len() && s[*i] == ')' {
                *i += 1;
                return Some(v);
            }
            return None;
        }
        let start = *i;
        while *i < s.len() && (s[*i].is_ascii_digit() || s[*i] == '.') {
            *i += 1;
        }
        s[start..*i].iter().collect::<String>().parse().ok()
    }
    let mut i = 0;
    let v = expr(&s, &mut i)?;
    (i == s.len() && v.is_finite()).then_some(v)
}

impl SketchEdit {
    pub fn new(feature: u32, new: bool, sketch: &ProfileSketch) -> Self {
        Self {
            feature,
            new,
            tool: if new { Tool::Line } else { Tool::Select },
            clicks: Vec::new(),
            chain: None,
            chain_start: None,
            dim_pick: None,
            dim_edit: None,
            sel: Vec::new(),
            hover: None,
            drag: None,
            status: sketch.status(),
            status_dirty: false,
            message: String::new(),
            grid: true,
            relations: true,
            cursor: None,
            snap: None,
            labels: Vec::new(),
        }
    }
    pub fn set_tool(&mut self, tool: Tool) {
        self.tool = tool;
        self.reset_op();
        self.sel.clear();
        self.message.clear();
    }
    fn reset_op(&mut self) {
        self.clicks.clear();
        self.chain = None;
        self.chain_start = None;
        self.dim_pick = None;
    }
    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }
    pub fn busy(&self) -> bool {
        !self.clicks.is_empty()
            || self.chain.is_some()
            || self.dim_pick.is_some()
            || self.dim_edit.is_some()
    }
    /// Escape: finish the current operation, then the tool, then the selection.
    pub fn escape(&mut self) {
        if self.dim_edit.is_some() {
            return;
        }
        if !self.clicks.is_empty() || self.chain.is_some() || self.dim_pick.is_some() {
            self.reset_op();
        } else if self.tool != Tool::Select {
            self.tool = Tool::Select;
        } else {
            self.sel.clear();
        }
    }
    pub fn hint(&self) -> String {
        match (self.tool, self.clicks.len(), self.chain.is_some()) {
            (Tool::Select, ..) => "Cliquez pour sélectionner, glissez les points pour déformer l’esquisse, double-cliquez une cote pour la modifier.".into(),
            (Tool::Line, _, false) => "Ligne : cliquez le point de départ.".into(),
            (Tool::Line, _, true) => "Ligne : cliquez le point suivant. Double-clic ou Échap pour terminer, cliquez le premier point pour fermer.".into(),
            (Tool::Rectangle, 0, _) => "Rectangle : cliquez le premier coin.".into(),
            (Tool::Rectangle, ..) => "Rectangle : cliquez le coin opposé.".into(),
            (Tool::CenterRectangle, 0, _) => "Rectangle centré : cliquez le centre.".into(),
            (Tool::CenterRectangle, ..) => "Rectangle centré : cliquez un coin.".into(),
            (Tool::Circle, 0, _) => "Cercle : cliquez le centre.".into(),
            (Tool::Circle, ..) => "Cercle : cliquez pour fixer le rayon.".into(),
            (Tool::Arc, 0, _) => "Arc : cliquez le centre.".into(),
            (Tool::Arc, 1, _) => "Arc : cliquez le point de départ.".into(),
            (Tool::Arc, ..) => "Arc : cliquez le point d’arrivée (sens anti-horaire).".into(),
            (Tool::Slot, 0, _) => "Rainure : cliquez le premier centre.".into(),
            (Tool::Slot, 1, _) => "Rainure : cliquez le second centre.".into(),
            (Tool::Slot, ..) => "Rainure : cliquez pour fixer la largeur.".into(),
            (Tool::Centerline, 0, _) => "Ligne de construction : cliquez le départ (axe de révolution, symétrie…).".into(),
            (Tool::Centerline, ..) => "Ligne de construction : cliquez l’arrivée.".into(),
            (Tool::Dimension, ..) => {
                if self.dim_pick.is_some() {
                    "Cotation : cliquez un second élément, ou un endroit vide pour placer la cote.".into()
                } else {
                    "Cotation intelligente : cliquez une ligne, un cercle, un arc ou un point.".into()
                }
            }
        }
    }
    fn refresh(&mut self, sketch: &ProfileSketch) {
        if self.status_dirty {
            self.status = sketch.status();
            self.status_dirty = false;
        }
    }
    pub fn changed(&mut self) {
        self.status_dirty = true;
    }
    fn item_color(&self, sketch: &ProfileSketch, item: Item, defined: bool) -> Color32 {
        if self.sel.contains(&item) || self.dim_pick == Some(item) {
            SELECTED
        } else if self.hover == Some(item) {
            HOVER
        } else {
            let _ = sketch;
            if defined {
                DEFINED
            } else {
                SKETCH_BLUE
            }
        }
    }
    fn entity_defined(&self, sketch: &ProfileSketch, e: usize) -> bool {
        let ent = sketch.entities[e];
        ent.points()
            .iter()
            .all(|p| self.status.defined_points.get(*p).copied().unwrap_or(false))
            && (!matches!(ent, Entity::Circle { .. })
                || self.status.defined_radius.get(e).copied().unwrap_or(false))
    }
    fn pick(
        &self,
        sketch: &ProfileSketch,
        frame: &Frame,
        view: &Viewport,
        pos: Pos2,
    ) -> Option<Item> {
        let mut best: Option<(f32, Item)> = None;
        for (i, p) in sketch.points.iter().enumerate() {
            let d = frame.screen(view, *p).distance(pos);
            if d < 8.0 && best.is_none_or(|b| d < b.0) {
                best = Some((d, Item::Point(i)));
            }
        }
        if best.is_some() {
            return best.map(|b| b.1);
        }
        for (i, r) in &self.labels {
            if r.expand(3.0).contains(pos) {
                return Some(Item::Dim(*i));
            }
        }
        for e in 0..sketch.entities.len() {
            let pl: Vec<Pos2> = sketch
                .polyline(e)
                .iter()
                .map(|p| frame.screen(view, *p))
                .collect();
            let d = pl
                .windows(2)
                .map(|w| segment_distance_2d(pos, w[0], w[1]))
                .fold(f32::INFINITY, f32::min);
            if d < 6.0 && best.is_none_or(|b| d < b.0) {
                best = Some((d, Item::Entity(e)));
            }
        }
        best.map(|b| b.1)
    }
    fn snap(
        &self,
        sketch: &ProfileSketch,
        frame: &Frame,
        view: &Viewport,
        pos: Pos2,
        q: [f64; 2],
        anchor: Option<usize>,
    ) -> Snap {
        let ups = 1.0 / view.scale;
        let tol = 8.0 * ups;
        let mut s = Snap {
            p: q,
            target: Target::None,
            h: false,
            v: false,
            guides: Vec::new(),
        };
        let dragged = match self.drag {
            Some((Item::Point(i), ..)) => Some(i),
            _ => None,
        };
        // Existing points first, then midpoints, then curves.
        let mut best = f32::INFINITY;
        for (i, p) in sketch.points.iter().enumerate() {
            if Some(i) == dragged {
                continue;
            }
            let d = frame.screen(view, *p).distance(pos);
            if d < 9.0 && d < best {
                best = d;
                s.p = *p;
                s.target = Target::Point(i);
            }
        }
        if s.target != Target::None {
            return s;
        }
        for (e, ent) in sketch.entities.iter().enumerate() {
            if let Entity::Line { a, b, .. } = *ent {
                let m = [
                    (sketch.points[a][0] + sketch.points[b][0]) / 2.0,
                    (sketch.points[a][1] + sketch.points[b][1]) / 2.0,
                ];
                if frame.screen(view, m).distance(pos) < 8.0 {
                    s.p = m;
                    s.target = Target::Mid(e);
                    return s;
                }
            }
        }
        for (e, ent) in sketch.entities.iter().enumerate() {
            if sketch.distance_to(e, q) * view.scale < 5.0 {
                let p = match *ent {
                    Entity::Line { a, b, .. } => {
                        let (pa, pb) = (sketch.points[a], sketch.points[b]);
                        let d = [pb[0] - pa[0], pb[1] - pa[1]];
                        let t = ((q[0] - pa[0]) * d[0] + (q[1] - pa[1]) * d[1])
                            / (d[0] * d[0] + d[1] * d[1]).max(1e-12);
                        [pa[0] + d[0] * t, pa[1] + d[1] * t]
                    }
                    Entity::Circle { c, .. } | Entity::Arc { c, .. } => {
                        let pc = sketch.points[c];
                        let r = sketch.radius(e);
                        let l = psketch::dist(q, pc).max(1e-9);
                        [
                            pc[0] + (q[0] - pc[0]) * r / l,
                            pc[1] + (q[1] - pc[1]) * r / l,
                        ]
                    }
                };
                s.p = p;
                s.target = Target::On(e);
                return s;
            }
        }
        if self.grid {
            let step = nice_step(10.0 * ups);
            s.p = [(q[0] / step).round() * step, (q[1] / step).round() * step];
        }
        if let Some(a) = anchor {
            let pa = sketch.points[a];
            if (q[1] - pa[1]).abs() < tol {
                s.p[1] = pa[1];
                s.h = true;
            } else if (q[0] - pa[0]).abs() < tol {
                s.p[0] = pa[0];
                s.v = true;
            }
        }
        // Alignment guides with other points (snap only, no relation).
        for (i, p) in sketch.points.iter().enumerate() {
            if Some(i) == anchor || Some(i) == dragged {
                continue;
            }
            if !s.v && (q[0] - p[0]).abs() < tol * 0.75 {
                s.p[0] = p[0];
                s.guides.push([*p, s.p]);
            }
            if !s.h && (q[1] - p[1]).abs() < tol * 0.75 {
                s.p[1] = p[1];
                s.guides.push([*p, s.p]);
            }
        }
        s
    }
    fn resolve(&self, sketch: &mut ProfileSketch, s: &Snap) -> usize {
        match s.target {
            Target::Point(i) => i,
            Target::Mid(e) => {
                let i = sketch.add_point(s.p);
                sketch.rels.push(Rel::Midpoint(i, e));
                i
            }
            Target::On(e) => {
                let i = sketch.add_point(s.p);
                sketch.rels.push(Rel::PointOn(i, e));
                i
            }
            Target::None => sketch.add_point(s.p),
        }
    }
    fn anchor(&self) -> Option<usize> {
        match self.tool {
            Tool::Line => self.chain,
            Tool::Centerline => self.clicks.first().copied(),
            _ => None,
        }
    }
    /// Handles mouse and keyboard input in the 3D view. Returns true when the sketch changed.
    pub fn interact(
        &mut self,
        sketch: &mut ProfileSketch,
        frame: &Frame,
        view: &Viewport,
        response: &egui::Response,
        ui: &egui::Ui,
    ) -> bool {
        self.refresh(sketch);
        let mut changed = false;
        let pos = response
            .hover_pos()
            .or_else(|| response.interact_pointer_pos());
        self.cursor = pos.and_then(|p| frame.local(view, p));
        self.hover = None;
        self.snap = None;
        if self.dim_edit.is_some() {
            return false;
        }
        if let (Some(p), Some(q)) = (pos, self.cursor) {
            if matches!(self.tool, Tool::Select | Tool::Dimension) && self.drag.is_none() {
                self.hover = self.pick(sketch, frame, view, p);
            }
            if !matches!(self.tool, Tool::Select | Tool::Dimension) {
                self.snap = Some(self.snap(sketch, frame, view, p, q, self.anchor()));
            }
        }
        // Dragging under-defined geometry, with any tool: a press-and-drag on a point or a curve
        // moves it, while a simple click keeps drawing. Fully defined geometry does not move.
        if response.drag_started_by(egui::PointerButton::Primary) {
            let grab = match (self.hover, pos) {
                (Some(h), _) => Some(h),
                (None, Some(p)) => {
                    // Use the press position: the pointer has already moved a few pixels.
                    let press = ui.input(|i| i.pointer.press_origin()).unwrap_or(p);
                    self.pick(sketch, frame, view, press)
                }
                _ => None,
            };
            if let (Some(item), Some(q)) = (grab, self.cursor) {
                if !matches!(item, Item::Point(0)) {
                    self.drag = Some((item, q, sketch.clone()));
                    self.reset_op();
                    self.message.clear();
                }
            }
        }
        if let (Some((item, last, base)), Some(q)) = (self.drag.clone(), self.cursor) {
            if response.dragged_by(egui::PointerButton::Primary) {
                let moved = drag_step(sketch, &base, item, last, q);
                if let Some(next) = moved {
                    if next != *sketch {
                        *sketch = next;
                        changed = true;
                    }
                    if let Some(d) = &mut self.drag {
                        d.1 = q;
                    }
                } else if !matches!(item, Item::Dim(_)) {
                    self.message = "Cet élément est totalement défini par ses cotes et relations : il ne peut pas bouger.".into();
                }
            }
        }
        if response.drag_stopped() && self.drag.take().is_some() {
            self.status_dirty = true;
        }
        if self.drag.is_some() {
            self.hover = self.drag.as_ref().map(|d| d.0);
        }
        let double = response.double_clicked();
        if response.clicked_by(egui::PointerButton::Primary)
            && !(double && self.tool == Tool::Select)
        {
            changed |= self.click(
                sketch,
                frame,
                view,
                ui.input(|i| i.modifiers.ctrl || i.modifiers.shift),
                pos.unwrap_or_default(),
            );
        }
        if double {
            match (self.tool, self.hover) {
                (Tool::Line, _) => {
                    self.chain = None;
                    self.chain_start = None;
                }
                (Tool::Select, Some(Item::Dim(i))) => {
                    self.open_dim(sketch, i, false, response.hover_pos().unwrap_or_default())
                }
                _ => {}
            }
        }
        if response.clicked_by(egui::PointerButton::Secondary) && self.busy() {
            self.reset_op();
        }
        if !ui.ctx().wants_keyboard_input() {
            let keys = ui.input(|i| {
                (
                    i.key_pressed(egui::Key::Delete) || i.key_pressed(egui::Key::Backspace),
                    i.key_pressed(egui::Key::L) && !i.modifiers.command,
                    i.key_pressed(egui::Key::R) && !i.modifiers.command,
                    i.key_pressed(egui::Key::C) && !i.modifiers.command,
                    i.key_pressed(egui::Key::D) && !i.modifiers.command,
                    i.key_pressed(egui::Key::A) && !i.modifiers.command,
                )
            });
            if keys.0 {
                changed |= self.delete_selection(sketch);
            }
            for (pressed, tool) in [
                (keys.1, Tool::Line),
                (keys.2, Tool::Rectangle),
                (keys.3, Tool::Circle),
                (keys.4, Tool::Dimension),
                (keys.5, Tool::Arc),
            ] {
                if pressed {
                    self.set_tool(tool);
                }
            }
        }
        if changed {
            self.status_dirty = true;
        }
        self.refresh(sketch);
        changed
    }
    fn click(
        &mut self,
        sketch: &mut ProfileSketch,
        frame: &Frame,
        view: &Viewport,
        add: bool,
        pos: Pos2,
    ) -> bool {
        let Some(q) = self.cursor else {
            self.message = "Vue presque parallèle au plan : utilisez Normal à (Ctrl+8).".into();
            return false;
        };
        match self.tool {
            Tool::Select => {
                match self.hover {
                    Some(item) => {
                        if add {
                            if let Some(i) = self.sel.iter().position(|s| *s == item) {
                                self.sel.remove(i);
                            } else {
                                self.sel.push(item);
                            }
                        } else {
                            self.sel = vec![item];
                        }
                    }
                    None => {
                        if !add {
                            self.sel.clear();
                        }
                    }
                }
                false
            }
            Tool::Dimension => self.dimension_click(sketch, frame, view, q, pos),
            _ => {
                let Some(s) = self.snap.clone() else {
                    return false;
                };
                self.draw_click(sketch, s)
            }
        }
    }
    fn draw_click(&mut self, sketch: &mut ProfileSketch, s: Snap) -> bool {
        let line = |sketch: &mut ProfileSketch, a: usize, b: usize, construction: bool| {
            sketch.add(Entity::Line { a, b, construction })
        };
        match self.tool {
            Tool::Line | Tool::Centerline => {
                let construction = self.tool == Tool::Centerline;
                let start = if construction {
                    self.clicks.first().copied()
                } else {
                    self.chain
                };
                let Some(a) = start else {
                    let i = self.resolve(sketch, &s);
                    if construction {
                        self.clicks.push(i);
                    } else {
                        self.chain = Some(i);
                        self.chain_start = Some(i);
                    }
                    return true;
                };
                if psketch::dist(sketch.points[a], s.p) < 1e-6 {
                    return false;
                }
                let b = self.resolve(sketch, &s);
                let e = line(sketch, a, b, construction);
                if s.h {
                    sketch.rels.push(Rel::Horizontal(e));
                } else if s.v {
                    sketch.rels.push(Rel::Vertical(e));
                }
                if construction {
                    self.clicks.clear();
                } else if Some(b) == self.chain_start
                    || matches!(s.target, Target::Point(_))
                        && Some(b) != self.chain_start
                        && self.is_closed(sketch, b)
                {
                    self.chain = None;
                    self.chain_start = None;
                } else {
                    self.chain = Some(b);
                }
            }
            Tool::Rectangle | Tool::CenterRectangle => {
                let Some(&first) = self.clicks.first() else {
                    let i = self.resolve(sketch, &s);
                    self.clicks.push(i);
                    return true;
                };
                let f = sketch.points[first];
                if (s.p[0] - f[0]).abs() < 1e-6 || (s.p[1] - f[1]).abs() < 1e-6 {
                    self.message = "Rectangle de largeur ou de hauteur nulle.".into();
                    return false;
                }
                let corners: [usize; 4] = if self.tool == Tool::Rectangle {
                    let c = self.resolve(sketch, &s);
                    let b = sketch.add_point([s.p[0], f[1]]);
                    let d = sketch.add_point([f[0], s.p[1]]);
                    [first, b, c, d]
                } else {
                    let (dx, dy) = (s.p[0] - f[0], s.p[1] - f[1]);
                    let c = self.resolve(sketch, &s);
                    let a = sketch.add_point([f[0] - dx, f[1] - dy]);
                    let b = sketch.add_point([f[0] + dx, f[1] - dy]);
                    let d = sketch.add_point([f[0] - dx, f[1] + dy]);
                    [a, b, c, d]
                };
                let mut e = [0; 4];
                for k in 0..4 {
                    e[k] = line(sketch, corners[k], corners[(k + 1) % 4], false);
                }
                let (p0, p1) = (sketch.points[corners[0]], sketch.points[corners[1]]);
                let first_horizontal = (p1[1] - p0[1]).abs() < (p1[0] - p0[0]).abs();
                for (k, entity) in e.iter().enumerate() {
                    sketch.rels.push(if (k % 2 == 0) == first_horizontal {
                        Rel::Horizontal(*entity)
                    } else {
                        Rel::Vertical(*entity)
                    });
                }
                if self.tool == Tool::CenterRectangle {
                    let d1 = line(sketch, corners[0], corners[2], true);
                    let d2 = line(sketch, corners[1], corners[3], true);
                    sketch.rels.push(Rel::Midpoint(first, d1));
                    sketch.rels.push(Rel::Midpoint(first, d2));
                }
                self.clicks.clear();
            }
            Tool::Circle => {
                let Some(&c) = self.clicks.first() else {
                    let i = self.resolve(sketch, &s);
                    self.clicks.push(i);
                    return true;
                };
                let r = psketch::dist(sketch.points[c], s.p);
                if r < 1e-4 {
                    return false;
                }
                let e = sketch.add(Entity::Circle {
                    c,
                    r,
                    construction: false,
                });
                if let Target::Point(p) = s.target {
                    sketch.rels.push(Rel::PointOn(p, e));
                }
                self.clicks.clear();
            }
            Tool::Arc => match self.clicks.len() {
                0 | 1 => {
                    if self.clicks.len() == 1
                        && psketch::dist(sketch.points[self.clicks[0]], s.p) < 1e-4
                    {
                        return false;
                    }
                    let i = self.resolve(sketch, &s);
                    self.clicks.push(i);
                }
                _ => {
                    let (c, a) = (self.clicks[0], self.clicks[1]);
                    let pc = sketch.points[c];
                    let r = psketch::dist(pc, sketch.points[a]);
                    let l = psketch::dist(pc, s.p);
                    if l < 1e-6 {
                        return false;
                    }
                    let b = match s.target {
                        Target::Point(i) => i,
                        _ => sketch.add_point([
                            pc[0] + (s.p[0] - pc[0]) * r / l,
                            pc[1] + (s.p[1] - pc[1]) * r / l,
                        ]),
                    };
                    if b == a {
                        return false;
                    }
                    sketch.add(Entity::Arc {
                        c,
                        a,
                        b,
                        construction: false,
                    });
                    self.clicks.clear();
                }
            },
            Tool::Slot => match self.clicks.len() {
                0 | 1 => {
                    if self.clicks.len() == 1
                        && psketch::dist(sketch.points[self.clicks[0]], s.p) < 1e-4
                    {
                        return false;
                    }
                    let i = self.resolve(sketch, &s);
                    self.clicks.push(i);
                }
                _ => {
                    let (c1, c2) = (self.clicks[0], self.clicks[1]);
                    let (p1, p2) = (sketch.points[c1], sketch.points[c2]);
                    let w = psketch::line_distance(s.p, p1, p2).abs();
                    if w < 1e-4 {
                        return false;
                    }
                    let l = psketch::dist(p1, p2);
                    let d = [(p2[0] - p1[0]) / l, (p2[1] - p1[1]) / l];
                    let n = [-d[1] * w, d[0] * w];
                    let a1 = sketch.add_point([p1[0] + n[0], p1[1] + n[1]]);
                    let b1 = sketch.add_point([p1[0] - n[0], p1[1] - n[1]]);
                    let a2 = sketch.add_point([p2[0] + n[0], p2[1] + n[1]]);
                    let b2 = sketch.add_point([p2[0] - n[0], p2[1] - n[1]]);
                    let top = line(sketch, a1, a2, false);
                    let bottom = line(sketch, b2, b1, false);
                    let arc2 = sketch.add(Entity::Arc {
                        c: c2,
                        a: b2,
                        b: a2,
                        construction: false,
                    });
                    let arc1 = sketch.add(Entity::Arc {
                        c: c1,
                        a: a1,
                        b: b1,
                        construction: false,
                    });
                    line(sketch, c1, c2, true);
                    sketch.rels.extend([
                        Rel::Tangent(top, arc1),
                        Rel::Tangent(top, arc2),
                        Rel::Tangent(bottom, arc1),
                        Rel::Tangent(bottom, arc2),
                        Rel::Equal(arc1, arc2),
                    ]);
                    self.clicks.clear();
                }
            },
            Tool::Select | Tool::Dimension => return false,
        }
        let _ = sketch.solve();
        true
    }
    fn is_closed(&self, sketch: &ProfileSketch, p: usize) -> bool {
        sketch
            .entities
            .iter()
            .filter(|e| {
                !e.construction() && e.points().contains(&p) && !matches!(e, Entity::Circle { .. })
            })
            .count()
            >= 2
    }
    fn dimension_click(
        &mut self,
        sketch: &mut ProfileSketch,
        frame: &Frame,
        view: &Viewport,
        q: [f64; 2],
        pos: Pos2,
    ) -> bool {
        let hovered = self.hover.filter(|h| !matches!(h, Item::Dim(_)));
        if let Some(Item::Dim(i)) = self.hover {
            self.open_dim(sketch, i, false, pos);
            return false;
        }
        let Some(first) = self.dim_pick else {
            if let Some(h) = hovered {
                self.dim_pick = Some(h);
            }
            return false;
        };
        let kind = match (first, hovered) {
            (Item::Entity(e), None) => match sketch.entities[e] {
                Entity::Line { .. } => Some(DimKind::Length(e)),
                Entity::Circle { .. } => Some(DimKind::Diameter(e)),
                Entity::Arc { .. } => Some(DimKind::Radius(e)),
            },
            (Item::Point(_), None) => None,
            (a, Some(b)) if a == b => None,
            (Item::Point(a), Some(Item::Point(b))) => Some(DimKind::Distance(a, b)),
            (Item::Point(p), Some(Item::Entity(e))) | (Item::Entity(e), Some(Item::Point(p))) => {
                match sketch.entities[e] {
                    Entity::Line { .. } => Some(DimKind::PointLine(p, e)),
                    Entity::Circle { c, .. } | Entity::Arc { c, .. } => {
                        Some(DimKind::Distance(p, c))
                    }
                }
            }
            (Item::Entity(e), Some(Item::Entity(f))) => {
                match (sketch.entities[e], sketch.entities[f]) {
                    (Entity::Line { a, b, .. }, Entity::Line { a: c, b: d, .. }) => {
                        let (u, v) = (
                            unit(sketch.points[a], sketch.points[b]),
                            unit(sketch.points[c], sketch.points[d]),
                        );
                        if (u[0] * v[1] - u[1] * v[0]).abs() < 1e-6 {
                            Some(DimKind::PointLine(c, e))
                        } else {
                            Some(DimKind::Angle(e, f))
                        }
                    }
                    (Entity::Line { .. }, Entity::Circle { c, .. } | Entity::Arc { c, .. }) => {
                        Some(DimKind::PointLine(c, e))
                    }
                    (Entity::Circle { c, .. } | Entity::Arc { c, .. }, Entity::Line { .. }) => {
                        Some(DimKind::PointLine(c, f))
                    }
                    (
                        Entity::Circle { c: c1, .. } | Entity::Arc { c: c1, .. },
                        Entity::Circle { c: c2, .. } | Entity::Arc { c: c2, .. },
                    ) => Some(DimKind::Distance(c1, c2)),
                }
            }
            _ => None,
        };
        self.dim_pick = None;
        let Some(kind) = kind else {
            if let Some(h) = hovered {
                self.dim_pick = Some(h);
            }
            return false;
        };
        let value = sketch.measure(kind).abs();
        let anchor = dim_anchor(sketch, kind);
        let offset = if hovered.is_none() {
            [q[0] - anchor[0], q[1] - anchor[1]]
        } else {
            [8.0 / view.scale * 3.0, 8.0 / view.scale * 3.0]
        };
        let before = self.status.dof;
        sketch.dims.push(Dim {
            kind,
            value,
            offset,
            flip: false,
        });
        let after = sketch.status();
        if after.dof >= before {
            sketch.dims.pop();
            self.message =
                "Cette cote surcontraindrait l’esquisse : la géométrie est déjà définie.".into();
            return false;
        }
        self.status = after;
        let i = sketch.dims.len() - 1;
        let at = frame.screen(view, [anchor[0] + offset[0], anchor[1] + offset[1]]);
        self.open_dim(sketch, i, true, at);
        true
    }
    fn open_dim(&mut self, sketch: &ProfileSketch, index: usize, fresh: bool, at: Pos2) {
        let d = sketch.dims[index];
        self.dim_edit = Some(DimEdit {
            index,
            text: psketch::fmt(d.value),
            at,
            fresh,
            old: d,
            focus: true,
        });
    }
    /// The "Modifier" box of a dimension. Returns true when the sketch changed.
    pub fn dim_popup(&mut self, ctx: &egui::Context, sketch: &mut ProfileSketch) -> bool {
        let Some(edit) = &mut self.dim_edit else {
            return false;
        };
        let mut close = false;
        let mut apply = false;
        let mut changed = false;
        let index = edit.index;
        egui::Area::new(egui::Id::new("dim-modify"))
            .fixed_pos(edit.at + Vec2::new(12.0, 12.0))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("Modifier").strong());
                        ui.small("(valeur ou calcul : 40/2+3)");
                    });
                    ui.horizontal(|ui| {
                        let r =
                            ui.add(egui::TextEdit::singleline(&mut edit.text).desired_width(110.0));
                        if edit.focus {
                            r.request_focus();
                            edit.focus = false;
                        }
                        if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                            apply = true;
                        }
                        ui.label(match sketch.dims[index].kind {
                            DimKind::Angle(..) => "°",
                            _ => "mm",
                        });
                        if ui
                            .button(egui::RichText::new("✔").color(Color32::from_rgb(30, 140, 50)))
                            .on_hover_text("Valider (Entrée)")
                            .clicked()
                        {
                            apply = true;
                        }
                        if ui
                            .button(egui::RichText::new("✖").color(Color32::from_rgb(200, 40, 40)))
                            .on_hover_text("Fermer (Échap)")
                            .clicked()
                        {
                            close = true;
                        }
                    });
                    if let DimKind::Distance(a, b)
                    | DimKind::DistanceX(a, b)
                    | DimKind::DistanceY(a, b) = sketch.dims[index].kind
                    {
                        ui.horizontal(|ui| {
                            for (label, kind) in [
                                ("Alignée", DimKind::Distance(a, b)),
                                ("Horizontale", DimKind::DistanceX(a, b)),
                                ("Verticale", DimKind::DistanceY(a, b)),
                            ] {
                                if ui
                                    .selectable_label(sketch.dims[index].kind == kind, label)
                                    .clicked()
                                    && sketch.dims[index].kind != kind
                                {
                                    let mut trial = sketch.clone();
                                    let value = trial.measure(kind);
                                    trial.dims[index].kind = kind;
                                    trial.dims[index].value = value.abs();
                                    trial.dims[index].flip = value < 0.0;
                                    if trial.solve().is_ok() {
                                        edit.text = psketch::fmt(value.abs());
                                        *sketch = trial;
                                        changed = true;
                                    }
                                }
                            }
                        });
                    }
                });
            });
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            close = true;
        }
        if apply {
            match eval(&edit.text) {
                Some(v)
                    if v > 0.0
                        || matches!(sketch.dims[index].kind, DimKind::Angle(..)) && v >= 0.0 =>
                {
                    let mut trial = sketch.clone();
                    trial.dims[index].value = v;
                    match trial.solve() {
                        Ok(()) => {
                            *sketch = trial;
                            changed = true;
                            close = true;
                        }
                        Err(_) => {
                            self.message =
                                "Valeur impossible avec les autres cotes et relations.".into()
                        }
                    }
                }
                _ => self.message = "Valeur invalide.".into(),
            }
        }
        if close {
            if !apply && !edit.fresh {
                // Closing without validating keeps the previous value.
                if sketch
                    .dims
                    .get(index)
                    .is_some_and(|d| d.value != edit.old.value)
                {
                    sketch.dims[index] = edit.old;
                    let _ = sketch.solve();
                    changed = true;
                }
            }
            self.dim_edit = None;
        }
        if changed {
            self.status_dirty = true;
            self.refresh(sketch);
        }
        changed
    }
    pub fn delete_selection(&mut self, sketch: &mut ProfileSketch) -> bool {
        if self.sel.is_empty() {
            return false;
        }
        let mut dims: Vec<usize> = Vec::new();
        let mut ents: Vec<usize> = Vec::new();
        for s in &self.sel {
            match *s {
                Item::Dim(d) => dims.push(d),
                Item::Entity(e) => ents.push(e),
                Item::Point(p) if p != 0 => {
                    ents.extend(
                        sketch
                            .entities
                            .iter()
                            .enumerate()
                            .filter(|(_, e)| e.points().contains(&p))
                            .map(|(i, _)| i),
                    );
                    sketch.rels.retain(|r| !r.points().contains(&p));
                }
                _ => {}
            }
        }
        dims.sort();
        dims.dedup();
        for d in dims.into_iter().rev() {
            if d < sketch.dims.len() {
                sketch.dims.remove(d);
            }
        }
        ents.sort();
        ents.dedup();
        for e in ents.into_iter().rev() {
            sketch.delete_entity(e);
        }
        sketch.purge_points();
        self.sel.clear();
        self.hover = None;
        let _ = sketch.solve();
        self.status_dirty = true;
        self.refresh(sketch);
        true
    }
    /// Relations that can be added to the current selection.
    pub fn possible_relations(&self, sketch: &ProfileSketch) -> Vec<Rel> {
        let points: Vec<usize> = self
            .sel
            .iter()
            .filter_map(|s| {
                if let Item::Point(p) = s {
                    Some(*p)
                } else {
                    None
                }
            })
            .collect();
        let ents: Vec<usize> = self
            .sel
            .iter()
            .filter_map(|s| {
                if let Item::Entity(e) = s {
                    Some(*e)
                } else {
                    None
                }
            })
            .collect();
        let is_line = |e: usize| matches!(sketch.entities[e], Entity::Line { .. });
        let mut out = Vec::new();
        match (points.as_slice(), ents.as_slice()) {
            ([], [e]) if is_line(*e) => out.extend([Rel::Horizontal(*e), Rel::Vertical(*e)]),
            ([], [e, f]) if is_line(*e) && is_line(*f) => out.extend([
                Rel::Parallel(*e, *f),
                Rel::Perpendicular(*e, *f),
                Rel::Equal(*e, *f),
            ]),
            ([], [e, f]) if is_line(*e) != is_line(*f) => out.push(Rel::Tangent(*e, *f)),
            ([], [e, f]) => out.extend([
                Rel::Concentric(*e, *f),
                Rel::Equal(*e, *f),
                Rel::Tangent(*e, *f),
            ]),
            ([p], []) => out.push(Rel::Fixed(*p, sketch.points[*p])),
            ([a, b], []) => out.extend([
                Rel::Coincident(*a, *b),
                Rel::HorizontalPoints(*a, *b),
                Rel::VerticalPoints(*a, *b),
            ]),
            ([p], [e]) if is_line(*e) => out.extend([Rel::Midpoint(*p, *e), Rel::PointOn(*p, *e)]),
            ([p], [e]) => out.push(Rel::PointOn(*p, *e)),
            _ => {}
        }
        out
    }
    pub fn add_relation(&mut self, sketch: &mut ProfileSketch, rel: Rel) -> bool {
        let mut trial = sketch.clone();
        trial.rels.push(rel);
        if trial.solve().is_err() {
            self.message = format!(
                "Relation « {} » impossible avec la géométrie actuelle.",
                rel.label()
            );
            return false;
        }
        let st = trial.status();
        if st.dof >= self.status.dof {
            self.message = format!(
                "Relation « {} » redondante : elle surcontraindrait l’esquisse.",
                rel.label()
            );
            return false;
        }
        *sketch = trial;
        self.status = st;
        self.message.clear();
        true
    }
    pub fn paint(
        &mut self,
        sketch: &ProfileSketch,
        frame: &Frame,
        view: &Viewport,
        painter: &egui::Painter,
        editing: bool,
    ) {
        let at = |p: [f64; 2]| frame.screen(view, p);
        if editing {
            // Sketch origin, red as in a sketch.
            let o = at([0.0, 0.0]);
            let red = Color32::from_rgb(210, 40, 40);
            for (d, label) in [([1.0, 0.0], "x"), ([0.0, 1.0], "y")] {
                let end = at([d[0] * 18.0 / view.scale, d[1] * 18.0 / view.scale]);
                arrow(painter, o, end, Stroke::new(1.6, red));
                painter.text(
                    end + (end - o).normalized() * 7.0,
                    egui::Align2::CENTER_CENTER,
                    label,
                    egui::FontId::proportional(10.0),
                    red,
                );
            }
        }
        for e in 0..sketch.entities.len() {
            let ent = sketch.entities[e];
            let pl: Vec<Pos2> = sketch.polyline(e).iter().map(|p| at(*p)).collect();
            let color = if editing {
                self.item_color(sketch, Item::Entity(e), self.entity_defined(sketch, e))
            } else {
                Color32::from_rgb(70, 90, 130)
            };
            let width =
                if self.sel.contains(&Item::Entity(e)) || self.hover == Some(Item::Entity(e)) {
                    2.8
                } else {
                    1.7
                };
            if ent.construction() {
                dashed(painter, &pl, Stroke::new(1.2, color));
            } else {
                painter.add(egui::Shape::line(pl, Stroke::new(width, color)));
            }
        }
        if editing {
            for (i, p) in sketch.points.iter().enumerate().skip(1) {
                let used = sketch.entities.iter().any(|e| e.points().contains(&i));
                if !used {
                    continue;
                }
                let color = self.item_color(
                    sketch,
                    Item::Point(i),
                    self.status.defined_points.get(i).copied().unwrap_or(false),
                );
                let r = if self.hover == Some(Item::Point(i)) || self.sel.contains(&Item::Point(i))
                {
                    4.0
                } else {
                    2.6
                };
                painter.circle_filled(at(*p), r, color);
            }
            if self.hover == Some(Item::Point(0)) {
                painter.circle_stroke(at([0.0, 0.0]), 6.0, Stroke::new(2.0, HOVER));
            }
            if self.relations {
                self.paint_relations(sketch, frame, view, painter);
            }
        }
        self.labels.clear();
        for i in 0..sketch.dims.len() {
            let color = if editing {
                self.item_color(sketch, Item::Dim(i), true)
            } else {
                Color32::from_rgb(70, 90, 130)
            };
            let rect = paint_dim(
                painter,
                sketch,
                &sketch.dims[i],
                frame,
                view,
                if self.hover == Some(Item::Dim(i)) || self.sel.contains(&Item::Dim(i)) {
                    color
                } else {
                    DIM
                },
                editing,
            );
            self.labels.push((i, rect));
        }
        if !editing {
            return;
        }
        // Tool preview.
        if let (Some(s), Some(_)) = (&self.snap, self.cursor) {
            let p = at(s.p);
            let preview = Stroke::new(1.5, Color32::from_rgb(255, 150, 0));
            for g in &s.guides {
                dashed(
                    painter,
                    &[at(g[0]), at(g[1])],
                    Stroke::new(1.0, Color32::from_rgb(60, 120, 255)),
                );
            }
            match s.target {
                Target::Point(_) => {
                    painter.circle_stroke(p, 7.0, Stroke::new(2.0, HOVER));
                }
                Target::Mid(_) => {
                    painter.add(egui::Shape::convex_polygon(
                        vec![
                            p + Vec2::new(0.0, -7.0),
                            p + Vec2::new(6.0, 4.0),
                            p + Vec2::new(-6.0, 4.0),
                        ],
                        Color32::TRANSPARENT,
                        Stroke::new(1.6, HOVER),
                    ));
                }
                Target::On(_) => {
                    painter.line_segment(
                        [p + Vec2::new(-5.0, -5.0), p + Vec2::new(5.0, 5.0)],
                        Stroke::new(1.6, HOVER),
                    );
                    painter.line_segment(
                        [p + Vec2::new(-5.0, 5.0), p + Vec2::new(5.0, -5.0)],
                        Stroke::new(1.6, HOVER),
                    );
                }
                Target::None => {
                    painter.circle_filled(p, 2.5, Color32::from_rgb(255, 150, 0));
                }
            }
            let badge = |text: &str| {
                let r = Rect::from_min_size(p + Vec2::new(12.0, 10.0), Vec2::new(15.0, 15.0));
                painter.rect_filled(r, 2.0, Color32::from_rgb(255, 236, 150));
                painter.rect_stroke(
                    r,
                    2.0,
                    Stroke::new(1.0, Color32::from_rgb(180, 140, 30)),
                    egui::StrokeKind::Inside,
                );
                painter.text(
                    r.center(),
                    egui::Align2::CENTER_CENTER,
                    text,
                    egui::FontId::proportional(10.0),
                    Color32::from_rgb(90, 70, 0),
                );
            };
            if s.h {
                badge("H");
            } else if s.v {
                badge("V");
            }
            let pts = &sketch.points;
            match self.tool {
                Tool::Line | Tool::Centerline => {
                    let start = if self.tool == Tool::Line {
                        self.chain
                    } else {
                        self.clicks.first().copied()
                    };
                    if let Some(a) = start {
                        painter.line_segment([at(pts[a]), p], preview);
                        let len = psketch::dist(pts[a], s.p);
                        painter.text(
                            p + Vec2::new(14.0, -14.0),
                            egui::Align2::LEFT_BOTTOM,
                            format!("{} mm", psketch::fmt((len * 100.0).round() / 100.0)),
                            egui::FontId::proportional(11.0),
                            Color32::from_rgb(80, 80, 80),
                        );
                    }
                }
                Tool::Rectangle => {
                    if let Some(&a) = self.clicks.first() {
                        let f = pts[a];
                        let c = [
                            [f[0], f[1]],
                            [s.p[0], f[1]],
                            s.p,
                            [f[0], s.p[1]],
                            [f[0], f[1]],
                        ];
                        painter.add(egui::Shape::line(
                            c.iter().map(|q| at(*q)).collect(),
                            preview,
                        ));
                        painter.text(
                            p + Vec2::new(14.0, -14.0),
                            egui::Align2::LEFT_BOTTOM,
                            format!(
                                "{} × {}",
                                psketch::fmt(((s.p[0] - f[0]).abs() * 100.0).round() / 100.0),
                                psketch::fmt(((s.p[1] - f[1]).abs() * 100.0).round() / 100.0)
                            ),
                            egui::FontId::proportional(11.0),
                            Color32::from_rgb(80, 80, 80),
                        );
                    }
                }
                Tool::CenterRectangle => {
                    if let Some(&a) = self.clicks.first() {
                        let f = pts[a];
                        let (dx, dy) = (s.p[0] - f[0], s.p[1] - f[1]);
                        let c = [
                            [f[0] - dx, f[1] - dy],
                            [f[0] + dx, f[1] - dy],
                            [f[0] + dx, f[1] + dy],
                            [f[0] - dx, f[1] + dy],
                            [f[0] - dx, f[1] - dy],
                        ];
                        painter.add(egui::Shape::line(
                            c.iter().map(|q| at(*q)).collect(),
                            preview,
                        ));
                    }
                }
                Tool::Circle => {
                    if let Some(&c) = self.clicks.first() {
                        let r = psketch::dist(pts[c], s.p);
                        let ring: Vec<Pos2> = (0..=64)
                            .map(|k| {
                                let t = k as f64 / 64.0 * std::f64::consts::TAU;
                                at([pts[c][0] + r * t.cos(), pts[c][1] + r * t.sin()])
                            })
                            .collect();
                        painter.add(egui::Shape::line(ring, preview));
                        painter.text(
                            p + Vec2::new(14.0, -14.0),
                            egui::Align2::LEFT_BOTTOM,
                            format!("Ø {}", psketch::fmt((2.0 * r * 100.0).round() / 100.0)),
                            egui::FontId::proportional(11.0),
                            Color32::from_rgb(80, 80, 80),
                        );
                    }
                }
                Tool::Arc => {
                    if self.clicks.len() == 1 {
                        painter.line_segment(
                            [at(pts[self.clicks[0]]), p],
                            Stroke::new(1.0, preview.color),
                        );
                    } else if self.clicks.len() == 2 {
                        let (c, a) = (pts[self.clicks[0]], pts[self.clicks[1]]);
                        let r = psketch::dist(c, a);
                        let l = psketch::dist(c, s.p).max(1e-9);
                        let b = [
                            c[0] + (s.p[0] - c[0]) * r / l,
                            c[1] + (s.p[1] - c[1]) * r / l,
                        ];
                        painter.add(egui::Shape::line(
                            psketch::arc_points(c, a, b, false)
                                .iter()
                                .map(|q| at(*q))
                                .collect(),
                            preview,
                        ));
                    }
                }
                Tool::Slot => {
                    if self.clicks.len() == 1 {
                        painter.line_segment([at(pts[self.clicks[0]]), p], preview);
                    } else if self.clicks.len() == 2 {
                        let (p1, p2) = (pts[self.clicks[0]], pts[self.clicks[1]]);
                        let w = psketch::line_distance(s.p, p1, p2).abs();
                        let l = psketch::dist(p1, p2).max(1e-9);
                        let d = [(p2[0] - p1[0]) / l, (p2[1] - p1[1]) / l];
                        let n = [-d[1] * w, d[0] * w];
                        let (a1, b1) = ([p1[0] + n[0], p1[1] + n[1]], [p1[0] - n[0], p1[1] - n[1]]);
                        let (a2, b2) = ([p2[0] + n[0], p2[1] + n[1]], [p2[0] - n[0], p2[1] - n[1]]);
                        let mut outline = vec![a1];
                        let mut cap2 = psketch::arc_points(p2, b2, a2, false);
                        cap2.reverse();
                        outline.extend(cap2);
                        let mut cap1 = psketch::arc_points(p1, a1, b1, false);
                        cap1.reverse();
                        outline.extend(cap1);
                        painter.add(egui::Shape::line(
                            outline.iter().map(|q| at(*q)).collect(),
                            preview,
                        ));
                    }
                }
                _ => {}
            }
        }
        if self.tool == Tool::Dimension {
            if let (Some(Item::Entity(e)), Some(q)) = (self.dim_pick, self.cursor) {
                let kind = match sketch.entities[e] {
                    Entity::Line { .. } => DimKind::Length(e),
                    Entity::Circle { .. } => DimKind::Diameter(e),
                    Entity::Arc { .. } => DimKind::Radius(e),
                };
                let anchor = dim_anchor(sketch, kind);
                let ghost = Dim {
                    kind,
                    value: sketch.measure(kind).abs(),
                    offset: [q[0] - anchor[0], q[1] - anchor[1]],
                    flip: false,
                };
                paint_dim(
                    painter,
                    sketch,
                    &ghost,
                    frame,
                    view,
                    Color32::from_rgb(120, 120, 120),
                    true,
                );
            }
        }
    }
    fn paint_relations(
        &self,
        sketch: &ProfileSketch,
        frame: &Frame,
        view: &Viewport,
        painter: &egui::Painter,
    ) {
        let mut slots: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        for r in &sketch.rels {
            let glyph = match r {
                Rel::Horizontal(_) | Rel::HorizontalPoints(..) => "H",
                Rel::Vertical(_) | Rel::VerticalPoints(..) => "V",
                Rel::Parallel(..) => "//",
                Rel::Perpendicular(..) => "L",
                Rel::Equal(..) => "=",
                Rel::Tangent(..) => "T",
                Rel::Concentric(..) => "C",
                Rel::Midpoint(..) => "M",
                Rel::PointOn(..) | Rel::Coincident(..) => continue,
                Rel::Fixed(..) => "F",
            };
            let mut anchors: Vec<[f64; 2]> = r
                .entities()
                .iter()
                .filter(|e| **e < sketch.entities.len())
                .map(|e| {
                    let pl = sketch.polyline(*e);
                    match sketch.entities[*e] {
                        Entity::Line { .. } => {
                            [(pl[0][0] + pl[1][0]) / 2.0, (pl[0][1] + pl[1][1]) / 2.0]
                        }
                        _ => pl[pl.len() / 4],
                    }
                })
                .collect();
            if anchors.is_empty() {
                anchors = r
                    .points()
                    .iter()
                    .filter(|p| **p < sketch.points.len())
                    .map(|p| sketch.points[*p])
                    .collect();
            }
            let key = r
                .entities()
                .first()
                .copied()
                .unwrap_or(10_000 + r.points().first().copied().unwrap_or(0));
            let slot = slots.entry(key).or_insert(0);
            for a in anchors {
                let p = frame.screen(view, a) + Vec2::new(8.0 + *slot as f32 * 15.0, 8.0);
                let rect = Rect::from_min_size(p, Vec2::new(13.0, 12.0));
                painter.rect_filled(
                    rect,
                    2.0,
                    Color32::from_rgba_unmultiplied(225, 245, 225, 230),
                );
                painter.rect_stroke(
                    rect,
                    2.0,
                    Stroke::new(0.8, RELATION),
                    egui::StrokeKind::Inside,
                );
                painter.text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    glyph,
                    egui::FontId::proportional(9.0),
                    RELATION,
                );
            }
            *slot += 1;
        }
    }
}
/// One frame of dragging. The solve starts from the current shape, so the geometry follows
/// the pointer smoothly; when the target cannot be reached at once, shorter steps are tried.
fn drag_step(
    sketch: &ProfileSketch,
    base: &ProfileSketch,
    item: Item,
    last: [f64; 2],
    q: [f64; 2],
) -> Option<ProfileSketch> {
    if let Item::Dim(d) = item {
        let mut next = sketch.clone();
        let anchor_shift = [q[0] - last[0], q[1] - last[1]];
        let dim = next.dims.get_mut(d)?;
        dim.offset = [
            dim.offset[0] + anchor_shift[0],
            dim.offset[1] + anchor_shift[1],
        ];
        let _ = base;
        return Some(next);
    }
    for t in [1.0, 0.5, 0.25, 0.1] {
        let target = [
            last[0] + (q[0] - last[0]) * t,
            last[1] + (q[1] - last[1]) * t,
        ];
        let delta = [target[0] - last[0], target[1] - last[1]];
        let mut trial = sketch.clone();
        let shift = |p: [f64; 2]| [p[0] + delta[0], p[1] + delta[1]];
        let ok = match item {
            Item::Point(i) => trial.solve_with(&[Rel::Fixed(i, target)]).is_ok(),
            Item::Entity(e) => match trial.entities[e] {
                Entity::Line { a, b, .. } => {
                    let (pa, pb) = (sketch.points[a], sketch.points[b]);
                    trial
                        .solve_with(&[Rel::Fixed(a, shift(pa)), Rel::Fixed(b, shift(pb))])
                        .is_ok()
                }
                Entity::Circle { c, .. } => {
                    if let Entity::Circle { r, .. } = &mut trial.entities[e] {
                        *r = psketch::dist(target, sketch.points[c]).max(0.01);
                    }
                    trial.solve().is_ok()
                }
                Entity::Arc { c, .. } => trial
                    .solve_with(&[Rel::Fixed(c, shift(sketch.points[c]))])
                    .is_ok(),
            },
            Item::Dim(_) => unreachable!(),
        };
        // A fully defined element snaps back: the solve "succeeds" without following the pointer.
        let followed = match item {
            Item::Point(i) => psketch::dist(trial.points[i], target) < 1e-3,
            _ => true,
        };
        if ok && followed {
            return Some(trial);
        }
    }
    None
}
fn unit(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    let l = psketch::dist(a, b).max(1e-12);
    [(b[0] - a[0]) / l, (b[1] - a[1]) / l]
}
pub fn segment_distance_2d(p: Pos2, a: Pos2, b: Pos2) -> f32 {
    let d = b - a;
    let t = ((p - a).dot(d) / d.length_sq().max(1e-9)).clamp(0.0, 1.0);
    (a + d * t).distance(p)
}
pub fn dashed(painter: &egui::Painter, pts: &[Pos2], stroke: Stroke) {
    let mut on = true;
    let mut left: f32 = 6.0;
    for w in pts.windows(2) {
        let (mut a, b) = (w[0], w[1]);
        let mut len = a.distance(b);
        while len > 0.0 {
            let step = left.min(len);
            let next = a + (b - a).normalized() * step;
            if on {
                painter.line_segment([a, next], stroke);
            }
            left -= step;
            len -= step;
            a = next;
            if left <= 0.0 {
                on = !on;
                left = if on { 6.0 } else { 4.0 };
            }
        }
    }
}
pub fn arrow(painter: &egui::Painter, from: Pos2, to: Pos2, stroke: Stroke) {
    painter.line_segment([from, to], stroke);
    let d = (to - from).normalized();
    let n = Vec2::new(-d.y, d.x);
    painter.add(egui::Shape::convex_polygon(
        vec![to, to - d * 7.0 + n * 3.0, to - d * 7.0 - n * 3.0],
        stroke.color,
        Stroke::NONE,
    ));
}
fn arrowhead(painter: &egui::Painter, tip: Pos2, dir: Vec2, color: Color32) {
    let d = dir.normalized();
    let n = Vec2::new(-d.y, d.x);
    painter.add(egui::Shape::convex_polygon(
        vec![tip, tip - d * 8.0 + n * 2.6, tip - d * 8.0 - n * 2.6],
        color,
        Stroke::NONE,
    ));
}
/// Reference point of a dimension, in sketch coordinates.
pub fn dim_anchor(s: &ProfileSketch, kind: DimKind) -> [f64; 2] {
    let p = &s.points;
    let mid = |a: [f64; 2], b: [f64; 2]| [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0];
    match kind {
        DimKind::Distance(a, b) | DimKind::DistanceX(a, b) | DimKind::DistanceY(a, b) => {
            mid(p[a], p[b])
        }
        DimKind::Length(e) => match s.entities[e] {
            Entity::Line { a, b, .. } => mid(p[a], p[b]),
            _ => [0.0, 0.0],
        },
        DimKind::PointLine(q, e) => match s.entities[e] {
            Entity::Line { a, b, .. } => {
                let foot = foot(p[q], p[a], p[b]);
                mid(p[q], foot)
            }
            _ => p[q],
        },
        DimKind::Diameter(e) | DimKind::Radius(e) => match s.entities[e] {
            Entity::Circle { c, .. } | Entity::Arc { c, .. } => p[c],
            _ => [0.0, 0.0],
        },
        DimKind::Angle(e, f) => match (s.entities[e], s.entities[f]) {
            (Entity::Line { a, b, .. }, Entity::Line { a: c, b: d, .. }) => {
                intersection(p[a], p[b], p[c], p[d]).unwrap_or(mid(p[a], p[c]))
            }
            _ => [0.0, 0.0],
        },
    }
}
fn foot(q: [f64; 2], a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
    let d = [b[0] - a[0], b[1] - a[1]];
    let t = ((q[0] - a[0]) * d[0] + (q[1] - a[1]) * d[1]) / (d[0] * d[0] + d[1] * d[1]).max(1e-12);
    [a[0] + d[0] * t, a[1] + d[1] * t]
}
fn intersection(a: [f64; 2], b: [f64; 2], c: [f64; 2], d: [f64; 2]) -> Option<[f64; 2]> {
    let r = [b[0] - a[0], b[1] - a[1]];
    let s = [d[0] - c[0], d[1] - c[1]];
    let den = r[0] * s[1] - r[1] * s[0];
    if den.abs() < 1e-12 {
        return None;
    }
    let t = ((c[0] - a[0]) * s[1] - (c[1] - a[1]) * s[0]) / den;
    Some([a[0] + r[0] * t, a[1] + r[1] * t])
}
/// Draws a dimension and returns the rectangle of its value.
pub fn paint_dim(
    painter: &egui::Painter,
    s: &ProfileSketch,
    d: &Dim,
    frame: &Frame,
    view: &Viewport,
    color: Color32,
    editing: bool,
) -> Rect {
    let at = |p: [f64; 2]| frame.screen(view, p);
    let anchor = dim_anchor(s, d.kind);
    let label = [anchor[0] + d.offset[0], d.offset[1] + anchor[1]];
    let thin = Stroke::new(0.9, color);
    let p = &s.points;
    let linear = |painter: &egui::Painter, a: [f64; 2], b: [f64; 2], dir: [f64; 2]| {
        let n = [-dir[1], dir[0]];
        let off = |q: [f64; 2]| (label[0] - q[0]) * n[0] + (label[1] - q[1]) * n[1];
        let a2 = [a[0] + n[0] * off(a), a[1] + n[1] * off(a)];
        let b2 = [b[0] + n[0] * off(b), b[1] + n[1] * off(b)];
        painter.line_segment([at(a), at(a2)], Stroke::new(0.7, color));
        painter.line_segment([at(b), at(b2)], Stroke::new(0.7, color));
        let (sa, sb) = (at(a2), at(b2));
        painter.line_segment([sa, sb], thin);
        if sa.distance(sb) > 4.0 {
            arrowhead(painter, sa, sa - sb, color);
            arrowhead(painter, sb, sb - sa, color);
        }
        // Extend the dimension line to the label when it is placed outside.
        let sl = at(label);
        let ab = sb - sa;
        let t = (sl - sa).dot(ab) / ab.length_sq().max(1e-6);
        if !(0.0..=1.0).contains(&t) {
            let end = if t < 0.0 { sa } else { sb };
            painter.line_segment([end, sa + ab * t], thin);
        }
    };
    match d.kind {
        DimKind::Distance(a, b) => {
            let u = unit(p[a], p[b]);
            if psketch::dist(p[a], p[b]) > 1e-9 {
                linear(painter, p[a], p[b], u);
            }
        }
        DimKind::DistanceX(a, b) => linear(painter, p[a], p[b], [1.0, 0.0]),
        DimKind::DistanceY(a, b) => linear(painter, p[a], p[b], [0.0, 1.0]),
        DimKind::Length(e) => {
            if let Entity::Line { a, b, .. } = s.entities[e] {
                linear(painter, p[a], p[b], unit(p[a], p[b]));
            }
        }
        DimKind::PointLine(q, e) => {
            if let Entity::Line { a, b, .. } = s.entities[e] {
                let f = foot(p[q], p[a], p[b]);
                if psketch::dist(f, p[q]) > 1e-9 {
                    linear(painter, f, p[q], unit(f, p[q]));
                }
            }
        }
        DimKind::Diameter(e) | DimKind::Radius(e) => {
            if let Entity::Circle { c, .. } | Entity::Arc { c, .. } = s.entities[e] {
                let r = s.radius(e);
                let dir = unit(p[c], label);
                let near = [p[c][0] + dir[0] * r, p[c][1] + dir[1] * r];
                let far = [p[c][0] - dir[0] * r, p[c][1] - dir[1] * r];
                let start = if matches!(d.kind, DimKind::Diameter(_)) {
                    far
                } else {
                    p[c]
                };
                painter.line_segment([at(start), at(label)], thin);
                arrowhead(painter, at(near), at(near) - at(start), color);
                if matches!(d.kind, DimKind::Diameter(_)) {
                    arrowhead(painter, at(far), at(far) - at(near), color);
                }
            }
        }
        DimKind::Angle(e, f) => {
            if let (Entity::Line { a, b, .. }, Entity::Line { a: c, b: dd, .. }) =
                (s.entities[e], s.entities[f])
            {
                let r = psketch::dist(anchor, label).max(1e-6);
                // The measured angle is between the directions a→b and c→d; draw it on the side
                // of the label (the opposite sector has the same angle).
                let mut u = unit(p[a], p[b]);
                let mut v = unit(p[c], p[dd]);
                let to_label = unit(anchor, label);
                if (u[0] + v[0]) * to_label[0] + (u[1] + v[1]) * to_label[1] < 0.0 {
                    u = [-u[0], -u[1]];
                    v = [-v[0], -v[1]];
                }
                let t0 = u[1].atan2(u[0]);
                let sweep = (u[0] * v[1] - u[1] * v[0]).atan2(u[0] * v[0] + u[1] * v[1]);
                let t1 = t0 + sweep;
                let pts: Vec<Pos2> = (0..=24)
                    .map(|k| {
                        let t = t0 + (t1 - t0) * k as f64 / 24.0;
                        at([anchor[0] + r * t.cos(), anchor[1] + r * t.sin()])
                    })
                    .collect();
                painter.add(egui::Shape::line(pts, thin));
            }
        }
    }
    let text = d.text();
    let galley = painter.layout_no_wrap(
        text,
        egui::FontId::proportional(if editing { 13.0 } else { 11.0 }),
        color,
    );
    let rect = Rect::from_center_size(at(label), galley.size() + Vec2::new(6.0, 2.0));
    painter.rect_filled(
        rect,
        2.0,
        Color32::from_rgba_unmultiplied(255, 255, 255, 215),
    );
    painter.galley(rect.min + Vec2::new(3.0, 1.0), galley, color);
    rect
}

#[cfg(test)]
mod tests {
    use super::*;
    use solidcraft::psketch::{Dim, DimKind, SketchPlane};
    fn triangle() -> ProfileSketch {
        let mut s = ProfileSketch::on(SketchPlane::top());
        let a = 0;
        let b = s.add_point([10.0, -20.0]);
        let c = s.add_point([5.0, 30.0]);
        s.add(Entity::Line {
            a,
            b,
            construction: false,
        });
        s.add(Entity::Line {
            a: b,
            b: c,
            construction: false,
        });
        s.add(Entity::Line {
            a: c,
            b: a,
            construction: false,
        });
        s.dims.push(Dim {
            kind: DimKind::Angle(0, 2),
            value: s.measure(DimKind::Angle(0, 2)),
            offset: [5.0, 5.0],
            flip: false,
        });
        s
    }
    #[test]
    fn free_point_follows_the_pointer_and_keeps_the_angle() {
        let s = triangle();
        let next =
            drag_step(&s, &s, Item::Point(2), s.points[2], [8.0, 40.0]).expect("free point moves");
        assert!(psketch::dist(next.points[2], [8.0, 40.0]) < 1e-3);
        assert!((next.measure(DimKind::Angle(0, 2)) - s.dims[0].value).abs() < 1e-3);
    }
    #[test]
    fn fully_defined_point_does_not_move() {
        let mut s = triangle();
        s.rels.push(Rel::Fixed(2, s.points[2]));
        assert!(drag_step(&s, &s, Item::Point(2), s.points[2], [20.0, 40.0]).is_none());
    }
}
