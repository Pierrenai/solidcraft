//! Small bounded Levenberg–Marquardt solver shared by sketch and assembly constraints.
#[derive(Clone, Debug)]
pub struct SolveReport {
    pub residual: f64,
    pub degrees_of_freedom: usize,
    pub iterations: usize,
}
fn jacobian(x: &[f64], r: &[f64], f: &impl Fn(&[f64]) -> Vec<f64>) -> Vec<Vec<f64>> {
    let mut j = vec![vec![0.0; x.len()]; r.len()];
    for col in 0..x.len() {
        let mut p = x.to_vec();
        let h = 1e-5 * (1.0 + x[col].abs());
        p[col] += h;
        let rr = f(&p);
        for row in 0..r.len() {
            j[row][col] = (rr[row] - r[row]) / h;
        }
    }
    j
}
fn linear(mut a: Vec<Vec<f64>>, mut b: Vec<f64>) -> Option<Vec<f64>> {
    let n = b.len();
    for i in 0..n {
        let pivot = (i..n).max_by(|&a1, &b1| a[a1][i].abs().total_cmp(&a[b1][i].abs()))?;
        if a[pivot][i].abs() < 1e-15 {
            return None;
        }
        a.swap(i, pivot);
        b.swap(i, pivot);
        let v = a[i][i];
        for entry in a[i].iter_mut().skip(i) {
            *entry /= v;
        }
        b[i] /= v;
        let pivot_row = a[i].clone();
        for row in 0..n {
            if row == i {
                continue;
            }
            let v = a[row][i];
            for (col, entry) in a[row].iter_mut().enumerate().skip(i) {
                *entry -= v * pivot_row[col];
            }
            b[row] -= v * b[i];
        }
    }
    Some(b)
}
fn rank(mut a: Vec<Vec<f64>>, cols: usize) -> usize {
    let mut row = 0;
    for col in 0..cols {
        let Some(pivot) =
            (row..a.len()).max_by(|&i, &j| a[i][col].abs().total_cmp(&a[j][col].abs()))
        else {
            break;
        };
        if a[pivot][col].abs() < 1e-5 {
            continue;
        }
        a.swap(row, pivot);
        let d = a[row][col];
        for entry in a[row].iter_mut().take(cols).skip(col) {
            *entry /= d;
        }
        let pivot_row = a[row].clone();
        for entries in a.iter_mut().skip(row + 1) {
            let v = entries[col];
            for (k, entry) in entries.iter_mut().enumerate().take(cols).skip(col) {
                *entry -= v * pivot_row[k];
            }
        }
        row += 1;
    }
    row
}
pub fn solve(x: &mut [f64], f: impl Fn(&[f64]) -> Vec<f64>) -> Result<SolveReport, String> {
    if x.len() > 180 {
        return Err("Trop de variables pour le solveur interactif (180 maximum).".into());
    }
    let mut trial = x.to_vec();
    let mut lambda = 1e-3;
    let mut iterations = 0;
    for step in 0..160 {
        iterations = step;
        let r = f(&trial);
        if r.iter().any(|v| !v.is_finite()) {
            return Err("Contrainte dégénérée ou non finie.".into());
        }
        let err = r.iter().map(|v| v * v).sum::<f64>();
        if r.iter().fold(0.0_f64, |a, v| a.max(v.abs())) < 1e-6 {
            break;
        }
        let j = jacobian(&trial, &r, &f);
        let n = trial.len();
        let mut a = vec![vec![0.0; n]; n];
        let mut b = vec![0.0; n];
        for i in 0..n {
            for row in 0..r.len() {
                b[i] -= j[row][i] * r[row];
                for k in 0..n {
                    a[i][k] += j[row][i] * j[row][k];
                }
            }
            a[i][i] += lambda;
        }
        let Some(delta) = linear(a, b) else {
            return Err("Contraintes singulières.".into());
        };
        let candidate: Vec<_> = trial
            .iter()
            .zip(delta)
            .map(|(x, d)| x + d.clamp(-100.0, 100.0))
            .collect();
        let next = f(&candidate).iter().map(|v| v * v).sum::<f64>();
        if next < err {
            trial = candidate;
            lambda = (lambda * 0.35).max(1e-10);
        } else {
            lambda = (lambda * 8.0).min(1e12);
        }
    }
    let r = f(&trial);
    let residual = r.iter().fold(0.0_f64, |a, v| a.max(v.abs()));
    if !residual.is_finite() || residual > 1e-4 {
        return Err(format!("Contraintes incompatibles ou non convergentes (écart {residual:.5}). Aucune modification appliquée."));
    }
    let degrees_of_freedom = x
        .len()
        .saturating_sub(rank(jacobian(&trial, &r, &f), x.len()));
    x.copy_from_slice(&trial);
    Ok(SolveReport {
        residual,
        degrees_of_freedom,
        iterations,
    })
}
