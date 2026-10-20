use solidcraft::{
    assembly::{self, Mate, MateKind},
    drawing::{self, Dimension, View},
    features::{Feature, FinishKind, HoleKind, HoleSpec, Section},
    geometry::{self, Vec3},
    model::{Body, Project, Shape},
    sketch::{Constraint, Curve, Sketch},
};
fn near(a: f64, b: f64, t: f64) {
    assert!((a - b).abs() < t, "{a} vs {b}, tolerance {t}");
}
fn block(w: f64, d: f64, h: f64) -> Body {
    Body::new(
        1,
        Shape::Box {
            width: w,
            depth: d,
            height: h,
        },
    )
}
fn part(f: Feature) -> Body {
    Body::new(1, Shape::Feature(Box::new(f)))
}
#[test]
fn constrained_rectangle_reaches_dimension_and_keeps_fixed_origin() {
    let mut s = Sketch::polygon(vec![[0.0, 0.0], [19.0, 1.0], [18.0, 12.0], [1.0, 11.0]]);
    s.constraints = vec![
        Constraint::Fixed {
            point: 0,
            at: [0.0, 0.0],
        },
        Constraint::Horizontal { edge: 0 },
        Constraint::Vertical { edge: 1 },
        Constraint::Horizontal { edge: 2 },
        Constraint::Vertical { edge: 3 },
        Constraint::Distance {
            a: 0,
            b: 1,
            value: 30.0,
        },
        Constraint::Distance {
            a: 1,
            b: 2,
            value: 20.0,
        },
    ];
    let report = s.solve().unwrap();
    assert_eq!(report.degrees_of_freedom, 0);
    near(s.points[1][0], 30.0, 1e-4);
    near(s.points[2][1], 20.0, 1e-4);
    near(
        geometry::extrude(&s.sample().unwrap(), 10.0)
            .unwrap()
            .volume(),
        6000.0,
        0.01,
    );
}
#[test]
fn contradictory_constraints_do_not_change_sketch() {
    let mut s = Sketch::default();
    let original = s.points.clone();
    s.constraints = vec![
        Constraint::Distance {
            a: 0,
            b: 1,
            value: 10.0,
        },
        Constraint::Distance {
            a: 0,
            b: 1,
            value: 20.0,
        },
    ];
    assert!(s.solve().is_err());
    assert_eq!(s.points, original);
}
#[test]
fn circle_arcs_are_tangent_and_splines_are_sampled() {
    let mut s = Sketch::circle(10.0);
    s.constraints = vec![
        Constraint::Tangent { a: 0, b: 1 },
        Constraint::Tangent { a: 1, b: 2 },
    ];
    let original = s.points.clone();
    s.solve().unwrap();
    assert_eq!(original, s.points);
    near(
        geometry::extrude(&s.sample().unwrap(), 10.0)
            .unwrap()
            .volume(),
        std::f64::consts::PI * 1000.0,
        10.0,
    );
    let mut s = Sketch::default();
    s.curves[2] = Curve::Spline {
        c1: [10.0, 25.0],
        c2: [-10.0, 25.0],
    };
    assert!(s.sample().unwrap().len() > 4);
}
#[test]
fn pocket_is_blind_and_preserves_outer_bounds() {
    let f = Feature::Pocket {
        source: Box::new(block(40.0, 30.0, 10.0)),
        sketch: Sketch::rectangle(20.0, 10.0),
        depth: 4.0,
        through: false,
    };
    let m = f.mesh().unwrap();
    near(m.volume(), 12000.0 - 800.0, 1e-5);
    near(m.bounds().unwrap().1.z, 10.0, 1e-9);
}
#[test]
fn revolution_full_and_half_have_expected_volume() {
    let profile = Sketch::polygon(vec![[0.0, 0.0], [10.0, 0.0], [10.0, 20.0], [0.0, 20.0]]);
    for angle in [360.0, 180.0] {
        let m = Feature::Revolve {
            profile: profile.clone(),
            angle,
        }
        .mesh()
        .unwrap();
        near(
            m.volume(),
            std::f64::consts::PI * 100.0 * 20.0 * angle / 360.0,
            5.0,
        );
    }
}
#[test]
fn straight_sweep_and_constant_loft_preserve_volume() {
    let s = Sketch::rectangle(10.0, 8.0);
    let m = Feature::Sweep {
        profile: s.clone(),
        path: vec![[0.0, 0.0, 0.0], [0.0, 0.0, 30.0]],
    }
    .mesh()
    .unwrap();
    near(m.volume(), 2400.0, 1e-5);
    let m = Feature::Loft {
        sections: vec![
            Section {
                profile: s.clone(),
                z: 0.0,
                offset: [0.0; 2],
                twist: 0.0,
            },
            Section {
                profile: s,
                z: 30.0,
                offset: [0.0; 2],
                twist: 0.0,
            },
        ],
    }
    .mesh()
    .unwrap();
    near(m.volume(), 2400.0, 5.0);
}
#[test]
fn sweep_rejects_reversal_and_loft_rejects_unordered_sections() {
    let f = Feature::Sweep {
        profile: Sketch::circle(2.0),
        path: vec![[0.0, 0.0, 0.0], [0.0, 0.0, 10.0], [0.0, 0.0, 0.0]],
    };
    assert!(f.mesh().is_err());
    let s = Section {
        profile: Sketch::default(),
        z: 10.0,
        offset: [0.0; 2],
        twist: 0.0,
    };
    assert!(Feature::Loft {
        sections: vec![s.clone(), s]
    }
    .validate()
    .is_err());
}
#[test]
fn fillet_chamfer_and_shell_remove_expected_volume() {
    let b = block(40.0, 30.0, 10.0);
    let fillet = Feature::Finish {
        source: Box::new(b.clone()),
        kind: FinishKind::Fillet,
        size: 3.0,
    }
    .mesh()
    .unwrap();
    near(
        fillet.volume(),
        12000.0 - (4.0 - std::f64::consts::PI) * 9.0 * 10.0,
        2.0,
    );
    let chamfer = Feature::Finish {
        source: Box::new(b.clone()),
        kind: FinishKind::Chamfer,
        size: 3.0,
    }
    .mesh()
    .unwrap();
    near(chamfer.volume(), 12000.0 - 180.0, 1e-5);
    let shell = Feature::Shell {
        source: Box::new(b),
        thickness: 2.0,
    }
    .mesh()
    .unwrap();
    near(shell.volume(), 12000.0 - 36.0 * 26.0 * 8.0, 1e-5);
}
#[test]
fn hole_wizard_counterbore_and_threaded_pilot() {
    let b = block(40.0, 30.0, 10.0);
    let mut s = HoleSpec {
        kind: HoleKind::Counterbore,
        diameter: 6.0,
        head_diameter: 10.0,
        head_depth: 3.0,
        ..Default::default()
    };
    let m = Feature::Hole {
        source: Box::new(b.clone()),
        spec: s.clone(),
    }
    .mesh()
    .unwrap();
    near(
        m.volume(),
        12000.0 - std::f64::consts::PI * (9.0 * 10.0 + 16.0 * 3.0),
        1.0,
    );
    s.kind = HoleKind::Threaded;
    s.pitch = 1.0;
    let m = Feature::Hole {
        source: Box::new(b),
        spec: s,
    }
    .mesh()
    .unwrap();
    near(
        m.volume(),
        12000.0 - std::f64::consts::PI * 6.25 * 10.0,
        0.5,
    );
}
fn pair() -> Project {
    let a = block(10.0, 10.0, 10.0);
    let mut b = a.clone();
    b.id = 2;
    b.position = [20.0, 8.0, 0.0];
    Project {
        bodies: vec![a, b],
        ..Project::default()
    }
}
fn mate(kind: MateKind) -> Mate {
    Mate {
        a: 1,
        b: 2,
        kind,
        anchor_a: [0.0; 3],
        anchor_b: [0.0; 3],
        axis_a: [0.0, 0.0, 1.0],
        axis_b: [0.0, 0.0, 1.0],
        value: 15.0,
    }
}
#[test]
fn coincident_mate_fixes_origin_without_rotational_overconstraint() {
    let mut p = pair();
    p.assembly.mates.push(mate(MateKind::Coincident));
    let r = assembly::solve_mates(&mut p).unwrap();
    assert_eq!(r.degrees_of_freedom, 3);
    near(Vec3::from_array(p.bodies[1].position).length(), 0.0, 1e-4);
    assert_eq!(p.bodies[0].position, [0.0; 3]);
}
#[test]
fn concentric_mate_retains_slide_and_spin() {
    let mut p = pair();
    p.bodies[1].position[2] = 30.0;
    p.assembly.mates.push(mate(MateKind::Concentric));
    let r = assembly::solve_mates(&mut p).unwrap();
    assert_eq!(r.degrees_of_freedom, 2);
    near(p.bodies[1].position[0], 0.0, 1e-4);
    near(p.bodies[1].position[1], 0.0, 1e-4);
    near(p.bodies[1].position[2], 30.0, 1e-4);
}
#[test]
fn conflicting_mates_leave_the_project_unmodified() {
    let mut p = pair();
    p.assembly.mates = vec![mate(MateKind::Coincident), mate(MateKind::Distance)];
    let before = p.clone();
    assert!(assembly::solve_mates(&mut p).is_err());
    assert_eq!(p, before);
}
#[test]
fn interference_reports_actual_overlap_and_skips_touching() {
    let mut p = pair();
    p.bodies[1].position = [5.0, 0.0, 0.0];
    let hits = assembly::interferences(&p, &p.meshes().unwrap()).unwrap();
    assert_eq!(hits.len(), 1);
    near(hits[0].volume, 500.0, 1e-6);
    p.bodies[1].position = [10.0, 0.0, 0.0];
    assert!(assembly::interferences(&p, &p.meshes().unwrap())
        .unwrap()
        .is_empty());
}
#[test]
fn insert_remaps_mates_and_bom_groups_instances() {
    let mut source = pair();
    source.assembly.mates.push(mate(MateKind::Distance));
    let mut target = Project::demo();
    assembly::insert_project(&mut target, &source).unwrap();
    assert_eq!(target.bodies.len(), 3);
    target.validate().unwrap();
    assert_eq!(target.assembly.mates[0].a, 2);
    assert_eq!(target.assembly.mates[0].b, 3);
    let rows = assembly::bom(&source);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].quantity, 2);
}
#[test]
fn exploded_offsets_do_not_change_assembly_or_export_geometry() {
    let mut p = pair();
    let meshes = p.meshes().unwrap();
    let original = p.bodies.clone();
    p.assembly.explode = assembly::auto_explode(&p, &meshes, 50.0);
    assert_eq!(p.bodies, original);
    assert_eq!(p.assembly.explode.len(), 2);
    near(p.meshes().unwrap()[0].volume(), meshes[0].volume(), 1e-9);
}
#[test]
fn svg_drawing_has_valid_cartouche_dimensions_and_bom() {
    let mut p = Project::demo();
    p.drawing.author = "Pierre & <atelier>".into();
    p.drawing.title = "Platine".into();
    p.drawing.tolerance = 0.1;
    p.drawing.dimensions.push(Dimension {
        view: View::Front,
        a: [-40.0, 0.0],
        b: [40.0, 0.0],
        offset: 12.0,
        minus: 0.2,
        plus: 0.1,
    });
    let sheet = drawing::generate(&p, &p.meshes().unwrap()).unwrap();
    let svg = sheet.svg();
    assert!(svg.contains("width=\"420mm\""));
    assert!(svg.contains("Pierre &amp; &lt;atelier&gt;"));
    assert!(svg.contains("80.00 +0.10/-0.20"));
    assert!(svg.contains("NOMENCLATURE"));
    assert!(svg.contains("P-0001"));
}
#[test]
fn drawing_section_is_real_intersection_and_bad_scale_is_rejected() {
    let mut p = Project::demo();
    p.drawing.section = true;
    p.drawing.section_z = 4.0;
    let meshes = p.meshes().unwrap();
    let triangles: Vec<_> = meshes[0].triangles().collect();
    let segments = drawing::section_segments(&triangles, 4.0);
    assert!(!segments.is_empty());
    for (a, b) in segments {
        near(a.z, 4.0, 1e-6);
        near(b.z, 4.0, 1e-6);
    }
    assert!(drawing::generate(&p, &meshes)
        .unwrap()
        .svg()
        .contains("COUPE A-A"));
    p.drawing.section_z = 100.0;
    assert!(drawing::generate(&p, &meshes).is_err());
    p.drawing.section = false;
    p.drawing.scale = 10.0;
    assert!(drawing::generate(&p, &meshes).is_err());
}
#[test]
fn advanced_features_and_metadata_survive_json_roundtrip() {
    let f = Feature::Bossage {
        sketch: Sketch::circle(8.0),
        height: 10.0,
    };
    let mut p = Project {
        bodies: vec![part(f)],
        ..Project::default()
    };
    p.drawing.author = "Auteur".into();
    p.bodies[0].material = "PLA".into();
    let text = serde_json::to_string(&p).unwrap();
    let q = Project::from_json(&text).unwrap();
    assert_eq!(p, q);
    near(
        p.meshes().unwrap()[0].volume(),
        q.meshes().unwrap()[0].volume(),
        1e-7,
    );
}
#[test]
fn version_one_projects_load_with_new_metadata_defaults() {
    let text = r#"{"format":"solidcraft-rust","version":1,"name":"Ancien","bodies":[{"id":1,"name":"Bloc","shape":{"Box":{"width":10.0,"depth":10.0,"height":10.0}},"position":[0,0,0],"rotation":[0,0,0],"holes":[],"visible":true,"color":[195,199,205]}]}"#;
    let p = Project::from_json(text).unwrap();
    assert!(p.assembly.mates.is_empty());
    assert!(p.bodies[0].reference.is_empty());
    near(p.meshes().unwrap()[0].volume(), 1000.0, 1e-6);
}
