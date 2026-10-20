//! Parameterized mesh features. Restrictions are checked before constructing geometry.
use crate::{
    geometry::{self, BooleanOp, Mesh, Polygon, Vec3},
    model::{Body, Shape},
    sketch::Sketch,
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum FinishKind {
    Fillet,
    Chamfer,
}
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum HoleKind {
    Simple,
    Counterbore,
    Countersink,
    Threaded,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct HoleSpec {
    pub kind: HoleKind,
    pub diameter: f64,
    pub x: f64,
    pub y: f64,
    pub depth: f64,
    pub through: bool,
    pub head_diameter: f64,
    pub head_depth: f64,
    pub pitch: f64,
}
impl Default for HoleSpec {
    fn default() -> Self {
        Self {
            kind: HoleKind::Simple,
            diameter: 6.0,
            x: 0.0,
            y: 0.0,
            depth: 10.0,
            through: true,
            head_diameter: 10.0,
            head_depth: 3.0,
            pitch: 1.0,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Section {
    pub profile: Sketch,
    pub z: f64,
    pub offset: [f64; 2],
    pub twist: f64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Feature {
    FaceExtrusion {
        source: Box<Body>,
        origin: [f64; 3],
        u: [f64; 3],
        v: [f64; 3],
        sketch: Sketch,
        depth: f64,
        cut: bool,
    },
    Bossage {
        sketch: Sketch,
        height: f64,
    },
    Pocket {
        source: Box<Body>,
        sketch: Sketch,
        depth: f64,
        through: bool,
    },
    Revolve {
        profile: Sketch,
        angle: f64,
    },
    Sweep {
        profile: Sketch,
        path: Vec<[f64; 3]>,
    },
    Loft {
        sections: Vec<Section>,
    },
    Finish {
        source: Box<Body>,
        kind: FinishKind,
        size: f64,
    },
    Shell {
        source: Box<Body>,
        thickness: f64,
    },
    Hole {
        source: Box<Body>,
        spec: HoleSpec,
    },
}
impl Feature {
    pub fn label(&self) -> &'static str {
        match self {
            Self::FaceExtrusion { cut, .. } => {
                if *cut {
                    "Découpe sur face"
                } else {
                    "Bossage sur face"
                }
            }
            Self::Bossage { .. } => "Bossage paramétrique",
            Self::Pocket { .. } => "Enlèvement de matière",
            Self::Revolve { .. } => "Révolution",
            Self::Sweep { .. } => "Balayage",
            Self::Loft { .. } => "Lissage",
            Self::Finish {
                kind: FinishKind::Fillet,
                ..
            } => "Congés verticaux",
            Self::Finish { .. } => "Chanfreins verticaux",
            Self::Shell { .. } => "Coque ouverte",
            Self::Hole { .. } => "Assistant de perçage",
        }
    }
    pub fn source(&self) -> Option<&Body> {
        match self {
            Self::FaceExtrusion { source, .. }
            | Self::Pocket { source, .. }
            | Self::Finish { source, .. }
            | Self::Shell { source, .. }
            | Self::Hole { source, .. } => Some(source),
            _ => None,
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        let positive = |v: f64| v.is_finite() && (0.1..=5000.0).contains(&v);
        match self {
            Self::FaceExtrusion {
                origin,
                u,
                v,
                sketch,
                depth,
                ..
            } => {
                sketch.sample()?;
                let a = Vec3::from_array(*u);
                let b = Vec3::from_array(*v);
                if !positive(*depth)
                    || origin
                        .iter()
                        .chain(u)
                        .chain(v)
                        .any(|x| !x.is_finite() || x.abs() > 5000.0)
                    || (a.length() - 1.0).abs() > 1e-6
                    || (b.length() - 1.0).abs() > 1e-6
                    || a.dot(b).abs() > 1e-6
                {
                    return Err("Plan d’esquisse ou profondeur invalide.".into());
                }
            }
            Self::Bossage { sketch, height } => {
                sketch.sample()?;
                if !positive(*height) {
                    return Err("Hauteur invalide.".into());
                }
            }
            Self::Pocket { sketch, depth, .. } => {
                sketch.sample()?;
                if !positive(*depth) {
                    return Err("Profondeur invalide.".into());
                }
            }
            Self::Revolve { profile, angle } => {
                let p = profile.sample()?;
                if !angle.is_finite()
                    || !(1.0..=360.0).contains(angle)
                    || p.iter().any(|p| p[0] < 0.0)
                {
                    return Err(
                        "Révolution : profil R/Z avec rayons positifs, angle de 1 à 360°.".into(),
                    );
                }
            }
            Self::Sweep { profile, path } => {
                profile.sample()?;
                if !(2..=32).contains(&path.len())
                    || path
                        .iter()
                        .flatten()
                        .any(|v| !v.is_finite() || v.abs() > 5000.0)
                {
                    return Err("Trajectoire invalide : 2 à 32 points.".into());
                }
                for pair in path.windows(2) {
                    if (Vec3::from_array(pair[1]) - Vec3::from_array(pair[0])).length() < 0.1 {
                        return Err("Deux points de trajectoire sont confondus.".into());
                    }
                }
            }
            Self::Loft { sections } => {
                if !(2..=8).contains(&sections.len()) {
                    return Err("Un lissage demande 2 à 8 sections.".into());
                }
                for s in sections {
                    s.profile.sample()?;
                    if !s.z.is_finite()
                        || s.z.abs() > 5000.0
                        || !s.twist.is_finite()
                        || s.twist.abs() > 360.0
                        || s.offset.iter().any(|v| !v.is_finite() || v.abs() > 5000.0)
                    {
                        return Err("Section invalide.".into());
                    }
                }
                if sections.windows(2).any(|w| w[1].z - w[0].z < 0.1) {
                    return Err("Les sections doivent être ordonnées suivant Z.".into());
                }
            }
            Self::Finish { source, size, .. } => {
                if let Shape::Box { width, depth, .. } = source.shape {
                    if !positive(*size) || *size >= width.min(depth) / 2.0 {
                        return Err("Rayon/chanfrein trop grand pour le bloc.".into());
                    }
                } else {
                    return Err("Cette version traite les quatre arêtes verticales d’un bloc. Sélectionnez un bloc primitif.".into());
                }
            }
            Self::Shell { source, thickness } => {
                if !positive(*thickness) {
                    return Err("Épaisseur invalide.".into());
                }
                let valid = match source.shape {
                    Shape::Box {
                        width,
                        depth,
                        height,
                    } => 2.0 * thickness < width.min(depth) && *thickness < height,
                    Shape::Cylinder { radius, height } => *thickness < radius.min(height),
                    _ => false,
                };
                if !valid {
                    return Err("Coque ouverte +Z : bloc ou cylindre primitif, épaisseur inférieure aux dimensions.".into());
                }
            }
            Self::Hole { spec, .. } => {
                if !positive(spec.diameter)
                    || !positive(spec.depth)
                    || ![spec.x, spec.y]
                        .iter()
                        .all(|v| v.is_finite() && v.abs() <= 5000.0)
                {
                    return Err("Dimensions de perçage invalides.".into());
                }
                if matches!(spec.kind, HoleKind::Counterbore | HoleKind::Countersink)
                    && (!positive(spec.head_diameter)
                        || spec.head_diameter <= spec.diameter
                        || !positive(spec.head_depth))
                {
                    return Err("Le diamètre de tête doit dépasser celui du trou ; profondeur positive requise.".into());
                }
                if spec.kind == HoleKind::Threaded
                    && (!positive(spec.pitch) || spec.pitch >= spec.diameter)
                {
                    return Err("Pas de taraudage invalide.".into());
                }
            }
        }
        Ok(())
    }
    pub fn mesh(&self) -> Result<Mesh, String> {
        self.validate()?;
        match self {
            Self::FaceExtrusion {
                source,
                origin,
                u,
                v,
                sketch,
                depth,
                cut,
            } => {
                let a = Vec3::from_array(*u);
                let b = Vec3::from_array(*v);
                let n = a.cross(b);
                let origin = Vec3::from_array(*origin);
                let tool = geometry::extrude(&sketch.sample()?, *depth)?;
                let polygons = tool
                    .polygons
                    .into_iter()
                    .filter_map(|p| {
                        let mut vertices: Vec<_> = p
                            .vertices
                            .into_iter()
                            .map(|p| {
                                origin + a * p.x + b * p.y + n * (if *cut { -p.z } else { p.z })
                            })
                            .collect();
                        if *cut {
                            vertices.reverse();
                        }
                        Polygon::new(vertices)
                    })
                    .collect();
                let base = source.mesh()?;
                let tool = Mesh { polygons };
                let overlap = base.boolean(&tool, BooleanOp::Intersect);
                if *cut && overlap.as_ref().map_or(true, |m| m.volume() < 1e-6) {
                    return Err("Le contour ne coupe pas la pièce.".into());
                }
                base.boolean(
                    &tool,
                    if *cut {
                        BooleanOp::Subtract
                    } else {
                        BooleanOp::Union
                    },
                )
            }
            Self::Bossage { sketch, height } => geometry::extrude(&sketch.sample()?, *height),
            Self::Pocket {
                source,
                sketch,
                depth,
                through,
            } => {
                let mesh = source.mesh()?;
                let (min, max) = mesh.bounds().ok_or("Solide vide")?;
                let d = if *through {
                    max.z - min.z + 2.0
                } else {
                    *depth
                };
                let cut = geometry::extrude(&sketch.sample()?, d + 1.0)?
                    .transformed([0.0, 0.0, max.z - d], [0.0; 3]);
                mesh.boolean(&cut, BooleanOp::Subtract)
            }
            Self::Revolve { profile, angle } => revolve(&profile.sample()?, *angle),
            Self::Sweep { profile, path } => sweep(&profile.sample()?, path),
            Self::Loft { sections } => loft(sections),
            Self::Finish { source, kind, size } => {
                let Shape::Box {
                    width,
                    depth,
                    height,
                } = source.shape
                else {
                    return Err("Bloc requis.".into());
                };
                let points = rounded_rectangle(width, depth, *size, *kind);
                let mut result = geometry::extrude(&points, height)?;
                for h in &source.holes {
                    let cut = geometry::cylinder(h.diameter / 2.0, height + 2.0)
                        .transformed([h.x, h.y, -1.0], [0.0; 3]);
                    result = result.boolean(&cut, BooleanOp::Subtract)?;
                }
                Ok(result.transformed(source.position, source.rotation))
            }
            Self::Shell { source, thickness } => {
                let outer = source.mesh()?;
                let t = *thickness;
                let cavity = match source.shape {
                    Shape::Box {
                        width,
                        depth,
                        height,
                    } => geometry::cuboid(width - 2.0 * t, depth - 2.0 * t, height - t + 1.0)?,
                    Shape::Cylinder { radius, height } => {
                        geometry::cylinder(radius - t, height - t + 1.0)
                    }
                    _ => return Err("Bloc ou cylindre requis.".into()),
                }
                .transformed([0.0, 0.0, t], [0.0; 3])
                .transformed(source.position, source.rotation);
                outer.boolean(&cavity, BooleanOp::Subtract)
            }
            Self::Hole { source, spec } => {
                let mesh = source.mesh()?;
                let (min, max) = mesh.bounds().ok_or("Solide vide")?;
                let d = if spec.through {
                    max.z - min.z + 2.0
                } else {
                    spec.depth
                };
                // Thread callouts are cosmetic, as in a tapped-hole feature: geometry is a pilot bore.
                let diameter = if spec.kind == HoleKind::Threaded {
                    spec.diameter - spec.pitch
                } else {
                    spec.diameter
                };
                let cut = geometry::cylinder(diameter / 2.0, d + 1.0)
                    .transformed([spec.x, spec.y, max.z - d], [0.0; 3]);
                let mut result = mesh.boolean(&cut, BooleanOp::Subtract)?;
                if spec.kind == HoleKind::Counterbore {
                    let cut = geometry::cylinder(spec.head_diameter / 2.0, spec.head_depth + 1.0)
                        .transformed([spec.x, spec.y, max.z - spec.head_depth], [0.0; 3]);
                    result = result.boolean(&cut, BooleanOp::Subtract)?;
                } else if spec.kind == HoleKind::Countersink {
                    let profile = vec![
                        [0.0, 0.0],
                        [spec.diameter / 2.0, 0.0],
                        [spec.head_diameter / 2.0, spec.head_depth],
                        [spec.head_diameter / 2.0, spec.head_depth + 1.0],
                        [0.0, spec.head_depth + 1.0],
                    ];
                    let cut = revolve(&profile, 360.0)?
                        .transformed([spec.x, spec.y, max.z - spec.head_depth], [0.0; 3]);
                    result = result.boolean(&cut, BooleanOp::Subtract)?;
                }
                Ok(result)
            }
        }
    }
}
fn ccw(points: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let mut p = points.to_vec();
    let area: f64 = (0..p.len())
        .map(|i| p[i][0] * p[(i + 1) % p.len()][1] - p[(i + 1) % p.len()][0] * p[i][1])
        .sum();
    if area < 0.0 {
        p.reverse();
    }
    p
}
fn push(poly: &mut Vec<Polygon>, v: Vec<Vec3>) {
    if let Some(p) = Polygon::new(v) {
        poly.push(p);
    }
}
pub fn revolve(points: &[[f64; 2]], degrees: f64) -> Result<Mesh, String> {
    let points = ccw(points);
    let triangles = geometry::triangulate(&points)?;
    let full = (degrees - 360.0).abs() < 1e-8;
    let steps = (96.0 * degrees / 360.0).ceil().max(3.0) as usize;
    let point = |i: usize, j: usize| {
        let a = degrees.to_radians() * i as f64 / steps as f64;
        Vec3::new(points[j][0] * a.cos(), points[j][0] * a.sin(), points[j][1])
    };
    let mut polygons = Vec::new();
    for i in 0..steps {
        for j in 0..points.len() {
            let k = (j + 1) % points.len();
            push(
                &mut polygons,
                vec![point(i, j), point(i + 1, j), point(i + 1, k), point(i, k)],
            );
        }
    }
    if !full {
        for t in triangles {
            push(
                &mut polygons,
                vec![point(0, t[0]), point(0, t[1]), point(0, t[2])],
            );
            push(
                &mut polygons,
                vec![point(steps, t[2]), point(steps, t[1]), point(steps, t[0])],
            );
        }
    }
    Ok(Mesh { polygons })
}
fn resample(points: &[[f64; 2]], n: usize) -> Vec<[f64; 2]> {
    let p = ccw(points);
    let lengths: Vec<_> = (0..p.len())
        .map(|i| (p[i][0] - p[(i + 1) % p.len()][0]).hypot(p[i][1] - p[(i + 1) % p.len()][1]))
        .collect();
    let total: f64 = lengths.iter().sum();
    (0..n)
        .map(|i| {
            let mut d = total * i as f64 / n as f64;
            let mut j = 0;
            while j + 1 < p.len() && d > lengths[j] {
                d -= lengths[j];
                j += 1;
            }
            let t = d / lengths[j].max(1e-10);
            let b = p[(j + 1) % p.len()];
            [
                p[j][0] + (b[0] - p[j][0]) * t,
                p[j][1] + (b[1] - p[j][1]) * t,
            ]
        })
        .collect()
}
fn skin(rings: &[Vec<Vec3>], profile: &[[f64; 2]]) -> Result<Mesh, String> {
    let mut polygons = Vec::new();
    let n = profile.len();
    for k in 0..rings.len() - 1 {
        for i in 0..n {
            let j = (i + 1) % n;
            let a = rings[k][i];
            let b = rings[k][j];
            let c = rings[k + 1][j];
            let d = rings[k + 1][i];
            push(&mut polygons, vec![a, b, c]);
            push(&mut polygons, vec![a, c, d]);
        }
    }
    for t in geometry::triangulate(profile)? {
        push(
            &mut polygons,
            vec![rings[0][t[2]], rings[0][t[1]], rings[0][t[0]]],
        );
        let last = rings.last().unwrap();
        push(&mut polygons, vec![last[t[0]], last[t[1]], last[t[2]]]);
    }
    Ok(Mesh { polygons })
}
pub fn loft(sections: &[Section]) -> Result<Mesh, String> {
    let profiles: Vec<_> = sections
        .iter()
        .map(|s| s.profile.sample().map(|p| resample(&p, 64)))
        .collect::<Result<_, _>>()?;
    let rings: Vec<_> = sections
        .iter()
        .zip(&profiles)
        .map(|(s, p)| {
            p.iter()
                .map(|p| {
                    Vec3::new(p[0], p[1], 0.0).rotate([0.0, 0.0, s.twist])
                        + Vec3::new(s.offset[0], s.offset[1], s.z)
                })
                .collect()
        })
        .collect();
    skin(&rings, &profiles[0])
}
pub fn sweep(profile: &[[f64; 2]], path: &[[f64; 3]]) -> Result<Mesh, String> {
    let profile = ccw(profile);
    let path: Vec<_> = path.iter().copied().map(Vec3::from_array).collect();
    let mut rings = Vec::new();
    let mut prev_t = (path[1] - path[0]).normalized();
    let ref_axis = if prev_t.z.abs() < 0.9 {
        Vec3::new(0.0, 0.0, 1.0)
    } else {
        Vec3::new(0.0, 1.0, 0.0)
    };
    let mut right = ref_axis.cross(prev_t).normalized();
    for i in 0..path.len() {
        let t = if i == 0 {
            prev_t
        } else if i == path.len() - 1 {
            (path[i] - path[i - 1]).normalized()
        } else {
            ((path[i] - path[i - 1]).normalized() + (path[i + 1] - path[i]).normalized())
                .normalized()
        };
        if t.length() < 0.5 || t.dot(prev_t) < -0.95 {
            return Err("Trajectoire avec demi-tour trop brusque.".into());
        }
        let axis = prev_t.cross(t);
        let sine = axis.length();
        let cosine = prev_t.dot(t);
        if sine > 1e-8 {
            let k = axis / sine;
            right = right * cosine + k.cross(right) * sine + k * (k.dot(right) * (1.0 - cosine));
        }
        right = (right - t * right.dot(t)).normalized();
        let up = t.cross(right).normalized();
        rings.push(
            profile
                .iter()
                .map(|p| path[i] + right * p[0] + up * p[1])
                .collect(),
        );
        prev_t = t;
    }
    skin(&rings, &profile)
}
pub fn rounded_rectangle(w: f64, d: f64, r: f64, kind: FinishKind) -> Vec<[f64; 2]> {
    let mut points = Vec::new();
    let steps = if kind == FinishKind::Chamfer { 1 } else { 10 };
    for (cx, cy, start) in [
        (w / 2.0 - r, d / 2.0 - r, 0.0),
        (-w / 2.0 + r, d / 2.0 - r, 90.0),
        (-w / 2.0 + r, -d / 2.0 + r, 180.0),
        (w / 2.0 - r, -d / 2.0 + r, 270.0),
    ] {
        for j in 0..=steps {
            let a = (start + 90.0 * j as f64 / steps as f64).to_radians();
            points.push([cx + r * a.cos(), cy + r * a.sin()]);
        }
    }
    points
}
