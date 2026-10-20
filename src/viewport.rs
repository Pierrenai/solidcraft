//! Orthographic software renderer with per-pixel face identifiers.
//!
//! The scene is rasterised only when the camera or the geometry changes. Highlights (pre-selection,
//! selection, preview) only recolour the stored image, so hovering stays instantaneous.
use eframe::egui::{self, Color32, ColorImage, Pos2, Rect, Stroke, TextureHandle, Vec2};
use solidcraft::{
    geometry::{Mesh, Vec3},
    part,
};
use std::collections::HashMap;

#[derive(Clone, Copy, Debug)]
pub struct SurfaceHit {
    pub body: usize,
    pub tag: u32,
    pub point: Vec3,
    pub normal: Vec3,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Pick {
    Face(usize, u32),
    Feature(usize, u32),
    Body(usize),
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tint {
    pub pick: Pick,
    pub color: Color32,
    pub amount: f32,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Style {
    ShadedEdges,
    Shaded,
    HiddenLines,
    Wireframe,
}
impl Style {
    pub fn label(self) -> &'static str {
        match self {
            Self::ShadedEdges => "Ombré avec arêtes",
            Self::Shaded => "Ombré",
            Self::HiddenLines => "Lignes cachées supprimées",
            Self::Wireframe => "Image filaire",
        }
    }
}
struct Tri {
    p: [Vec3; 3],
    n: [Vec3; 3],
    flat: Vec3,
    face: u32,
}
pub struct Viewport {
    pub rect: Rect,
    right: Vec3,
    up: Vec3,
    anim: Option<([Vec3; 2], [Vec3; 2], f64)>,
    pub scale: f64,
    pub target: Vec3,
    pub pan: Vec2,
    pub style: Style,
    pub dirty: bool,
    pub left_orbit: bool,
    version: u64,
    tris: Vec<Tri>,
    faces: Vec<(usize, u32)>,
    colors: Vec<[u8; 3]>,
    size: [usize; 2],
    ppp: f32,
    depth: Vec<f32>,
    ids: Vec<u32>,
    shade: Vec<Color32>,
    edge: Vec<bool>,
    tints: Vec<Tint>,
    texture: Option<TextureHandle>,
    compose_needed: bool,
    requested_ppp: f32,
    pub hover_pos: Option<Pos2>,
}
impl Default for Viewport {
    fn default() -> Self {
        let (right, up) = Self::iso();
        Self {
            rect: Rect::NOTHING,
            right,
            up,
            anim: None,
            scale: 4.0,
            target: Vec3::default(),
            pan: Vec2::ZERO,
            style: Style::ShadedEdges,
            dirty: true,
            left_orbit: true,
            version: u64::MAX,
            tris: Vec::new(),
            faces: Vec::new(),
            colors: Vec::new(),
            size: [0, 0],
            ppp: 1.0,
            depth: Vec::new(),
            ids: Vec::new(),
            shade: Vec::new(),
            edge: Vec::new(),
            tints: Vec::new(),
            texture: None,
            compose_needed: true,
            requested_ppp: 0.0,
            hover_pos: None,
        }
    }
}
fn orthonormal(right: Vec3, up: Vec3) -> (Vec3, Vec3) {
    let r = right.normalized();
    let u = (up - r * up.dot(r)).normalized();
    (r, u)
}
fn rotate(v: Vec3, axis: Vec3, angle: f64) -> Vec3 {
    let (s, c) = angle.sin_cos();
    v * c + axis.cross(v) * s + axis * (axis.dot(v) * (1.0 - c))
}
pub const VIEWS: [(&str, &str); 8] = [
    ("Face", "Ctrl+1"),
    ("Arrière", "Ctrl+2"),
    ("Gauche", "Ctrl+3"),
    ("Droite", "Ctrl+4"),
    ("Dessus", "Ctrl+5"),
    ("Dessous", "Ctrl+6"),
    ("Isométrique", "Ctrl+7"),
    ("Dimétrique", ""),
];
impl Viewport {
    fn iso() -> (Vec3, Vec3) {
        Self::from_eye(Vec3::new(1.0, -1.0, 1.0))
    }
    fn from_eye(eye: Vec3) -> (Vec3, Vec3) {
        let eye = eye.normalized();
        let z = Vec3::new(0.0, 0.0, 1.0);
        let mut right = z.cross(eye);
        if right.length() < 1e-6 {
            right = Vec3::new(1.0, 0.0, 0.0);
        }
        let right = right.normalized();
        (right, eye.cross(right))
    }
    /// Screen right, screen up and the direction towards the viewer.
    pub fn basis(&self) -> (Vec3, Vec3, Vec3) {
        (self.right, self.up, self.right.cross(self.up))
    }
    pub fn set_view(&mut self, right: Vec3, up: Vec3, ctx: &egui::Context) {
        let (r, u) = orthonormal(right, up);
        let now = ctx.input(|i| i.time);
        self.anim = Some(([self.right, self.up], [r, u], now));
        ctx.request_repaint();
        self.dirty = true;
    }
    pub fn standard(&mut self, index: usize, ctx: &egui::Context) {
        let (x, y, z) = (
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        );
        let (r, u) = match index {
            0 => (x, z),
            1 => (-x, z),
            2 => (-y, z),
            3 => (y, z),
            4 => (x, y),
            5 => (x, -y),
            6 => Self::iso(),
            _ => Self::from_eye(Vec3::new(0.6, -1.0, 0.55)),
        };
        self.set_view(r, u, ctx);
    }
    /// Looks straight at a plane, keeping its own axes horizontal and vertical.
    pub fn normal_to(&mut self, u: Vec3, v: Vec3, ctx: &egui::Context) {
        self.set_view(u, v, ctx);
    }
    pub fn orbit(&mut self, delta: Vec2) {
        let z = Vec3::new(0.0, 0.0, 1.0);
        let a = -delta.x as f64 * 0.008;
        let b = -delta.y as f64 * 0.008;
        let r = rotate(self.right, z, a);
        let u = rotate(self.up, z, a);
        let u = rotate(u, r, b);
        let (r, u) = orthonormal(r, u);
        self.right = r;
        self.up = u;
        self.anim = None;
        self.dirty = true;
    }
    pub fn project(&self, v: Vec3) -> Pos2 {
        let d = v - self.target;
        self.rect.center()
            + self.pan
            + Vec2::new(
                (d.dot(self.right) * self.scale) as f32,
                (-d.dot(self.up) * self.scale) as f32,
            )
    }
    /// Distance towards the viewer (larger is nearer).
    pub fn depth_of(&self, v: Vec3) -> f64 {
        (v - self.target).dot(self.right.cross(self.up))
    }
    pub fn unproject(&self, p: Pos2, depth: f64) -> Vec3 {
        let (r, u, e) = self.basis();
        let d = p - self.rect.center() - self.pan;
        self.target + r * (d.x as f64 / self.scale) - u * (d.y as f64 / self.scale) + e * depth
    }
    pub fn on_plane(&self, p: Pos2, origin: Vec3, normal: Vec3) -> Option<Vec3> {
        let e = self.right.cross(self.up);
        let base = self.unproject(p, 0.0);
        let denom = e.dot(normal);
        if denom.abs() < 0.02 {
            return None;
        }
        Some(base + e * ((origin - base).dot(normal) / denom))
    }
    pub fn fit(&mut self, meshes: &[Mesh], extra: &[Vec3]) {
        let mut pts: Vec<Vec3> = meshes
            .iter()
            .flat_map(|m| m.polygons.iter().flat_map(|p| p.vertices.iter().copied()))
            .collect();
        pts.extend_from_slice(extra);
        if pts.is_empty() {
            self.target = Vec3::default();
            self.scale = 4.0;
            self.pan = Vec2::ZERO;
            self.dirty = true;
            return;
        }
        let (r, u, _) = self.basis();
        let (mut x0, mut x1, mut y0, mut y1) = (f64::MAX, f64::MIN, f64::MAX, f64::MIN);
        let mut c = Vec3::default();
        for p in &pts {
            x0 = x0.min(p.dot(r));
            x1 = x1.max(p.dot(r));
            y0 = y0.min(p.dot(u));
            y1 = y1.max(p.dot(u));
            c = c + *p;
        }
        c = c / pts.len() as f64;
        let e = r.cross(u);
        self.target = r * ((x0 + x1) / 2.0) + u * ((y0 + y1) / 2.0) + e * c.dot(e);
        let w = self.rect.width().max(200.0) as f64;
        let h = self.rect.height().max(200.0) as f64;
        self.scale = ((w * 0.8) / (x1 - x0).max(1.0))
            .min((h * 0.75) / (y1 - y0).max(1.0))
            .clamp(0.005, 2000.0);
        self.pan = Vec2::ZERO;
        self.dirty = true;
    }
    pub fn animating(&self) -> bool {
        self.anim.is_some()
    }
    pub fn zoom_at(&mut self, at: Pos2, factor: f64) {
        let new_scale = (self.scale * factor).clamp(0.005, 2000.0);
        let f = (new_scale / self.scale) as f32;
        let c = self.rect.center();
        self.pan = at - c - (at - c - self.pan) * f;
        self.scale = new_scale;
        self.dirty = true;
    }
    pub fn tints(&mut self, tints: Vec<Tint>) {
        if tints != self.tints {
            self.tints = tints;
            self.compose_needed = true;
        }
    }
    /// Face under a screen position.
    pub fn hit(&self, p: Pos2) -> Option<SurfaceHit> {
        let i = self.pixel(p)?;
        let id = self.ids[i];
        if id == 0 {
            return None;
        }
        let (body, tag) = self.faces[id as usize - 1];
        let point = self.unproject(p, self.depth[i] as f64);
        // Normal of the nearest triangle of that face.
        let normal = self
            .tris
            .iter()
            .filter(|t| t.face == id)
            .map(|t| {
                let c = (t.p[0] + t.p[1] + t.p[2]) / 3.0;
                ((c - point).length(), t.flat)
            })
            .min_by(|a, b| a.0.total_cmp(&b.0))
            .map(|t| t.1)
            .unwrap_or_default();
        Some(SurfaceHit {
            body,
            tag,
            point,
            normal,
        })
    }
    fn pixel(&self, p: Pos2) -> Option<usize> {
        let x = ((p.x - self.rect.left()) * self.ppp).floor();
        let y = ((p.y - self.rect.top()) * self.ppp).floor();
        if x < 0.0 || y < 0.0 || x as usize >= self.size[0] || y as usize >= self.size[1] {
            return None;
        }
        Some(y as usize * self.size[0] + x as usize)
    }
    /// False when a solid hides the point.
    pub fn visible(&self, v: Vec3) -> bool {
        let p = self.project(v);
        let Some(i) = self.pixel(p) else {
            return true;
        };
        if self.ids.get(i).copied().unwrap_or(0) == 0 {
            return true;
        }
        self.depth_of(v) >= self.depth[i] as f64 - (0.05 + 2.5 / self.scale)
    }
    fn prepare(&mut self, meshes: &[Mesh], colors: &[[u8; 3]], visible: &[bool]) {
        self.tris.clear();
        self.faces.clear();
        self.colors = colors.to_vec();
        let mut face_index: HashMap<(usize, u32), u32> = HashMap::new();
        for (b, mesh) in meshes.iter().enumerate() {
            if !visible.get(b).copied().unwrap_or(true) {
                continue;
            }
            // Smooth normals inside curved faces.
            let mut planar: HashMap<u32, (Vec3, bool)> = HashMap::new();
            for p in &mesh.polygons {
                let e = planar.entry(p.tag).or_insert((p.normal, true));
                if e.0.dot(p.normal) < 0.9999 {
                    e.1 = false;
                }
            }
            let key = |v: Vec3, tag: u32| {
                (
                    (v.x * 1e4).round() as i64,
                    (v.y * 1e4).round() as i64,
                    (v.z * 1e4).round() as i64,
                    tag,
                )
            };
            let mut smooth: HashMap<(i64, i64, i64, u32), Vec3> = HashMap::new();
            for p in &mesh.polygons {
                if planar[&p.tag].1 || p.tag == 0 {
                    continue;
                }
                for v in &p.vertices {
                    let s = smooth.entry(key(*v, p.tag)).or_default();
                    *s = *s + p.normal;
                }
            }
            for p in &mesh.polygons {
                let next = face_index.len() as u32 + 1;
                let id = *face_index.entry((b, p.tag)).or_insert_with(|| {
                    self.faces.push((b, p.tag));
                    next
                });
                let curved = !planar[&p.tag].1 && p.tag != 0;
                let normal_at = |v: Vec3| {
                    if curved {
                        let n = smooth
                            .get(&key(v, p.tag))
                            .copied()
                            .unwrap_or(p.normal)
                            .normalized();
                        if n.dot(p.normal) > 0.5 {
                            return n;
                        }
                    }
                    p.normal
                };
                for i in 1..p.vertices.len().saturating_sub(1) {
                    let t = [p.vertices[0], p.vertices[i], p.vertices[i + 1]];
                    if (t[1] - t[0]).cross(t[2] - t[0]).length() < 1e-12 {
                        continue;
                    }
                    self.tris.push(Tri {
                        p: t,
                        n: t.map(normal_at),
                        flat: p.normal,
                        face: id,
                    });
                }
            }
        }
    }
    fn render(&mut self, rect: Rect, ppp: f32) {
        let w = (rect.width() * ppp).ceil().clamp(1.0, 2600.0) as usize;
        let h = (rect.height() * ppp).ceil().clamp(1.0, 1700.0) as usize;
        self.ppp = (w as f32 / rect.width().max(1.0)).min(h as f32 / rect.height().max(1.0));
        self.size = [w, h];
        self.depth = vec![f32::NEG_INFINITY; w * h];
        self.ids = vec![0; w * h];
        self.shade = vec![Color32::TRANSPARENT; w * h];
        let mut tri_of = vec![u32::MAX; w * h];
        let (right, up, eye) = self.basis();
        let light = (eye * 0.75 + up * 0.5 - right * 0.35).normalized();
        let half = (light + eye).normalized();
        let k = self.scale * self.ppp as f64;
        let cx = (w as f64) / 2.0 + self.pan.x as f64 * self.ppp as f64;
        let cy = (h as f64) / 2.0 + self.pan.y as f64 * self.ppp as f64;
        let hidden_lines = self.style == Style::HiddenLines;
        for (ti, t) in self.tris.iter().enumerate() {
            if t.flat.dot(eye) < -1e-4 {
                continue;
            }
            let s: [(f64, f64, f64); 3] = t.p.map(|v| {
                let d = v - self.target;
                (cx + d.dot(right) * k, cy - d.dot(up) * k, d.dot(eye))
            });
            let area =
                (s[1].0 - s[0].0) * (s[2].1 - s[0].1) - (s[1].1 - s[0].1) * (s[2].0 - s[0].0);
            if area.abs() < 1e-9 {
                continue;
            }
            let (b, _) = self.faces[t.face as usize - 1];
            let base = self.colors.get(b).copied().unwrap_or([200, 204, 210]);
            let miny = s
                .iter()
                .map(|v| v.1)
                .fold(f64::INFINITY, f64::min)
                .floor()
                .max(0.0) as usize;
            let maxy = s
                .iter()
                .map(|v| v.1)
                .fold(f64::NEG_INFINITY, f64::max)
                .ceil()
                .min(h as f64 - 1.0);
            if maxy < 0.0 {
                continue;
            }
            let maxy = maxy as usize;
            for y in miny..=maxy {
                let py = y as f64 + 0.5;
                // Span of x where all three edge functions are non-negative.
                let (mut x0, mut x1) = (0.0_f64, w as f64);
                let mut empty = false;
                for e in 0..3 {
                    let a = s[e];
                    let b = s[(e + 1) % 3];
                    // f(x) = (b.x-a.x)*(py-a.y) - (b.y-a.y)*(x-a.x), sign normalised by area.
                    let slope = -(b.1 - a.1) * area.signum();
                    let off = ((b.0 - a.0) * (py - a.1) + (b.1 - a.1) * a.0) * area.signum();
                    if slope.abs() < 1e-12 {
                        if off < -1e-9 {
                            empty = true;
                        }
                    } else if slope > 0.0 {
                        x0 = x0.max(-off / slope);
                    } else {
                        x1 = x1.min(-off / slope);
                    }
                }
                if empty || x1 < x0 {
                    continue;
                }
                let xs = (x0 - 0.5).ceil().max(0.0) as usize;
                let xe = ((x1 - 0.5).floor()).min(w as f64 - 1.0);
                if xe < 0.0 {
                    continue;
                }
                for x in xs..=xe as usize {
                    let px = x as f64 + 0.5;
                    let l0 = ((s[1].0 - px) * (s[2].1 - py) - (s[1].1 - py) * (s[2].0 - px)) / area;
                    let l1 = ((s[2].0 - px) * (s[0].1 - py) - (s[2].1 - py) * (s[0].0 - px)) / area;
                    let l2 = 1.0 - l0 - l1;
                    let z = (l0 * s[0].2 + l1 * s[1].2 + l2 * s[2].2) as f32;
                    let i = y * w + x;
                    if z <= self.depth[i] {
                        continue;
                    }
                    self.depth[i] = z;
                    self.ids[i] = t.face;
                    tri_of[i] = ti as u32;
                    let color = if hidden_lines {
                        Color32::WHITE
                    } else {
                        let n = (t.n[0] * l0 + t.n[1] * l1 + t.n[2] * l2).normalized();
                        let diffuse = n.dot(light).max(0.0);
                        let facing = n.dot(eye).max(0.0);
                        let spec = n.dot(half).max(0.0).powi(40) * 0.30;
                        let i = 0.30 + 0.55 * diffuse + 0.20 * facing;
                        let c =
                            base.map(|v| ((v as f64 * i + 255.0 * spec).clamp(0.0, 255.0)) as u8);
                        Color32::from_rgb(c[0], c[1], c[2])
                    };
                    self.shade[i] = color;
                }
            }
        }
        // Visible boundaries: changes of face, creases and silhouettes, one pixel wide.
        self.edge = vec![false; w * h];
        let crease = |a: u32, b: u32, tris: &[Tri]| {
            a != u32::MAX
                && b != u32::MAX
                && tris[a as usize].flat.dot(tris[b as usize].flat) < 0.82
        };
        // Tangent junctions (facets of a curved face) are not drawn as edges.
        let smooth = |a: u32, b: u32, tris: &[Tri]| {
            a != u32::MAX && b != u32::MAX && {
                let (ta, tb) = (&tris[a as usize], &tris[b as usize]);
                ta.flat.dot(tb.flat) > 0.996
            }
        };
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                let id = self.ids[i];
                if id == 0 {
                    continue;
                }
                let mut mark = false;
                for (dx, dy) in [(1i32, 0i32), (0, 1), (-1, 0), (0, -1)] {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx < 0 || ny < 0 || nx as usize >= w || ny as usize >= h {
                        continue;
                    }
                    let j = ny as usize * w + nx as usize;
                    let other = self.ids[j];
                    if other == 0 {
                        mark = true;
                    } else if (dx > 0 || dy > 0)
                        && ((other != id && !smooth(tri_of[i], tri_of[j], &self.tris))
                            || (self.faces[id as usize - 1].1 == 0
                                && crease(tri_of[i], tri_of[j], &self.tris)))
                    {
                        // Mark the nearer side only.
                        if self.depth[i] >= self.depth[j] - 1e-3 {
                            mark = true;
                        } else {
                            self.edge[j] = true;
                        }
                    }
                }
                if mark {
                    self.edge[i] = true;
                }
            }
        }
        self.compose_needed = true;
        self.dirty = false;
    }
    fn compose(&mut self, ctx: &egui::Context) {
        let [w, h] = self.size;
        let mut image = ColorImage::new([w, h], Color32::TRANSPARENT);
        let edges = matches!(self.style, Style::ShadedEdges | Style::HiddenLines);
        let wire = self.style == Style::Wireframe;
        let mut tint_of: Vec<Option<(Color32, f32)>> = vec![None; self.faces.len() + 1];
        for (fi, (b, tag)) in self.faces.iter().enumerate() {
            tint_of[fi + 1] = self
                .tints
                .iter()
                .find(|t| match t.pick {
                    Pick::Face(tb, tt) => tb == *b && tt == *tag,
                    Pick::Feature(tb, f) => tb == *b && part::feature_of(*tag) == f && *tag != 0,
                    Pick::Body(tb) => tb == *b,
                })
                .map(|t| (t.color, t.amount));
        }
        for i in 0..w * h {
            let id = self.ids[i];
            if id == 0 || wire {
                if wire {
                    if let Some((c, _)) = tint_of[id as usize].filter(|_| id != 0) {
                        image.pixels[i] = Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 60);
                    }
                }
                continue;
            }
            let mut c = self.shade[i];
            if let Some((t, a)) = tint_of[id as usize] {
                let mix = |x: u8, y: u8| (x as f32 * (1.0 - a) + y as f32 * a) as u8;
                c = Color32::from_rgb(mix(c.r(), t.r()), mix(c.g(), t.g()), mix(c.b(), t.b()));
            }
            if edges && self.edge[i] {
                c = Color32::from_rgb(28, 30, 34);
            }
            image.pixels[i] = c;
        }
        if let Some(t) = &mut self.texture {
            t.set(image, egui::TextureOptions::LINEAR);
        } else {
            self.texture =
                Some(ctx.load_texture("solidcraft-view", image, egui::TextureOptions::LINEAR));
        }
        self.compose_needed = false;
    }
    /// Lays out the viewport, handles navigation and draws the shaded scene.
    /// `version` must change whenever the meshes change.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        meshes: &[Mesh],
        colors: &[[u8; 3]],
        visible: &[bool],
        version: u64,
    ) -> egui::Response {
        let (rect, response) =
            ui.allocate_exact_size(ui.available_size(), egui::Sense::click_and_drag());
        if rect != self.rect {
            self.dirty = true;
        }
        self.rect = rect;
        let ctx = ui.ctx().clone();
        if let Some((from, to, start)) = self.anim {
            let t = ((ctx.input(|i| i.time) - start) / 0.35).clamp(0.0, 1.0);
            let s = t * t * (3.0 - 2.0 * t);
            let lerp = |a: Vec3, b: Vec3| a * (1.0 - s) + b * s;
            let (mut r, mut u) = (lerp(from[0], to[0]), lerp(from[1], to[1]));
            if r.length() < 0.1 || u.length() < 0.1 || t >= 1.0 {
                r = to[0];
                u = to[1];
            }
            let (r, u) = orthonormal(r, u);
            self.right = r;
            self.up = u;
            self.dirty = true;
            if t >= 1.0 {
                self.anim = None;
            } else {
                ctx.request_repaint();
            }
        }
        // Navigation: middle = orbit, Ctrl+middle or right = pan, Shift+middle = zoom, wheel = zoom.
        let (delta, mods) = ui.input(|i| (i.pointer.delta(), i.modifiers));
        if response.dragged_by(egui::PointerButton::Middle) {
            if mods.ctrl {
                self.pan += delta;
                self.dirty = true;
            } else if mods.shift {
                let c = self.rect.center();
                self.zoom_at(c, (-delta.y as f64 * 0.01).exp());
            } else {
                self.orbit(delta);
            }
        }
        if response.dragged_by(egui::PointerButton::Secondary) {
            self.pan += delta;
            self.dirty = true;
        }
        if self.left_orbit && response.dragged_by(egui::PointerButton::Primary) {
            if mods.ctrl {
                self.pan += delta;
                self.dirty = true;
            } else {
                self.orbit(delta);
            }
        }
        self.hover_pos = response.hover_pos();
        if let Some(p) = response.hover_pos() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.0 {
                self.zoom_at(p, (scroll as f64 * 0.0025).exp());
            }
        }
        if response.has_focus() || response.hovered() {
            let (l, r, u, d) = ui.input(|i| {
                (
                    i.key_pressed(egui::Key::ArrowLeft),
                    i.key_pressed(egui::Key::ArrowRight),
                    i.key_pressed(egui::Key::ArrowUp),
                    i.key_pressed(egui::Key::ArrowDown),
                )
            });
            if !ui.ctx().wants_keyboard_input() {
                let step = 15.0_f64.to_radians() / 0.008;
                let s = step as f32;
                if l {
                    self.orbit(Vec2::new(-s, 0.0));
                }
                if r {
                    self.orbit(Vec2::new(s, 0.0));
                }
                if u {
                    self.orbit(Vec2::new(0.0, -s));
                }
                if d {
                    self.orbit(Vec2::new(0.0, s));
                }
            }
        }
        let painter = ui.painter_at(rect);
        let mut background = egui::Mesh::default();
        for (pos, color) in [
            (rect.left_top(), Color32::from_rgb(214, 224, 238)),
            (rect.right_top(), Color32::from_rgb(214, 224, 238)),
            (rect.right_bottom(), Color32::from_rgb(252, 252, 253)),
            (rect.left_bottom(), Color32::from_rgb(252, 252, 253)),
        ] {
            background.vertices.push(egui::epaint::Vertex {
                pos,
                uv: egui::epaint::WHITE_UV,
                color,
            });
        }
        background.indices = vec![0, 1, 2, 0, 2, 3];
        painter.add(egui::Shape::mesh(background));
        if version != self.version {
            self.prepare(meshes, colors, visible);
            self.version = version;
            self.dirty = true;
        }
        let ppp = ctx.pixels_per_point();
        if (ppp - self.requested_ppp).abs() > 0.01 {
            self.requested_ppp = ppp;
            self.dirty = true;
        }
        if self.dirty {
            self.render(rect, ppp);
        }
        if self.compose_needed {
            self.compose(&ctx);
        }
        if let Some(t) = &self.texture {
            let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
            let shown = Rect::from_min_size(
                rect.min,
                Vec2::new(self.size[0] as f32, self.size[1] as f32) / self.ppp,
            );
            painter.image(t.id(), shown, uv, Color32::WHITE);
        }
        // Reference triad, bottom left.
        let origin = rect.left_bottom() + Vec2::new(48.0, -48.0);
        let (r, u, e) = self.basis();
        let mut axes = [
            (
                Vec3::new(1.0, 0.0, 0.0),
                "X",
                Color32::from_rgb(214, 54, 54),
            ),
            (
                Vec3::new(0.0, 1.0, 0.0),
                "Y",
                Color32::from_rgb(46, 160, 67),
            ),
            (
                Vec3::new(0.0, 0.0, 1.0),
                "Z",
                Color32::from_rgb(40, 100, 220),
            ),
        ];
        axes.sort_by(|a, b| a.0.dot(e).total_cmp(&b.0.dot(e)));
        for (v, label, color) in axes {
            let end = origin + Vec2::new(v.dot(r) as f32 * 30.0, -v.dot(u) as f32 * 30.0);
            painter.line_segment([origin, end], Stroke::new(2.5, color));
            painter.circle_filled(end, 2.5, color);
            painter.text(
                end + (end - origin).normalized() * 9.0,
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(11.0),
                color,
            );
        }
        response
    }
    /// Draws a 3D polyline, dimmed where a solid hides it.
    pub fn line3(&self, painter: &egui::Painter, pts: &[Vec3], stroke: Stroke, show_hidden: bool) {
        for w in pts.windows(2) {
            let n = ((self.project(w[0]) - self.project(w[1])).length() / 6.0)
                .ceil()
                .clamp(1.0, 200.0) as usize;
            for k in 0..n {
                let a = w[0] + (w[1] - w[0]) * (k as f64 / n as f64);
                let b = w[0] + (w[1] - w[0]) * ((k + 1) as f64 / n as f64);
                let visible = self.visible((a + b) * 0.5);
                if visible {
                    painter.line_segment([self.project(a), self.project(b)], stroke);
                } else if show_hidden {
                    let c = stroke.color;
                    painter.line_segment(
                        [self.project(a), self.project(b)],
                        Stroke::new(
                            stroke.width.min(1.2),
                            Color32::from_rgba_unmultiplied(c.r(), c.g(), c.b(), 70),
                        ),
                    );
                }
            }
        }
    }
}
