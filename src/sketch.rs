use crate::solver::{self, SolveReport};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Curve {
    Line,
    Arc { bulge: f64 },
    Spline { c1: [f64; 2], c2: [f64; 2] },
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Constraint {
    Fixed { point: usize, at: [f64; 2] },
    Horizontal { edge: usize },
    Vertical { edge: usize },
    Coincident { a: usize, b: usize },
    Distance { a: usize, b: usize, value: f64 },
    Parallel { a: usize, b: usize },
    Perpendicular { a: usize, b: usize },
    Angle { a: usize, b: usize, degrees: f64 },
    Tangent { a: usize, b: usize },
}
impl Constraint {
    pub fn label(&self) -> String {
        match self {
            Self::Fixed { point, .. } => format!("Fixer P{}", point + 1),
            Self::Horizontal { edge } => format!("Horizontal E{}", edge + 1),
            Self::Vertical { edge } => format!("Vertical E{}", edge + 1),
            Self::Coincident { a, b } => format!("Coïncidence P{} / P{}", a + 1, b + 1),
            Self::Distance { a, b, value } => format!("P{} — P{} : {value:.3} mm", a + 1, b + 1),
            Self::Parallel { a, b } => format!("Parallèles E{} / E{}", a + 1, b + 1),
            Self::Perpendicular { a, b } => format!("Perpendiculaires E{} / E{}", a + 1, b + 1),
            Self::Angle { a, b, degrees } => {
                format!("Angle E{} / E{} : {degrees:.1}°", a + 1, b + 1)
            }
            Self::Tangent { a, b } => format!("Tangentes E{} / E{}", a + 1, b + 1),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Sketch {
    pub points: Vec<[f64; 2]>,
    pub curves: Vec<Curve>,
    pub constraints: Vec<Constraint>,
}
impl Default for Sketch {
    fn default() -> Self {
        Self::rectangle(40.0, 25.0)
    }
}
impl Sketch {
    pub fn polygon(points: Vec<[f64; 2]>) -> Self {
        let n = points.len();
        Self {
            points,
            curves: vec![Curve::Line; n],
            constraints: Vec::new(),
        }
    }
    pub fn rectangle(w: f64, h: f64) -> Self {
        Self::polygon(vec![
            [-w / 2.0, -h / 2.0],
            [w / 2.0, -h / 2.0],
            [w / 2.0, h / 2.0],
            [-w / 2.0, h / 2.0],
        ])
    }
    pub fn circle(radius: f64) -> Self {
        Self {
            points: vec![[radius, 0.0], [0.0, radius], [-radius, 0.0], [0.0, -radius]],
            curves: vec![
                Curve::Arc {
                    bulge: (std::f64::consts::PI / 8.0).tan()
                };
                4
            ],
            constraints: vec![],
        }
    }
    pub fn validate(&self) -> Result<(), String> {
        let n = self.points.len();
        if !(3..=48).contains(&n)
            || self.curves.len() != n
            || self.constraints.len() > 96
            || self
                .points
                .iter()
                .flatten()
                .any(|v| !v.is_finite() || v.abs() > 5000.0)
        {
            return Err(
                "Esquisse invalide (3 à 48 sommets, coordonnées limitées à 5000 mm).".into(),
            );
        }
        for c in &self.curves {
            match c {
                Curve::Arc { bulge } => {
                    if !bulge.is_finite() || bulge.abs() > 4.0 {
                        return Err("Courbure d’arc invalide.".into());
                    }
                }
                Curve::Spline { c1, c2 } => {
                    if c1
                        .iter()
                        .chain(c2)
                        .any(|v| !v.is_finite() || v.abs() > 5000.0)
                    {
                        return Err("Points de contrôle invalides.".into());
                    }
                }
                _ => {}
            }
        }
        for c in &self.constraints {
            let valid = match c {
                Constraint::Fixed { point, at } => {
                    *point < n && at.iter().all(|v| v.is_finite() && v.abs() <= 5000.0)
                }
                Constraint::Horizontal { edge } | Constraint::Vertical { edge } => *edge < n,
                Constraint::Distance { a, b, value } => {
                    *a < n
                        && *b < n
                        && a != b
                        && value.is_finite()
                        && *value >= 0.1
                        && *value <= 5000.0
                }
                Constraint::Angle { a, b, degrees } => {
                    *a < n && *b < n && degrees.is_finite() && *degrees >= 0.0 && *degrees <= 180.0
                }
                Constraint::Coincident { a, b }
                | Constraint::Parallel { a, b }
                | Constraint::Perpendicular { a, b }
                | Constraint::Tangent { a, b } => *a < n && *b < n && a != b,
            };
            if !valid {
                return Err("Référence de contrainte ou valeur invalide.".into());
            }
        }
        Ok(())
    }
    pub fn sample(&self) -> Result<Vec<[f64; 2]>, String> {
        self.validate()?;
        let flat: Vec<f64> = self.points.iter().flatten().copied().collect();
        if self.residuals(&flat).iter().any(|v| v.abs() > 1e-4) {
            return Err("L’esquisse n’est pas résolue. Cliquez Résoudre les contraintes.".into());
        }
        let mut out = Vec::new();
        for i in 0..self.points.len() {
            let steps = if matches!(self.curves[i], Curve::Line) {
                1
            } else {
                12
            };
            for j in 0..steps {
                out.push(self.point_on(i, j as f64 / steps as f64, &self.points));
            }
        }
        // Coincident adjacent endpoints are allowed in constraints, but not zero-length contour segments.
        out.dedup_by(|a, b| (a[0] - b[0]).hypot(a[1] - b[1]) < 1e-7);
        if out.len() > 2
            && (out[0][0] - out.last().unwrap()[0]).hypot(out[0][1] - out.last().unwrap()[1]) < 1e-7
        {
            out.pop();
        }
        crate::geometry::triangulate(&out)?;
        Ok(out)
    }
    pub fn preview_points(&self) -> Vec<[f64; 2]> {
        if self.points.len() != self.curves.len() || self.points.len() < 2 {
            return vec![];
        }
        (0..self.points.len())
            .flat_map(|i| (0..12).map(move |j| self.point_on(i, j as f64 / 12.0, &self.points)))
            .collect()
    }
    fn point_on(&self, edge: usize, t: f64, points: &[[f64; 2]]) -> [f64; 2] {
        let a = points[edge];
        let b = points[(edge + 1) % points.len()];
        match self.curves[edge] {
            Curve::Line => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t],
            Curve::Arc { bulge } if bulge.abs() > 1e-7 => {
                let dx = b[0] - a[0];
                let dy = b[1] - a[1];
                let q = (1.0 - bulge * bulge) / (4.0 * bulge);
                let center = [(a[0] + b[0]) / 2.0 - dy * q, (a[1] + b[1]) / 2.0 + dx * q];
                let radius = (a[0] - center[0]).hypot(a[1] - center[1]);
                let angle = (a[1] - center[1]).atan2(a[0] - center[0]) + 4.0 * bulge.atan() * t;
                [
                    center[0] + radius * angle.cos(),
                    center[1] + radius * angle.sin(),
                ]
            }
            Curve::Arc { .. } => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t],
            Curve::Spline { c1, c2 } => {
                let u = 1.0 - t;
                [
                    u * u * u * a[0]
                        + 3.0 * u * u * t * c1[0]
                        + 3.0 * u * t * t * c2[0]
                        + t * t * t * b[0],
                    u * u * u * a[1]
                        + 3.0 * u * u * t * c1[1]
                        + 3.0 * u * t * t * c2[1]
                        + t * t * t * b[1],
                ]
            }
        }
    }
    fn tangent(&self, edge: usize, end: bool, points: &[[f64; 2]]) -> [f64; 2] {
        let a = points[edge];
        let b = points[(edge + 1) % points.len()];
        match self.curves[edge] {
            Curve::Line => [b[0] - a[0], b[1] - a[1]],
            Curve::Arc { bulge } if bulge.abs() > 1e-7 => {
                let dx = b[0] - a[0];
                let dy = b[1] - a[1];
                let q = (1.0 - bulge * bulge) / (4.0 * bulge);
                let center = [(a[0] + b[0]) / 2.0 - dy * q, (a[1] + b[1]) / 2.0 + dx * q];
                let p = if end { b } else { a };
                let sign = bulge.signum();
                [-(p[1] - center[1]) * sign, (p[0] - center[0]) * sign]
            }
            Curve::Arc { .. } => [b[0] - a[0], b[1] - a[1]],
            Curve::Spline { c1, c2 } => {
                if end {
                    [b[0] - c2[0], b[1] - c2[1]]
                } else {
                    [c1[0] - a[0], c1[1] - a[1]]
                }
            }
        }
    }
    fn residuals(&self, x: &[f64]) -> Vec<f64> {
        let points: Vec<[f64; 2]> = x.chunks_exact(2).map(|p| [p[0], p[1]]).collect();
        let n = points.len();
        let vector = |i: usize| {
            let a = points[i];
            let b = points[(i + 1) % n];
            [b[0] - a[0], b[1] - a[1]]
        };
        let norm = |v: [f64; 2]| {
            let d = v[0].hypot(v[1]).max(1e-10);
            [v[0] / d, v[1] / d]
        };
        let mut out = Vec::new();
        for c in &self.constraints {
            match *c {
                Constraint::Fixed { point, at } => {
                    out.push(points[point][0] - at[0]);
                    out.push(points[point][1] - at[1]);
                }
                Constraint::Horizontal { edge } => out.push(vector(edge)[1]),
                Constraint::Vertical { edge } => out.push(vector(edge)[0]),
                Constraint::Coincident { a, b } => {
                    out.push(points[a][0] - points[b][0]);
                    out.push(points[a][1] - points[b][1]);
                }
                Constraint::Distance { a, b, value } => out
                    .push((points[a][0] - points[b][0]).hypot(points[a][1] - points[b][1]) - value),
                Constraint::Parallel { a, b } => {
                    let u = norm(vector(a));
                    let v = norm(vector(b));
                    out.push(10.0 * (u[0] * v[1] - u[1] * v[0]));
                }
                Constraint::Perpendicular { a, b } => {
                    let u = norm(vector(a));
                    let v = norm(vector(b));
                    out.push(10.0 * (u[0] * v[0] + u[1] * v[1]));
                }
                Constraint::Angle { a, b, degrees } => {
                    let u = norm(vector(a));
                    let v = norm(vector(b));
                    let angle = (u[0] * v[1] - u[1] * v[0])
                        .atan2(u[0] * v[0] + u[1] * v[1])
                        .abs();
                    out.push(10.0 * (angle - degrees.to_radians()));
                }
                Constraint::Tangent { a, b } => {
                    let a1 = points[(a + 1) % n];
                    let b0 = points[b];
                    let u = norm(self.tangent(a, true, &points));
                    let v = norm(self.tangent(b, false, &points));
                    out.extend([
                        a1[0] - b0[0],
                        a1[1] - b0[1],
                        10.0 * (u[0] * v[1] - u[1] * v[0]),
                    ]);
                }
            }
        }
        out
    }
    pub fn solve(&mut self) -> Result<SolveReport, String> {
        self.validate()?;
        let mut x: Vec<f64> = self.points.iter().flatten().copied().collect();
        let report = solver::solve(&mut x, |x| self.residuals(x))?;
        let points: Vec<_> = x.chunks_exact(2).map(|p| [p[0], p[1]]).collect();
        if points
            .iter()
            .flatten()
            .any(|v| !v.is_finite() || v.abs() > 5000.0)
        {
            return Err("Solution en dehors des limites.".into());
        }
        self.points = points;
        Ok(report)
    }
}
