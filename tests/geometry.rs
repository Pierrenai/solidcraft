use solidcraft::{
    geometry::{self, BooleanOp, Vec3},
    model::{Body, Hole, Project, Shape},
    stl,
};
fn near(a: f64, b: f64, tol: f64) {
    assert!((a - b).abs() < tol, "{a} != {b} (tol {tol})");
}
#[test]
fn block_dimensions_and_volume() {
    let m = geometry::cuboid(80.0, 50.0, 8.0).unwrap();
    assert_eq!(
        m.bounds(),
        Some((Vec3::new(-40.0, -25.0, 0.0), Vec3::new(40.0, 25.0, 8.0)))
    );
    near(m.volume(), 32000.0, 1e-6);
}
#[test]
fn cylinder_is_z_aligned() {
    let m = geometry::cylinder(15.0, 30.0);
    let (a, b) = m.bounds().unwrap();
    near(a.z, 0.0, 1e-9);
    near(b.z, 30.0, 1e-9);
    near(m.volume(), std::f64::consts::PI * 225.0 * 30.0, 20.0);
}
#[test]
fn four_holes_remove_expected_material() {
    let p = Project::demo();
    let m = p.meshes().unwrap();
    near(
        m[0].volume(),
        32000.0 - 4.0 * std::f64::consts::PI * 9.0 * 8.0,
        1.0,
    );
    assert!(m[0].triangles().count() > 100);
}
#[test]
fn boolean_volumes_and_translations() {
    for (op, expected) in [
        (BooleanOp::Union, 1500.0),
        (BooleanOp::Subtract, 500.0),
        (BooleanOp::Intersect, 500.0),
    ] {
        let a = geometry::cuboid(10.0, 10.0, 10.0).unwrap();
        let b = a.transformed([5.0, 0.0, 0.0], [0.0; 3]);
        let m = a.boolean(&b, op).unwrap();
        near(m.volume(), expected, 1e-5);
    }
}
#[test]
fn empty_intersection_is_an_error() {
    let a = geometry::cuboid(10.0, 10.0, 10.0).unwrap();
    let b = a.transformed([100.0, 0.0, 0.0], [0.0; 3]);
    assert!(a.boolean(&b, BooleanOp::Intersect).is_err());
}
#[test]
fn concave_extrusion_and_reversed_winding() {
    let mut p = vec![
        [-25.0, -15.0],
        [25.0, -15.0],
        [25.0, 5.0],
        [5.0, 5.0],
        [5.0, 25.0],
        [-25.0, 25.0],
    ];
    let a = geometry::extrude(&p, 12.0).unwrap();
    near(a.volume(), 19200.0, 1e-6);
    p.reverse();
    near(geometry::extrude(&p, 12.0).unwrap().volume(), 19200.0, 1e-6);
}
#[test]
fn crossed_and_zero_area_sketches_rejected() {
    assert!(geometry::triangulate(&[[0.0, 0.0], [10.0, 10.0], [0.0, 10.0], [10.0, 0.0]]).is_err());
    assert!(geometry::triangulate(&[[0.0, 0.0], [10.0, 0.0], [20.0, 0.0]]).is_err());
}
#[test]
fn tube_volume_and_rotation() {
    let b = Body::new(
        1,
        Shape::Tube {
            radius: 20.0,
            inner_radius: 15.0,
            height: 30.0,
        },
    );
    let m = b.mesh().unwrap();
    near(m.volume(), std::f64::consts::PI * 175.0 * 30.0, 20.0);
    let m = m.transformed([0.0, 0.0, 0.0], [90.0, 0.0, 0.0]);
    let (a, b) = m.bounds().unwrap();
    near(b.z - a.z, 40.0, 1e-5);
    near(b.y - a.y, 30.0, 1e-5);
}
#[test]
fn binary_stl_layout_normals_and_roundtrip_volume() {
    let m = Project::demo().meshes().unwrap().remove(0);
    let mut bytes = Vec::new();
    stl::write_binary(&mut bytes, &[&m]).unwrap();
    let count = u32::from_le_bytes(bytes[80..84].try_into().unwrap()) as usize;
    assert_eq!(bytes.len(), 84 + 50 * count);
    let mut volume = 0.0;
    for i in 0..count {
        let offset = 84 + i * 50;
        let float =
            |j| f32::from_le_bytes(bytes[offset + j..offset + j + 4].try_into().unwrap()) as f64;
        let v = |j| Vec3::new(float(j), float(j + 4), float(j + 8));
        let normal = v(0);
        let a = v(12);
        let b = v(24);
        let c = v(36);
        assert!(normal.dot((b - a).cross(c - a)) > 0.0);
        volume += a.dot(b.cross(c)) / 6.0;
    }
    near(volume.abs(), m.volume(), 0.03);
}
#[test]
fn project_roundtrip_and_invalid_inputs() {
    let p = Project::demo();
    let text = serde_json::to_string(&p).unwrap();
    assert_eq!(p, Project::from_json(&text).unwrap());
    let mut bad = p.clone();
    bad.bodies[0].holes.push(Hole {
        diameter: -1.0,
        x: 0.0,
        y: 0.0,
    });
    assert!(bad.validate().is_err());
    let mut bad = p.clone();
    bad.bodies[0].position[0] = f64::NAN;
    assert!(bad.validate().is_err());
    let mut bad = p.clone();
    bad.bodies.push(bad.bodies[0].clone());
    assert!(bad.validate().is_err());
    assert!(Project::from_json("{}").is_err());
}
#[test]
fn unchanged_sources_after_boolean() {
    let a = geometry::cuboid(20.0, 20.0, 10.0).unwrap();
    let b = geometry::cylinder(3.0, 20.0);
    let _ = a.boolean(&b, BooleanOp::Subtract).unwrap();
    near(a.volume(), 4000.0, 1e-6);
}
