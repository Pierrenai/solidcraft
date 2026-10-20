use solidcraft::{
    assembly::{self, Mate, MateKind},
    drawing,
    features::{Feature, FinishKind, Section},
    model::{Body, Hole, Project, Shape},
    sketch::Sketch,
    stl,
};
use std::fs;
fn save(path: &str, p: &Project) -> Result<(), Box<dyn std::error::Error>> {
    fs::write(
        format!("Exemples/{path}.solidcraft"),
        serde_json::to_string_pretty(p)?,
    )?;
    let meshes = p.meshes()?;
    let refs: Vec<_> = meshes.iter().collect();
    stl::write_binary(fs::File::create(format!("Exemples/{path}.stl"))?, &refs)?;
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    fs::create_dir_all("Exemples")?;
    let source = Body::new(
        1,
        Shape::Box {
            width: 80.0,
            depth: 50.0,
            height: 8.0,
        },
    );
    let mut plate = Body::new(
        1,
        Shape::Feature(Box::new(Feature::Finish {
            source: Box::new(source),
            kind: FinishKind::Fillet,
            size: 5.0,
        })),
    );
    plate.name = "Platine arrondie".into();
    plate.reference = "PLAT-001".into();
    plate.material = "PLA / aluminium".into();
    plate.holes = vec![
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
            x: -30.0,
            y: 15.0,
        },
        Hole {
            diameter: 6.0,
            x: 30.0,
            y: 15.0,
        },
        Hole {
            diameter: 20.0,
            x: 0.0,
            y: 0.0,
        },
    ];
    let mut p = Project {
        name: "Platine à congés et perçages".into(),
        bodies: vec![plate.clone()],
        ..Default::default()
    };
    p.drawing.title = "PLAT-001 — PLATINE DE SUPPORT".into();
    p.drawing.author = "Atelier SolidCraft".into();
    p.drawing.revision = "A".into();
    p.drawing.tolerance = 0.2;
    p.drawing.detail = true;
    p.drawing.detail_center = [0.0, 4.0];
    p.drawing.detail_scale = 2.0;
    save("01-Platine", &p)?;
    fs::write(
        "Exemples/01-Platine-plan.svg",
        drawing::generate(&p, &p.meshes()?)?.svg(),
    )?;
    fs::write("Exemples/01-Platine-nomenclature.csv", drawing::bom_csv(&p))?;
    let mut assembly = Project {
        name: "Palier — assemblage de démonstration".into(),
        bodies: vec![plate],
        ..Default::default()
    };
    let mut ring = Body::new(
        2,
        Shape::Tube {
            radius: 18.0,
            inner_radius: 10.0,
            height: 22.0,
        },
    );
    ring.position = [0.0, 0.0, 8.0];
    ring.name = "Bague de guidage".into();
    ring.reference = "BAG-001".into();
    ring.color = [155, 164, 179];
    assembly.bodies.push(ring);
    let shaft_profile = Sketch::polygon(vec![
        [0.0, 0.0],
        [9.0, 0.0],
        [9.0, 43.0],
        [13.0, 43.0],
        [13.0, 48.0],
        [0.0, 48.0],
    ]);
    let mut shaft = Body::new(
        3,
        Shape::Feature(Box::new(Feature::Revolve {
            profile: shaft_profile,
            angle: 360.0,
        })),
    );
    shaft.name = "Axe épaulé".into();
    shaft.reference = "AXE-001".into();
    shaft.color = [113, 141, 181];
    assembly.bodies.push(shaft);
    for (i, (x, y)) in [(-30.0, -15.0), (30.0, -15.0), (-30.0, 15.0), (30.0, 15.0)]
        .into_iter()
        .enumerate()
    {
        let profile = Sketch::polygon(vec![
            [0.0, -12.0],
            [2.5, -12.0],
            [2.5, 8.0],
            [5.0, 8.0],
            [5.0, 12.0],
            [0.0, 12.0],
        ]);
        let mut bolt = Body::new(
            4 + i as u64,
            Shape::Feature(Box::new(Feature::Revolve {
                profile,
                angle: 360.0,
            })),
        );
        bolt.name = "Fixation (simplifiée)".into();
        bolt.reference = "FIX-001".into();
        bolt.color = [67, 91, 171];
        bolt.position = [x, y, 0.0];
        assembly.bodies.push(bolt);
    }
    assembly.assembly.fixed = vec![1];
    for b in &assembly.bodies[1..] {
        assembly.assembly.mates.push(Mate {
            a: 1,
            b: b.id,
            kind: MateKind::Coincident,
            anchor_a: b.position,
            anchor_b: [0.0; 3],
            axis_a: [0.0, 0.0, 1.0],
            axis_b: [0.0, 0.0, 1.0],
            value: 0.0,
        });
    }
    assembly.assembly.explode = assembly::auto_explode(&assembly, &assembly.meshes()?, 45.0);
    assembly.drawing.scale = 0.65;
    assembly.drawing.author = "Atelier SolidCraft".into();
    save("02-Palier-assemblage", &assembly)?;
    fs::write(
        "Exemples/02-Palier-nomenclature.csv",
        drawing::bom_csv(&assembly),
    )?;
    let features = [
        (
            "03-Coque",
            Feature::Shell {
                source: Box::new(Body::new(
                    1,
                    Shape::Box {
                        width: 60.0,
                        depth: 40.0,
                        height: 25.0,
                    },
                )),
                thickness: 2.5,
            },
        ),
        (
            "04-Balayage",
            Feature::Sweep {
                profile: Sketch::circle(4.0),
                path: vec![
                    [0.0, 0.0, 0.0],
                    [0.0, 0.0, 25.0],
                    [10.0, 0.0, 40.0],
                    [30.0, 0.0, 45.0],
                ],
            },
        ),
        (
            "05-Lissage",
            Feature::Loft {
                sections: vec![
                    Section {
                        profile: Sketch::rectangle(35.0, 28.0),
                        z: 0.0,
                        offset: [0.0; 2],
                        twist: 0.0,
                    },
                    Section {
                        profile: Sketch::circle(12.0),
                        z: 30.0,
                        offset: [0.0; 2],
                        twist: 10.0,
                    },
                    Section {
                        profile: Sketch::circle(8.0),
                        z: 45.0,
                        offset: [0.0; 2],
                        twist: 10.0,
                    },
                ],
            },
        ),
    ];
    for (name, f) in features {
        let b = Body::new(1, Shape::Feature(Box::new(f)));
        save(
            name,
            &Project {
                name: name.into(),
                bodies: vec![b],
                ..Default::default()
            },
        )?;
    }
    println!("5 projets paramétriques, 5 STL, 1 plan SVG et 2 nomenclatures générés.");
    Ok(())
}
