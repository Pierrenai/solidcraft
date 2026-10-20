//! A3 vector manufacturing sheets. Coordinates are millimetres on paper.
use crate::{
    assembly,
    geometry::{Mesh, Vec3},
    model::Project,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum View {
    Front,
    Top,
    Right,
    Iso,
}
impl View {
    pub fn label(self) -> &'static str {
        match self {
            Self::Front => "FACE",
            Self::Top => "DESSUS",
            Self::Right => "DROITE",
            Self::Iso => "ISOMÉTRIQUE",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Dimension {
    pub view: View,
    pub a: [f64; 2],
    pub b: [f64; 2],
    pub offset: f64,
    pub minus: f64,
    pub plus: f64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Drawing {
    pub title: String,
    pub author: String,
    pub revision: String,
    pub scale: f64,
    pub show_hidden: bool,
    pub auto_dimensions: bool,
    pub tolerance: f64,
    pub section: bool,
    pub section_z: f64,
    pub detail: bool,
    pub detail_center: [f64; 2],
    pub detail_scale: f64,
    pub dimensions: Vec<Dimension>,
}
impl Default for Drawing {
    fn default() -> Self {
        Self {
            title: String::new(),
            author: String::new(),
            revision: "A".into(),
            scale: 1.0,
            show_hidden: true,
            auto_dimensions: true,
            tolerance: 0.0,
            section: false,
            section_z: 4.0,
            detail: false,
            detail_center: [0.0, 0.0],
            detail_scale: 2.0,
            dimensions: vec![],
        }
    }
}
impl Drawing {
    pub fn validate(&self) -> Result<(), String> {
        if self.title.len() > 200
            || self.author.len() > 100
            || self.revision.len() > 40
            || !self.scale.is_finite()
            || !(0.01..=10.0).contains(&self.scale)
            || !self.tolerance.is_finite()
            || !(0.0..=100.0).contains(&self.tolerance)
            || !self.section_z.is_finite()
            || self.section_z.abs() > 5000.0
            || !self.detail_scale.is_finite()
            || !(0.01..=20.0).contains(&self.detail_scale)
            || self
                .detail_center
                .iter()
                .any(|v| !v.is_finite() || v.abs() > 5000.0)
            || self.dimensions.len() > 30
        {
            return Err("Cartouche, échelle ou paramètres de plan invalides.".into());
        }
        for d in &self.dimensions {
            if d.a
                .iter()
                .chain(&d.b)
                .chain([&d.offset, &d.minus, &d.plus])
                .any(|v| !v.is_finite() || v.abs() > 5000.0)
                || d.minus < 0.0
                || d.plus < 0.0
            {
                return Err("Cote ou tolérance invalide.".into());
            }
        }
        Ok(())
    }
}
#[derive(Clone, Debug)]
pub enum Element {
    Line {
        a: [f64; 2],
        b: [f64; 2],
        hidden: bool,
    },
    Text {
        at: [f64; 2],
        text: String,
        size: f64,
    },
    Rect {
        at: [f64; 2],
        size: [f64; 2],
    },
    Circle {
        at: [f64; 2],
        radius: f64,
    },
}
#[derive(Clone, Debug)]
pub struct Sheet {
    pub elements: Vec<Element>,
    pub notes: Vec<String>,
}
impl Sheet {
    fn line(&mut self, a: [f64; 2], b: [f64; 2]) {
        self.elements.push(Element::Line {
            a,
            b,
            hidden: false,
        });
    }
    fn text(&mut self, x: f64, y: f64, text: impl Into<String>, size: f64) {
        self.elements.push(Element::Text {
            at: [x, y],
            text: text.into(),
            size,
        });
    }
    fn rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        self.elements.push(Element::Rect {
            at: [x, y],
            size: [w, h],
        });
    }
    pub fn svg(&self) -> String {
        let mut svg=String::from("<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"420mm\" height=\"297mm\" viewBox=\"0 0 420 297\"><rect width=\"420\" height=\"297\" fill=\"white\"/><g stroke=\"#202b38\" stroke-width=\"0.25\" fill=\"none\" stroke-linecap=\"round\">");
        for e in &self.elements {
            match e{
            Element::Line{a,b,hidden}=>svg.push_str(&format!("<path d=\"M {:.4} {:.4} L {:.4} {:.4}\" {}/>",a[0],a[1],b[0],b[1],if *hidden{"stroke-dasharray=\"1.5 1\" stroke-width=\"0.15\" stroke=\"#78818d\""}else{""})),
            Element::Rect{at,size}=>svg.push_str(&format!("<rect x=\"{}\" y=\"{}\" width=\"{}\" height=\"{}\"/>",at[0],at[1],size[0],size[1])),
            Element::Circle{at,radius}=>svg.push_str(&format!("<circle cx=\"{}\" cy=\"{}\" r=\"{}\" fill=\"white\"/>",at[0],at[1],radius)),
            Element::Text{at,text,size}=>svg.push_str(&format!("<text x=\"{}\" y=\"{}\" font-family=\"Arial,sans-serif\" font-size=\"{}\" fill=\"#202b38\" stroke=\"none\">{}</text>",at[0],at[1],size,escape(text))),
        }
        }
        svg.push_str("</g></svg>");
        svg
    }
}
fn escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}
fn basis(view: View) -> (Vec3, Vec3, Vec3) {
    match view {
        View::Front => (
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(0.0, -1.0, 0.0),
        ),
        View::Top => (
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
        ),
        View::Right => (
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec3::new(1.0, 0.0, 0.0),
        ),
        View::Iso => {
            let eye = Vec3::new(1.0, -1.5, 1.0).normalized();
            let right = Vec3::new(0.0, 0.0, 1.0).cross(eye).normalized();
            (right, eye.cross(right), eye)
        }
    }
}
fn project(p: Vec3, view: View) -> [f64; 2] {
    let (r, u, _) = basis(view);
    [p.dot(r), p.dot(u)]
}
type PointKey = (i64, i64, i64);
fn key(p: Vec3) -> PointKey {
    (
        (p.x * 1e6).round() as i64,
        (p.y * 1e6).round() as i64,
        (p.z * 1e6).round() as i64,
    )
}
struct Edge {
    a: Vec3,
    b: Vec3,
    normals: Vec<Vec3>,
}
fn edges(meshes: &[Mesh]) -> Vec<Edge> {
    let mut map: HashMap<(PointKey, PointKey), Edge> = HashMap::new();
    for mesh in meshes {
        for p in &mesh.polygons {
            for i in 0..p.vertices.len() {
                let a = p.vertices[i];
                let b = p.vertices[(i + 1) % p.vertices.len()];
                if (a - b).length() < 1e-7 {
                    continue;
                }
                let (ka, kb) = if key(a) < key(b) {
                    (key(a), key(b))
                } else {
                    (key(b), key(a))
                };
                map.entry((ka, kb))
                    .and_modify(|e| {
                        e.normals.push(p.normal);
                    })
                    .or_insert(Edge {
                        a,
                        b,
                        normals: vec![p.normal],
                    });
            }
        }
    }
    let mut edges: Vec<_> = map.into_values().collect();
    edges.sort_by_key(|e| (key(e.a), key(e.b)));
    // Group collinear edges by Plücker line coordinates to resolve BSP T-junctions in near-linear time.
    let line_key = |e: &Edge| {
        let mut dir = (e.b - e.a).normalized();
        if dir.x < -1e-8
            || (dir.x.abs() < 1e-8 && dir.y < -1e-8)
            || (dir.x.abs() < 1e-8 && dir.y.abs() < 1e-8 && dir.z < 0.0)
        {
            dir = -dir;
        }
        let moment = e.a.cross(dir);
        [
            (dir.x * 1e5).round() as i64,
            (dir.y * 1e5).round() as i64,
            (dir.z * 1e5).round() as i64,
            (moment.x * 1e4).round() as i64,
            (moment.y * 1e4).round() as i64,
            (moment.z * 1e4).round() as i64,
        ]
    };
    let mut lines: HashMap<[i64; 6], Vec<usize>> = HashMap::new();
    for (i, e) in edges.iter().enumerate() {
        lines.entry(line_key(e)).or_default().push(i);
    }
    for i in 0..edges.len() {
        if edges[i].normals.len() > 1 {
            continue;
        }
        let mid = (edges[i].a + edges[i].b) * 0.5;
        let mut added = Vec::new();
        if let Some(candidates) = lines.get(&line_key(&edges[i])) {
            for &j in candidates {
                if i == j {
                    continue;
                }
                let e = &edges[j];
                let delta = e.b - e.a;
                let t = (mid - e.a).dot(delta) / delta.dot(delta);
                if (-1e-6..=1.000001).contains(&t) && (mid - (e.a + delta * t)).length() < 1e-5 {
                    added.extend(e.normals.iter().copied());
                }
            }
        }
        edges[i].normals.extend(added);
    }
    edges
}
fn occluded(p: Vec3, eye: Vec3, triangles: &[[Vec3; 3]]) -> bool {
    for t in triangles {
        let e1 = t[1] - t[0];
        let e2 = t[2] - t[0];
        let h = eye.cross(e2);
        let det = e1.dot(h);
        if det.abs() < 1e-10 {
            continue;
        }
        let inv = 1.0 / det;
        let s = p - t[0];
        let u = s.dot(h) * inv;
        if !(-1e-7..=1.0000001).contains(&u) {
            continue;
        }
        let q = s.cross(e1);
        let v = eye.dot(q) * inv;
        if v < -1e-7 || u + v > 1.0000001 {
            continue;
        }
        if e2.dot(q) * inv > 1e-5 {
            return true;
        }
    }
    false
}
// Bounding hierarchy for hidden-line rays; avoids scanning the whole assembly for each edge.
struct RayTree {
    min: Vec3,
    max: Vec3,
    triangles: Vec<[Vec3; 3]>,
    children: Option<Box<[RayTree; 2]>>,
}
impl RayTree {
    fn new(mut triangles: Vec<[Vec3; 3]>) -> Self {
        let mut min = Vec3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
        let mut max = -min;
        for p in triangles.iter().flatten() {
            min.x = min.x.min(p.x);
            min.y = min.y.min(p.y);
            min.z = min.z.min(p.z);
            max.x = max.x.max(p.x);
            max.y = max.y.max(p.y);
            max.z = max.z.max(p.z);
        }
        if triangles.len() <= 12 {
            return Self {
                min,
                max,
                triangles,
                children: None,
            };
        }
        let extent = (max - min).array();
        let axis = (0..3)
            .max_by(|&i, &j| extent[i].total_cmp(&extent[j]))
            .unwrap_or(0);
        triangles.sort_by(|a, b| {
            let ca = (a[0] + a[1] + a[2]).array()[axis];
            let cb = (b[0] + b[1] + b[2]).array()[axis];
            ca.total_cmp(&cb)
        });
        let other = triangles.split_off(triangles.len() / 2);
        Self {
            min,
            max,
            triangles: vec![],
            children: Some(Box::new([Self::new(triangles), Self::new(other)])),
        }
    }
    fn blocked(&self, p: Vec3, dir: Vec3) -> bool {
        let mut lo = 0.0_f64;
        let mut hi = f64::INFINITY;
        for i in 0..3 {
            let o = p.array()[i];
            let d = dir.array()[i];
            let min = self.min.array()[i] - 1e-7;
            let max = self.max.array()[i] + 1e-7;
            if d.abs() < 1e-12 {
                if o < min || o > max {
                    return false;
                }
            } else {
                let a = (min - o) / d;
                let b = (max - o) / d;
                lo = lo.max(a.min(b));
                hi = hi.min(a.max(b));
                if hi < lo {
                    return false;
                }
            }
        }
        if hi < 1e-5 {
            return false;
        }
        if let Some(nodes) = &self.children {
            nodes[0].blocked(p, dir) || nodes[1].blocked(p, dir)
        } else {
            occluded(p, dir, &self.triangles)
        }
    }
}
fn clip(a: [f64; 2], b: [f64; 2], rect: [f64; 4]) -> Option<([f64; 2], [f64; 2])> {
    let d = [b[0] - a[0], b[1] - a[1]];
    let mut lo = 0.0_f64;
    let mut hi = 1.0_f64;
    for (p, q) in [
        (-d[0], a[0] - rect[0]),
        (d[0], rect[2] - a[0]),
        (-d[1], a[1] - rect[1]),
        (d[1], rect[3] - a[1]),
    ] {
        if p.abs() < 1e-12 {
            if q < 0.0 {
                return None;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                lo = lo.max(r);
            } else {
                hi = hi.min(r);
            }
        }
    }
    (lo <= hi).then_some((
        [a[0] + lo * d[0], a[1] + lo * d[1]],
        [a[0] + hi * d[0], a[1] + hi * d[1]],
    ))
}
fn dimension(sheet: &mut Sheet, a: [f64; 2], b: [f64; 2], offset: f64, label: String) {
    let d = [b[0] - a[0], b[1] - a[1]];
    let length = d[0].hypot(d[1]);
    if length < 1e-6 {
        return;
    }
    let u = [d[0] / length, d[1] / length];
    let n = [-u[1], u[0]];
    let aa = [a[0] + n[0] * offset, a[1] + n[1] * offset];
    let bb = [b[0] + n[0] * offset, b[1] + n[1] * offset];
    sheet.line(a, [aa[0] + n[0] * 2.0, aa[1] + n[1] * 2.0]);
    sheet.line(b, [bb[0] + n[0] * 2.0, bb[1] + n[1] * 2.0]);
    sheet.line(aa, bb);
    for (p, s) in [(aa, 1.0), (bb, -1.0)] {
        for side in [-1.0, 1.0] {
            sheet.line(
                p,
                [
                    p[0] + u[0] * 2.0 * s + n[0] * 0.65 * side,
                    p[1] + u[1] * 2.0 * s + n[1] * 0.65 * side,
                ],
            );
        }
    }
    sheet.text(
        (aa[0] + bb[0]) / 2.0 - 4.0,
        (aa[1] + bb[1]) / 2.0 - 1.5,
        label,
        2.8,
    );
}
pub fn generate(project_data: &Project, meshes: &[Mesh]) -> Result<Sheet, String> {
    let config = &project_data.drawing;
    config.validate()?;
    if meshes.is_empty() {
        return Err("Ajoutez une pièce avant de créer une mise en plan.".into());
    }
    if meshes.iter().map(|m| m.polygons.len()).sum::<usize>() > 50_000 {
        return Err("Plan limité à 50 000 faces. Utilisez un sous-ensemble.".into());
    }
    let edge_list = edges(meshes);
    let triangles: Vec<_> = meshes.iter().flat_map(Mesh::triangles).collect();
    let rays = RayTree::new(triangles.clone());
    let mut sheet = Sheet {
        elements: vec![],
        notes: vec![],
    };
    sheet.rect(8.0, 8.0, 404.0, 281.0);
    let views = [
        (View::Front, [80.0, 65.0]),
        (View::Top, [220.0, 65.0]),
        (View::Right, [80.0, 177.0]),
        (View::Iso, [220.0, 177.0]),
    ];
    for (view, center) in views {
        let projection_view = if view == View::Iso && config.section {
            View::Top
        } else {
            view
        };
        let mut min = [f64::INFINITY; 2];
        let mut max = [f64::NEG_INFINITY; 2];
        for p in triangles.iter().flatten() {
            let q = project(*p, projection_view);
            for i in 0..2 {
                min[i] = min[i].min(q[i]);
                max[i] = max[i].max(q[i]);
            }
        }
        let mid = [(min[0] + max[0]) / 2.0, (min[1] + max[1]) / 2.0];
        let scale = config.scale;
        if (max[0] - min[0]) * scale > 112.0 || (max[1] - min[1]) * scale > 77.0 {
            return Err(format!(
                "La vue {} dépasse sa zone à cette échelle. Réduisez l’échelle du plan.",
                view.label()
            ));
        }
        let screen = |q: [f64; 2]| {
            [
                center[0] + (q[0] - mid[0]) * scale,
                center[1] - (q[1] - mid[1]) * scale,
            ]
        };
        let (_, _, eye) = basis(view);
        if view == View::Iso && config.section {
            // True intersection with a horizontal plane; segments are not projected outlines.
            let cuts = section_segments(&triangles, config.section_z);
            if cuts.is_empty() {
                return Err("Le plan de coupe ne traverse aucun solide.".into());
            }
            for (a, b) in &cuts {
                sheet.line(screen([a.x, a.y]), screen([b.x, b.y]));
            }
            // Hatch intervals determined by even/odd intersections of the section boundary.
            let min_y = cuts
                .iter()
                .flat_map(|(a, b)| [a.y, b.y])
                .fold(f64::INFINITY, f64::min);
            let max_y = cuts
                .iter()
                .flat_map(|(a, b)| [a.y, b.y])
                .fold(f64::NEG_INFINITY, f64::max);
            let mut y = min_y + 1.0;
            let mut count = 0;
            while y < max_y && count < 500 {
                let mut xs: Vec<_> = cuts
                    .iter()
                    .filter_map(|(a, b)| {
                        if (a.y <= y && b.y > y) || (b.y <= y && a.y > y) {
                            Some(a.x + (b.x - a.x) * (y - a.y) / (b.y - a.y))
                        } else {
                            None
                        }
                    })
                    .collect();
                xs.sort_by(f64::total_cmp);
                for pair in xs.chunks_exact(2) {
                    sheet.line(screen([pair[0], y]), screen([pair[1], y]));
                }
                y += 2.0 / scale;
                count += 1;
            }
            sheet.text(
                center[0] - 35.0,
                center[1] + 49.0,
                format!("COUPE A-A · Z = {:.2}", config.section_z),
                3.0,
            );
        } else {
            for edge in &edge_list {
                let silhouette = edge.normals.iter().any(|n| n.dot(eye) > 1e-6)
                    && edge.normals.iter().any(|n| n.dot(eye) < -1e-6);
                let crease = edge
                    .normals
                    .iter()
                    .any(|a| edge.normals.iter().any(|b| a.dot(*b) < 0.95));
                if edge.normals.len() > 1 && !silhouette && !crease {
                    continue;
                }
                let hidden = rays.blocked((edge.a + edge.b) * 0.5, eye);
                if hidden && !config.show_hidden {
                    continue;
                }
                sheet.elements.push(Element::Line {
                    a: screen(project(edge.a, view)),
                    b: screen(project(edge.b, view)),
                    hidden,
                });
            }
            sheet.text(center[0] - 20.0, center[1] + 49.0, view.label(), 3.0);
        }
        if config.auto_dimensions && view != View::Iso {
            let label = |v: f64| {
                if config.tolerance > 0.0 {
                    format!("{v:.2} ±{:.2}", config.tolerance)
                } else {
                    format!("{v:.2}")
                }
            };
            dimension(
                &mut sheet,
                screen([min[0], min[1]]),
                screen([max[0], min[1]]),
                8.0,
                label(max[0] - min[0]),
            );
            dimension(
                &mut sheet,
                screen([min[0], max[1]]),
                screen([min[0], min[1]]),
                8.0,
                label(max[1] - min[1]),
            );
        }
        for d in &config.dimensions {
            if d.view == view {
                let value = (d.b[0] - d.a[0]).hypot(d.b[1] - d.a[1]);
                dimension(
                    &mut sheet,
                    screen(d.a),
                    screen(d.b),
                    d.offset,
                    format!("{value:.2} +{:.2}/-{:.2}", d.plus, d.minus),
                );
            }
        }
        // Component balloons use the same references as the BOM, once per component.
        if view == View::Iso && !config.section {
            for row in assembly::bom(project_data) {
                for id in row.ids {
                    if let Some(i) = project_data.bodies.iter().position(|b| b.id == id) {
                        if let Some((a, b)) = meshes[i].bounds() {
                            let c = screen(project((a + b) * 0.5, view));
                            let out = [c[0] + 9.0, c[1] - 9.0];
                            sheet.line(c, out);
                            sheet.elements.push(Element::Circle {
                                at: out,
                                radius: 3.0,
                            });
                            sheet.text(out[0] - 0.9, out[1] + 1.0, row.number.to_string(), 2.7);
                        }
                    }
                }
            }
        }
    }
    sheet.rect(306.0, 18.0, 100.0, 105.0);
    sheet.text(310.0, 24.0, "NOMENCLATURE", 3.3);
    sheet.line([306.0, 28.0], [406.0, 28.0]);
    sheet.text(309.0, 33.0, "REP.  RÉFÉRENCE / DÉSIGNATION", 2.5);
    sheet.text(394.0, 33.0, "QTÉ", 2.5);
    let rows = assembly::bom(project_data);
    if rows.len() > 10 {
        return Err(
            "La nomenclature dépasse 10 lignes sur cette feuille A3. Exportez un sous-ensemble."
                .into(),
        );
    }
    for (row, i) in rows.iter().zip(0..) {
        let y = 40.0 + i as f64 * 8.0;
        sheet.text(309.0, y, row.number.to_string(), 2.5);
        sheet.text(
            318.0,
            y,
            row.reference.chars().take(22).collect::<String>(),
            2.5,
        );
        sheet.text(394.0, y, row.quantity.to_string(), 2.5);
        sheet.text(
            318.0,
            y + 3.0,
            row.name.chars().take(28).collect::<String>(),
            2.0,
        );
    }
    if config.detail {
        let rect = [310.0, 143.0, 402.0, 221.0];
        sheet.rect(310.0, 143.0, 92.0, 78.0);
        let screen = |p: Vec3| {
            let q = project(p, View::Front);
            [
                356.0 + (q[0] - config.detail_center[0]) * config.detail_scale,
                182.0 - (q[1] - config.detail_center[1]) * config.detail_scale,
            ]
        };
        for edge in &edge_list {
            if let Some((a, b)) = clip(screen(edge.a), screen(edge.b), rect) {
                sheet.line(a, b);
            }
        }
        sheet.text(
            312.0,
            229.0,
            format!("DÉTAIL B · échelle {:.2}:1", config.detail_scale),
            3.0,
        );
    }
    sheet.text(308.0, 240.0, "Unités : mm · angles : degrés", 2.6);
    sheet.text(308.0, 246.0, "Projection orthographique · A3", 2.6);
    sheet.rect(8.0, 254.0, 404.0, 35.0);
    sheet.line([8.0, 270.0], [412.0, 270.0]);
    sheet.line([270.0, 254.0], [270.0, 289.0]);
    sheet.line([340.0, 254.0], [340.0, 289.0]);
    let title = if config.title.is_empty() {
        &project_data.name
    } else {
        &config.title
    };
    sheet.text(13.0, 262.0, title.chars().take(65).collect::<String>(), 4.2);
    sheet.text(13.0, 280.0, format!("Auteur : {}", config.author), 3.0);
    sheet.text(275.0, 262.0, format!("Échelle {:.3}:1", config.scale), 3.0);
    sheet.text(275.0, 280.0, format!("Indice : {}", config.revision), 3.0);
    sheet.text(345.0, 262.0, "SOLIDCRAFT", 4.0);
    sheet.text(345.0, 280.0, "A3 · Feuille 1 / 1", 3.0);
    sheet.notes.push("Traits cachés évalués au milieu des arêtes ; vérifier les vues complexes avant fabrication.".into());
    Ok(sheet)
}
pub fn section_segments(triangles: &[[Vec3; 3]], z: f64) -> Vec<(Vec3, Vec3)> {
    let mut out = Vec::new();
    for t in triangles {
        let mut hits = Vec::new();
        for i in 0..3 {
            let a = t[i];
            let b = t[(i + 1) % 3];
            if (a.z <= z && b.z > z) || (b.z <= z && a.z > z) {
                let p = a + (b - a) * ((z - a.z) / (b.z - a.z));
                if hits.iter().all(|q: &Vec3| (*q - p).length() > 1e-6) {
                    hits.push(p);
                }
            }
        }
        if hits.len() == 2 {
            out.push((hits[0], hits[1]));
        }
    }
    out
}
pub fn bom_csv(project: &Project) -> String {
    let quoted = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
    let mut out = String::from("\u{feff}Repère;Référence;Désignation;Matière;Quantité\r\n");
    for r in assembly::bom(project) {
        out.push_str(&format!(
            "{};{};{};{};{}\r\n",
            r.number,
            quoted(&r.reference),
            quoted(&r.name),
            quoted(&r.material),
            r.quantity
        ));
    }
    out
}
