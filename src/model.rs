use crate::geometry::{self, BooleanOp, Mesh, Vec3};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Hole {
    pub diameter: f64,
    pub x: f64,
    pub y: f64,
}
impl Default for Hole {
    fn default() -> Self {
        Self {
            diameter: 6.0,
            x: 0.0,
            y: 0.0,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum Shape {
    /// Feature-based part (sketches and features rebuilt in order).
    Part(Box<crate::part::Part>),
    Feature(Box<crate::features::Feature>),
    Box {
        width: f64,
        depth: f64,
        height: f64,
    },
    Cylinder {
        radius: f64,
        height: f64,
    },
    Tube {
        radius: f64,
        inner_radius: f64,
        height: f64,
    },
    Extrusion {
        points: Vec<[f64; 2]>,
        height: f64,
    },
    Boolean {
        operation: BooleanOp,
        a: Box<Body>,
        b: Box<Body>,
    },
}
impl Shape {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Part(_) => "Pièce paramétrique",
            Self::Feature(f) => f.label(),
            Self::Box { .. } => "Bloc",
            Self::Cylinder { .. } => "Cylindre",
            Self::Tube { .. } => "Tube",
            Self::Extrusion { .. } => "Extrusion",
            Self::Boolean { operation, .. } => operation.label(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Body {
    pub id: u64,
    pub name: String,
    pub shape: Shape,
    pub position: [f64; 3],
    pub rotation: [f64; 3],
    pub holes: Vec<Hole>,
    pub visible: bool,
    pub color: [u8; 3],
    #[serde(default)]
    pub reference: String,
    #[serde(default)]
    pub material: String,
    #[serde(default)]
    pub group: String,
}
impl Body {
    pub fn new(id: u64, shape: Shape) -> Self {
        Self {
            id,
            name: format!("{} {}", shape.label(), id),
            shape,
            position: [0.0; 3],
            rotation: [0.0; 3],
            holes: Vec::new(),
            visible: true,
            color: [195, 199, 205],
            reference: format!("P-{id:04}"),
            material: String::new(),
            group: String::new(),
        }
    }
    pub fn mesh(&self) -> Result<Mesh, String> {
        let mut mesh = match &self.shape {
            Shape::Part(p) => p.mesh()?,
            Shape::Feature(f) => f.mesh()?,
            Shape::Box {
                width,
                depth,
                height,
            } => geometry::cuboid(*width, *depth, *height)?,
            Shape::Cylinder { radius, height } => geometry::cylinder(*radius, *height),
            Shape::Tube {
                radius,
                inner_radius,
                height,
            } => {
                let outer = geometry::cylinder(*radius, *height);
                let inner = geometry::cylinder(*inner_radius, *height + 2.0)
                    .transformed([0.0, 0.0, -1.0], [0.0; 3]);
                outer.boolean(&inner, BooleanOp::Subtract)?
            }
            Shape::Extrusion { points, height } => geometry::extrude(points, *height)?,
            Shape::Boolean { operation, a, b } => a.mesh()?.boolean(&b.mesh()?, *operation)?,
        };
        if mesh.polygons.is_empty() {
            return Ok(mesh);
        }
        for hole in &self.holes {
            let (min, max) = mesh.bounds().ok_or("Solide vide.")?;
            let cut = geometry::cylinder(hole.diameter / 2.0, max.z - min.z + 2.0)
                .transformed([hole.x, hole.y, min.z - 1.0], [0.0; 3]);
            mesh = mesh.boolean(&cut, BooleanOp::Subtract)?;
        }
        Ok(mesh.transformed(self.position, self.rotation))
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Project {
    pub format: String,
    pub version: u32,
    pub name: String,
    pub bodies: Vec<Body>,
    #[serde(default)]
    pub assembly: crate::assembly::Assembly,
    #[serde(default)]
    pub drawing: crate::drawing::Drawing,
}
impl Default for Project {
    fn default() -> Self {
        Self {
            format: "solidcraft-rust".into(),
            version: 2,
            name: "Nouvelle pièce".into(),
            bodies: Vec::new(),
            assembly: Default::default(),
            drawing: Default::default(),
        }
    }
}
impl Project {
    /// Empty feature-based part, ready for a first sketch.
    pub fn new_part(name: &str) -> Self {
        let mut b = Body::new(1, Shape::Part(Box::default()));
        b.name = name.into();
        Self {
            name: name.into(),
            bodies: vec![b],
            ..Self::default()
        }
    }
    pub fn demo() -> Self {
        let mut b = Body::new(
            1,
            Shape::Box {
                width: 80.0,
                depth: 50.0,
                height: 8.0,
            },
        );
        b.name = "Platine de fixation".into();
        b.holes = vec![
            Hole {
                diameter: 6.0,
                x: -30.0,
                y: -15.0,
            },
            Hole {
                diameter: 6.0,
                x: 30.0,
                y: -15.0,
            },
            Hole {
                diameter: 6.0,
                x: 30.0,
                y: 15.0,
            },
            Hole {
                diameter: 6.0,
                x: -30.0,
                y: 15.0,
            },
        ];
        Self {
            name: "Platine de fixation".into(),
            bodies: vec![b],
            ..Self::default()
        }
    }
    pub fn next_id(&self) -> u64 {
        self.bodies
            .iter()
            .map(|b| b.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1)
    }
    pub fn validate(&self) -> Result<(), String> {
        if self.format != "solidcraft-rust"
            || !(1..=3).contains(&self.version)
            || self.name.len() > 200
            || self.bodies.len() > 100
        {
            return Err("Projet SolidCraft Rust invalide ou version incompatible.".into());
        }
        let mut count = 0;
        let mut ids = std::collections::HashSet::new();
        for b in &self.bodies {
            if !ids.insert(b.id) || b.id == u64::MAX {
                return Err("Identifiants invalides ou dupliqués.".into());
            }
            validate_body(b, 0, &mut count)?;
        }
        self.assembly.validate(self)?;
        self.drawing.validate()?;
        Ok(())
    }
    pub fn meshes(&self) -> Result<Vec<Mesh>, String> {
        self.validate()?;
        self.bodies.iter().map(Body::mesh).collect()
    }
    pub fn from_json(text: &str) -> Result<Self, String> {
        if text.len() > 2_000_000 {
            return Err("Le fichier dépasse 2 Mo.".into());
        }
        let p: Self = serde_json::from_str(text).map_err(|e| format!("Fichier invalide : {e}"))?;
        p.validate()?;
        Ok(p)
    }
}
pub(crate) fn validate_body(b: &Body, depth: usize, count: &mut usize) -> Result<(), String> {
    *count += 1;
    if depth > 12
        || *count > 300
        || b.name.len() > 200
        || b.reference.len() > 120
        || b.material.len() > 120
        || b.group.len() > 400
        || b.holes.len() > 32
    {
        return Err("Projet trop complexe.".into());
    }
    let finite = |v: &f64| v.is_finite() && v.abs() <= 5000.0;
    let positive = |v: &f64| finite(v) && *v >= 0.1;
    if !b.position.iter().all(finite) || !b.rotation.iter().all(finite) {
        return Err("Position ou rotation invalide.".into());
    }
    match &b.shape {
        Shape::Part(p) => p.validate()?,
        Shape::Feature(f) => {
            f.validate()?;
            if let Some(source) = f.source() {
                validate_body(source, depth + 1, count)?;
            }
        }
        Shape::Box {
            width,
            depth,
            height,
        } => {
            if ![width, depth, height].into_iter().all(positive) {
                return Err("Les dimensions doivent être entre 0,1 et 5000 mm.".into());
            }
        }
        Shape::Cylinder { radius, height } => {
            if ![radius, height].into_iter().all(positive) {
                return Err("Rayon ou hauteur invalide.".into());
            }
        }
        Shape::Tube {
            radius,
            inner_radius,
            height,
        } => {
            if ![radius, inner_radius, height].into_iter().all(positive) || inner_radius >= radius {
                return Err("Le rayon intérieur doit être inférieur au rayon extérieur.".into());
            }
        }
        Shape::Extrusion { points, height } => {
            if !positive(height) {
                return Err("Hauteur d’extrusion invalide.".into());
            }
            geometry::triangulate(points)?;
        }
        Shape::Boolean { a, b, .. } => {
            validate_body(a, depth + 1, count)?;
            validate_body(b, depth + 1, count)?;
        }
    }
    if b.holes
        .iter()
        .any(|h| !positive(&h.diameter) || !finite(&h.x) || !finite(&h.y))
    {
        return Err("Perçage invalide.".into());
    }
    Ok(())
}
pub fn scene_bounds(meshes: &[Mesh]) -> Option<(Vec3, Vec3)> {
    let mut points = meshes.iter().filter_map(Mesh::bounds);
    let (mut min, mut max) = points.next()?;
    for (a, b) in points {
        min.x = min.x.min(a.x);
        min.y = min.y.min(a.y);
        min.z = min.z.min(a.z);
        max.x = max.x.max(b.x);
        max.y = max.y.max(b.y);
        max.z = max.z.max(b.z);
    }
    Some((min, max))
}
