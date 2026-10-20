use crate::{
    geometry::{BooleanOp, Mesh, Vec3},
    model::Project,
    solver::{self, SolveReport},
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum MateKind {
    Coincident,
    Concentric,
    Parallel,
    Perpendicular,
    Distance,
    Angle,
}
impl MateKind {
    pub fn label(self) -> &'static str {
        match self {
            Self::Coincident => "Coïncidente",
            Self::Concentric => "Concentrique",
            Self::Parallel => "Parallèle",
            Self::Perpendicular => "Perpendiculaire",
            Self::Distance => "Distance",
            Self::Angle => "Angle",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mate {
    pub a: u64,
    pub b: u64,
    pub kind: MateKind,
    pub anchor_a: [f64; 3],
    pub anchor_b: [f64; 3],
    pub axis_a: [f64; 3],
    pub axis_b: [f64; 3],
    pub value: f64,
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ExplodeOffset {
    pub body: u64,
    pub delta: [f64; 3],
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct Assembly {
    pub fixed: Vec<u64>,
    pub mates: Vec<Mate>,
    pub explode: Vec<ExplodeOffset>,
}
#[derive(Clone, Debug)]
pub struct Interference {
    pub a: u64,
    pub b: u64,
    pub volume: f64,
}
#[derive(Clone, Debug)]
pub struct BomRow {
    pub number: usize,
    pub reference: String,
    pub name: String,
    pub material: String,
    pub quantity: usize,
    pub ids: Vec<u64>,
}
impl Assembly {
    pub fn validate(&self, project: &Project) -> Result<(), String> {
        if self.mates.len() > 100 || self.explode.len() > 100 {
            return Err("Assemblage trop complexe.".into());
        }
        let has = |id| project.bodies.iter().any(|b| b.id == id);
        for id in &self.fixed {
            if !has(*id) {
                return Err("Composant fixé introuvable.".into());
            }
        }
        for m in &self.mates {
            if m.a == m.b
                || !has(m.a)
                || !has(m.b)
                || !m.value.is_finite()
                || m.value.abs() > 5000.0
                || m.anchor_a
                    .iter()
                    .chain(&m.anchor_b)
                    .chain(&m.axis_a)
                    .chain(&m.axis_b)
                    .any(|v| !v.is_finite() || v.abs() > 5000.0)
                || Vec3::from_array(m.axis_a).length() < 1e-6
                || Vec3::from_array(m.axis_b).length() < 1e-6
            {
                return Err("Contrainte d’assemblage invalide.".into());
            }
            if (m.kind == MateKind::Distance && m.value < 0.0)
                || (m.kind == MateKind::Angle && !(0.0..=180.0).contains(&m.value))
            {
                return Err("Distance ou angle invalide.".into());
            }
        }
        let mut ids = std::collections::HashSet::new();
        for e in &self.explode {
            if !has(e.body)
                || !ids.insert(e.body)
                || e.delta.iter().any(|v| !v.is_finite() || v.abs() > 5000.0)
            {
                return Err("Déplacement éclaté invalide.".into());
            }
        }
        Ok(())
    }
}
fn mate_residual(m: &Mate, a: ([f64; 3], [f64; 3]), b: ([f64; 3], [f64; 3])) -> Vec<f64> {
    let pa = Vec3::from_array(m.anchor_a).rotate(a.1) + Vec3::from_array(a.0);
    let pb = Vec3::from_array(m.anchor_b).rotate(b.1) + Vec3::from_array(b.0);
    let u = Vec3::from_array(m.axis_a).rotate(a.1).normalized();
    let v = Vec3::from_array(m.axis_b).rotate(b.1).normalized();
    let d = pb - pa;
    match m.kind {
        MateKind::Coincident => d.array().to_vec(),
        MateKind::Concentric => u
            .cross(v)
            .array()
            .into_iter()
            .map(|x| x * 10.0)
            .chain((d - u * d.dot(u)).array())
            .collect(),
        MateKind::Parallel => u.cross(v).array().into_iter().map(|v| v * 10.0).collect(),
        MateKind::Perpendicular => vec![u.dot(v) * 10.0],
        MateKind::Distance => vec![d.length() - m.value],
        MateKind::Angle => {
            vec![10.0 * (u.cross(v).length().atan2(u.dot(v)) - m.value.to_radians())]
        }
    }
}
pub fn solve_mates(project: &mut Project) -> Result<SolveReport, String> {
    project.assembly.validate(project)?;
    if project.bodies.is_empty() {
        return Err("Ajoutez des composants.".into());
    }
    let fixed = if project.assembly.fixed.is_empty() {
        vec![project.bodies[0].id]
    } else {
        project.assembly.fixed.clone()
    };
    let movable: Vec<_> = project
        .bodies
        .iter()
        .filter(|b| !fixed.contains(&b.id))
        .map(|b| b.id)
        .collect();
    let mut values: Vec<_> = movable
        .iter()
        .flat_map(|id| {
            let b = project.bodies.iter().find(|b| b.id == *id).unwrap();
            b.position.into_iter().chain(b.rotation)
        })
        .collect();
    let pose = |id: u64, x: &[f64]| {
        if let Some(i) = movable.iter().position(|v| *v == id) {
            (
                [x[i * 6], x[i * 6 + 1], x[i * 6 + 2]],
                [x[i * 6 + 3], x[i * 6 + 4], x[i * 6 + 5]],
            )
        } else {
            let b = project.bodies.iter().find(|b| b.id == id).unwrap();
            (b.position, b.rotation)
        }
    };
    let report = solver::solve(&mut values, |x| {
        project
            .assembly
            .mates
            .iter()
            .flat_map(|m| mate_residual(m, pose(m.a, x), pose(m.b, x)))
            .collect()
    })?;
    if values.iter().any(|v| !v.is_finite() || v.abs() > 5000.0) {
        return Err("La solution dépasse les limites de coordonnées.".into());
    }
    for (id, chunk) in movable.iter().zip(values.chunks_exact(6)) {
        let b = project.bodies.iter_mut().find(|b| b.id == *id).unwrap();
        b.position = [chunk[0], chunk[1], chunk[2]];
        b.rotation = [chunk[3], chunk[4], chunk[5]];
    }
    project.assembly.fixed = fixed;
    Ok(report)
}
pub fn max_residual(project: &Project) -> f64 {
    project
        .assembly
        .mates
        .iter()
        .flat_map(|m| {
            let a = project.bodies.iter().find(|b| b.id == m.a);
            let b = project.bodies.iter().find(|b| b.id == m.b);
            match (a, b) {
                (Some(a), Some(b)) => {
                    mate_residual(m, (a.position, a.rotation), (b.position, b.rotation))
                }
                _ => vec![f64::INFINITY],
            }
        })
        .fold(0.0_f64, |a, v| a.max(v.abs()))
}
pub fn interferences(project: &Project, meshes: &[Mesh]) -> Result<Vec<Interference>, String> {
    let mut found = Vec::new();
    let mut tested = 0;
    for i in 0..meshes.len() {
        for j in i + 1..meshes.len() {
            let Some((a, b)) = meshes[i].bounds() else {
                continue;
            };
            let Some((c, d)) = meshes[j].bounds() else {
                continue;
            };
            if b.x <= c.x + 1e-6
                || d.x <= a.x + 1e-6
                || b.y <= c.y + 1e-6
                || d.y <= a.y + 1e-6
                || b.z <= c.z + 1e-6
                || d.z <= a.z + 1e-6
            {
                continue;
            }
            tested += 1;
            if tested > 100 {
                return Err("Limite de 100 paires en recouvrement. Vérifiez des sous-ensembles plus petits.".into());
            }
            match meshes[i].boolean(&meshes[j], BooleanOp::Intersect) {
                Ok(m) => {
                    if m.volume() > 1e-6 {
                        found.push(Interference {
                            a: project.bodies[i].id,
                            b: project.bodies[j].id,
                            volume: m.volume(),
                        });
                    }
                }
                Err(e) if e.contains("vide") => {}
                Err(e) => return Err(format!("Interférence non calculable : {e}")),
            }
        }
    }
    Ok(found)
}
pub fn auto_explode(project: &Project, meshes: &[Mesh], distance: f64) -> Vec<ExplodeOffset> {
    let center = crate::model::scene_bounds(meshes)
        .map(|(a, b)| (a + b) * 0.5)
        .unwrap_or_default();
    project
        .bodies
        .iter()
        .zip(meshes)
        .enumerate()
        .map(|(i, (b, m))| {
            let c = m.bounds().map(|(a, b)| (a + b) * 0.5).unwrap_or_default();
            let mut dir = c - center;
            if dir.length() < 1e-3 {
                let angle = i as f64 * 2.399;
                dir = Vec3::new(angle.cos(), angle.sin(), 0.7);
            }
            ExplodeOffset {
                body: b.id,
                delta: (dir.normalized() * distance).array(),
            }
        })
        .collect()
}
pub fn bom(project: &Project) -> Vec<BomRow> {
    let mut rows: Vec<BomRow> = Vec::new();
    let mut keys: Vec<String> = Vec::new();
    for b in &project.bodies {
        let key = format!(
            "{}|{}|{}|{}",
            b.reference,
            b.material,
            serde_json::to_string(&b.shape).unwrap_or_default(),
            serde_json::to_string(&b.holes).unwrap_or_default()
        );
        if let Some(i) = keys.iter().position(|k| k == &key) {
            rows[i].quantity += 1;
            rows[i].ids.push(b.id);
        } else {
            keys.push(key);
            rows.push(BomRow {
                number: rows.len() + 1,
                reference: if b.reference.is_empty() {
                    format!("P-{:04}", b.id)
                } else {
                    b.reference.clone()
                },
                name: b.name.clone(),
                material: b.material.clone(),
                quantity: 1,
                ids: vec![b.id],
            });
        }
    }
    rows
}
pub fn insert_project(target: &mut Project, source: &Project) -> Result<usize, String> {
    source.validate()?;
    if target.bodies.len() + source.bodies.len() > 100 {
        return Err("100 composants maximum.".into());
    }
    let mut map = std::collections::HashMap::new();
    let mut id = target.next_id();
    for original in &source.bodies {
        let mut b = original.clone();
        map.insert(b.id, id);
        b.id = id;
        id += 1;
        b.group = if b.group.is_empty() {
            source.name.clone()
        } else {
            format!("{}/{}", source.name, b.group)
        };
        target.bodies.push(b);
    }
    for m in &source.assembly.mates {
        let mut m = m.clone();
        m.a = map[&m.a];
        m.b = map[&m.b];
        target.assembly.mates.push(m);
    }
    // Import fixed bases as well, so a subassembly keeps its solved reference placement.
    target
        .assembly
        .fixed
        .extend(source.assembly.fixed.iter().map(|id| map[id]));
    Ok(source.bodies.len())
}
