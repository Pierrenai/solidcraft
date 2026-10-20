use eframe::egui::{self, Color32, RichText, Vec2};
use solidcraft::{
    assembly::{self, Mate, MateKind},
    drawing::{self, Dimension, Drawing, Element, Sheet, View},
    features::{Feature, FinishKind, HoleKind, HoleSpec, Section},
    geometry::{Mesh, Vec3},
    model::{Body, Project, Shape},
    sketch::{Constraint, Curve, Sketch},
};
use std::fs;
pub enum Panel {
    Part(Box<Body>),
    Assembly(Box<AssemblyEditor>),
    Drawing(Box<DrawingEditor>),
    None,
}
pub struct AssemblyEditor {
    pub draft: Project,
    pub mate: Mate,
    pub editing: Option<usize>,
    pub distance: f64,
}
pub struct DrawingEditor {
    pub settings: Drawing,
    pub sheet: Option<Sheet>,
    pub dimension: Dimension,
}
pub struct Workbench {
    pub panel: Panel,
    pub message: String,
    point: usize,
    rule: usize,
    a: usize,
    b: usize,
    value: f64,
    radius: f64,
    width: f64,
    height: f64,
    section: usize,
}
impl Default for Workbench {
    fn default() -> Self {
        Self {
            panel: Panel::None,
            message: String::new(),
            point: 0,
            rule: 0,
            a: 1,
            b: 2,
            value: 20.0,
            radius: 15.0,
            width: 40.0,
            height: 25.0,
            section: 0,
        }
    }
}
fn source(b: &Body) -> Body {
    let mut b = b.clone();
    b.position = [0.0; 3];
    b.rotation = [0.0; 3];
    b
}
impl Workbench {
    pub fn open_part(&mut self, kind: &str, project: &Project, selected: Option<u64>) {
        self.message.clear();
        self.point = 0;
        self.section = 0;
        let current = project.bodies.iter().find(|b| Some(b.id) == selected);
        let needs_source = matches!(
            kind,
            "pocket" | "fillet" | "chamfer" | "shell" | "hole" | "edit"
        );
        if needs_source && current.is_none() {
            self.message = "Sélectionnez un solide.".into();
            return;
        }
        if kind == "edit" {
            if let Some(b) = current {
                self.panel = Panel::Part(Box::new(b.clone()));
            }
            return;
        }
        let feature = match kind {
            "bossage" => Feature::Bossage {
                sketch: Sketch::default(),
                height: 12.0,
            },
            "pocket" => Feature::Pocket {
                source: Box::new(source(current.unwrap())),
                sketch: Sketch::rectangle(20.0, 15.0),
                depth: 3.0,
                through: false,
            },
            "revolve" => Feature::Revolve {
                profile: Sketch::polygon(vec![[8.0, 0.0], [20.0, 0.0], [20.0, 30.0], [8.0, 30.0]]),
                angle: 360.0,
            },
            "sweep" => Feature::Sweep {
                profile: Sketch::circle(5.0),
                path: vec![
                    [0.0, 0.0, 0.0],
                    [0.0, 0.0, 30.0],
                    [15.0, 0.0, 50.0],
                    [40.0, 0.0, 55.0],
                ],
            },
            "loft" => Feature::Loft {
                sections: vec![
                    Section {
                        profile: Sketch::rectangle(40.0, 30.0),
                        z: 0.0,
                        offset: [0.0, 0.0],
                        twist: 0.0,
                    },
                    Section {
                        profile: Sketch::circle(12.0),
                        z: 35.0,
                        offset: [0.0, 0.0],
                        twist: 0.0,
                    },
                ],
            },
            "fillet" | "chamfer" => Feature::Finish {
                source: Box::new(source(current.unwrap())),
                kind: if kind == "fillet" {
                    FinishKind::Fillet
                } else {
                    FinishKind::Chamfer
                },
                size: 3.0,
            },
            "shell" => Feature::Shell {
                source: Box::new(source(current.unwrap())),
                thickness: 2.0,
            },
            "hole" => Feature::Hole {
                source: Box::new(source(current.unwrap())),
                spec: HoleSpec::default(),
            },
            _ => return,
        };
        let mut draft = if needs_source {
            current.unwrap().clone()
        } else {
            Body::new(project.next_id(), Shape::Feature(Box::new(feature.clone())))
        };
        draft.shape = Shape::Feature(Box::new(feature));
        if needs_source {
            draft.holes.clear();
        }
        self.panel = Panel::Part(Box::new(draft));
    }
    pub fn open_assembly(&mut self, project: &Project) {
        if project.bodies.len() < 2 {
            self.message = "Ajoutez ou insérez au moins deux composants.".into();
            return;
        }
        let a = project.bodies[0].id;
        let b = project.bodies[1].id;
        self.panel = Panel::Assembly(Box::new(AssemblyEditor {
            draft: project.clone(),
            mate: Mate {
                a,
                b,
                kind: MateKind::Coincident,
                anchor_a: [0.0; 3],
                anchor_b: [0.0; 3],
                axis_a: [0.0, 0.0, 1.0],
                axis_b: [0.0, 0.0, 1.0],
                value: 10.0,
            },
            editing: None,
            distance: 50.0,
        }));
        self.message.clear();
    }
    pub fn open_drawing(&mut self, project: &Project) {
        self.panel = Panel::Drawing(Box::new(DrawingEditor {
            settings: project.drawing.clone(),
            sheet: None,
            dimension: Dimension {
                view: View::Front,
                a: [-40.0, 0.0],
                b: [40.0, 0.0],
                offset: 12.0,
                minus: 0.1,
                plus: 0.1,
            },
        }));
        self.message.clear();
    }
    pub fn show(
        &mut self,
        ctx: &egui::Context,
        project: &Project,
        meshes: &[Mesh],
    ) -> Option<(Project, Option<u64>)> {
        let mut panel = std::mem::replace(&mut self.panel, Panel::None);
        let mut output = None;
        let mut open = true;
        let mut finished = false;
        match &mut panel {
            Panel::None => {}
            Panel::Part(draft) => {
                let title = if let Shape::Feature(f) = &draft.shape {
                    f.label()
                } else {
                    "Fonction"
                };
                egui::Window::new(title)
                    .open(&mut open)
                    .fade_in(false)
                    .default_pos(egui::pos2(150.0, 115.0))
                    .default_width(720.0)
                    .default_height(680.0)
                    .resizable(true)
                    .show(ctx, |ui| {
                        egui::ScrollArea::vertical()
                            .max_height(620.0)
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label("Nom");
                                    ui.text_edit_singleline(&mut draft.name);
                                });
                                if let Shape::Feature(feature) = &mut draft.shape {
                                    self.feature_ui(ui, feature);
                                }
                            });
                        ui.separator();
                        ui.label(
                            RichText::new(&self.message).color(Color32::from_rgb(159, 72, 44)),
                        );
                        if ui.button("Construire / appliquer la fonction").clicked() {
                            let mut next = project.clone();
                            if let Some(b) = next.bodies.iter_mut().find(|b| b.id == draft.id) {
                                *b = (**draft).clone();
                            } else {
                                next.bodies.push((**draft).clone());
                            }
                            match next.validate().and_then(|_| draft.mesh()) {
                                Ok(_) => {
                                    output = Some((next, Some(draft.id)));
                                    finished = true;
                                }
                                Err(e) => self.message = e,
                            }
                        }
                    });
            }
            Panel::Assembly(editor) => {
                egui::Window::new("Assemblage : composants et contraintes")
                    .open(&mut open)
                    .fade_in(false)
                    .default_pos(egui::pos2(150.0, 115.0))
                    .default_width(860.0)
                    .default_height(700.0)
                    .resizable(true)
                    .show(ctx, |ui| {
                        egui::ScrollArea::vertical()
                            .max_height(610.0)
                            .show(ui, |ui| self.assembly_ui(ui, editor));
                        ui.separator();
                        ui.label(
                            RichText::new(&self.message).color(Color32::from_rgb(129, 76, 44)),
                        );
                        if ui.button("Appliquer l’assemblage au projet").clicked() {
                            if assembly::max_residual(&editor.draft) > 1e-4 {
                                self.message = "Résolvez les contraintes avant d’appliquer.".into();
                            } else {
                                output = Some((editor.draft.clone(), None));
                                finished = true;
                            }
                        }
                    });
            }
            Panel::Drawing(editor) => {
                egui::Window::new("Mise en plan — feuille A3")
                    .open(&mut open)
                    .fade_in(false)
                    .default_pos(egui::pos2(150.0, 115.0))
                    .default_width(1180.0)
                    .default_height(720.0)
                    .resizable(true)
                    .show(ctx, |ui| {
                        ui.columns(2, |cols| {
                            egui::ScrollArea::vertical()
                                .max_height(570.0)
                                .show(&mut cols[0], |ui| {
                                    self.drawing_ui(ui, editor, project, meshes)
                                });
                            egui::ScrollArea::both()
                                .max_height(570.0)
                                .show(&mut cols[1], |ui| {
                                    if let Some(sheet) = &editor.sheet {
                                        preview(ui, sheet, 1.3);
                                        for note in &sheet.notes {
                                            ui.small(note);
                                        }
                                    } else {
                                        ui.label(
                                            "Cliquez Générer l’aperçu pour vérifier les vues.",
                                        );
                                    }
                                });
                        });
                        ui.separator();
                        ui.label(
                            RichText::new(&self.message).color(Color32::from_rgb(129, 76, 44)),
                        );
                        if ui
                            .button("Enregistrer les paramètres du plan dans le projet")
                            .clicked()
                        {
                            if let Err(e) = editor.settings.validate() {
                                self.message = e;
                            } else {
                                let mut next = project.clone();
                                next.drawing = editor.settings.clone();
                                output = Some((next, None));
                                finished = true;
                            }
                        }
                    });
            }
        }
        if open && !finished {
            self.panel = panel;
        }
        output
    }
    fn feature_ui(&mut self, ui: &mut egui::Ui, f: &mut Feature) {
        match f {
            Feature::FaceExtrusion {
                sketch, depth, cut, ..
            } => {
                ui.label("Esquisse attachée à une face. Le plan local est conservé.");
                ui.checkbox(cut, "Enlèvement de matière");
                scalar(ui, "Profondeur", depth, " mm");
                self.sketch_ui(ui, sketch);
            }
            Feature::Bossage { sketch, height } => {
                scalar(ui, "Hauteur d’extrusion", height, " mm");
                self.sketch_ui(ui, sketch);
            }
            Feature::Pocket {
                sketch,
                depth,
                through,
                ..
            } => {
                ui.label("Coupe depuis le dessus du solide, suivant son axe Z local.");
                ui.checkbox(through, "Traversant");
                if !*through {
                    scalar(ui, "Profondeur", depth, " mm");
                }
                self.sketch_ui(ui, sketch);
            }
            Feature::Revolve { profile, angle } => {
                ui.label("Profil dans le plan R/Z : X = rayon positif, Y = hauteur. Axe de révolution : Z.");
                scalar(ui, "Angle", angle, " °");
                self.sketch_ui(ui, profile);
            }
            Feature::Sweep { profile, path } => {
                ui.label("Le profil est transporté perpendiculairement à la trajectoire. Évitez les demi-tours et les auto-intersections.");
                let mut remove = None;
                for (i, p) in path.iter_mut().enumerate() {
                    ui.push_id(i, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(format!("Trajectoire P{}", i + 1));
                            vector(ui, p);
                            if ui.small_button("×").clicked() {
                                remove = Some(i);
                            }
                        });
                    });
                }
                if let Some(i) = remove {
                    path.remove(i);
                }
                if path.len() < 32 && ui.button("+ Point de trajectoire").clicked() {
                    let mut p = path.last().copied().unwrap_or_default();
                    p[2] += 20.0;
                    path.push(p);
                }
                ui.separator();
                self.sketch_ui(ui, profile);
            }
            Feature::Loft { sections } => {
                ui.label("Sections sur des plans XY successifs. Z doit croître ; les profils sont raccordés par leur périmètre.");
                ui.horizontal(|ui| {
                    for (i, _) in sections.iter().enumerate() {
                        ui.selectable_value(&mut self.section, i, format!("Section {}", i + 1));
                    }
                    if sections.len() < 8 && ui.button("+").clicked() {
                        let mut s = sections.last().cloned().unwrap_or(Section {
                            profile: Sketch::default(),
                            z: 0.0,
                            offset: [0.0; 2],
                            twist: 0.0,
                        });
                        s.z += 25.0;
                        sections.push(s);
                        self.section = sections.len() - 1;
                    }
                });
                self.section = self.section.min(sections.len().saturating_sub(1));
                if sections.len() > 2 && ui.button("Supprimer cette section").clicked() {
                    sections.remove(self.section);
                    self.section = 0;
                }
                if let Some(s) = sections.get_mut(self.section) {
                    scalar(ui, "Position Z", &mut s.z, " mm");
                    ui.horizontal(|ui| {
                        scalar(ui, "Décalage X", &mut s.offset[0], "");
                        scalar(ui, "Y", &mut s.offset[1], "");
                    });
                    scalar(ui, "Rotation du profil", &mut s.twist, " °");
                    self.sketch_ui(ui, &mut s.profile);
                }
            }
            Feature::Finish { kind, size, .. } => {
                ui.label("Habillage des quatre arêtes verticales d’un bloc primitif. Les faces supérieure et inférieure restent planes.");
                ui.radio_value(kind, FinishKind::Fillet, "Congé (arc circulaire)");
                ui.radio_value(kind, FinishKind::Chamfer, "Chanfrein à 45°");
                scalar(ui, "Rayon / distance", size, " mm");
            }
            Feature::Shell { thickness, .. } => {
                ui.label("Coque d’un bloc ou cylindre primitif, ouverte sur la face supérieure (+Z local).");
                scalar(ui, "Épaisseur de paroi et de fond", thickness, " mm");
            }
            Feature::Hole { spec, .. } => {
                ui.label("Perçage suivant Z local, à partir de la face supérieure.");
                egui::ComboBox::from_id_salt("hole-kind")
                    .selected_text(hole_label(spec.kind))
                    .show_ui(ui, |ui| {
                        for k in [
                            HoleKind::Simple,
                            HoleKind::Counterbore,
                            HoleKind::Countersink,
                            HoleKind::Threaded,
                        ] {
                            ui.selectable_value(&mut spec.kind, k, hole_label(k));
                        }
                    });
                scalar(ui, "Diamètre", &mut spec.diameter, " mm");
                scalar(ui, "Centre X", &mut spec.x, " mm");
                scalar(ui, "Centre Y", &mut spec.y, " mm");
                ui.checkbox(&mut spec.through, "Traversant");
                if !spec.through {
                    scalar(ui, "Profondeur", &mut spec.depth, " mm");
                }
                if matches!(spec.kind, HoleKind::Counterbore | HoleKind::Countersink) {
                    scalar(ui, "Diamètre de tête", &mut spec.head_diameter, " mm");
                    scalar(ui, "Profondeur de tête", &mut spec.head_depth, " mm");
                }
                if spec.kind == HoleKind::Threaded {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Préréglages métriques :");
                        for (d, p) in [
                            (3.0, 0.5),
                            (4.0, 0.7),
                            (5.0, 0.8),
                            (6.0, 1.0),
                            (8.0, 1.25),
                            (10.0, 1.5),
                        ] {
                            if ui.button(format!("M{d} × {p}")).clicked() {
                                spec.diameter = d;
                                spec.pitch = p;
                            }
                        }
                    });
                    scalar(ui, "Pas", &mut spec.pitch, " mm");
                    ui.colored_label(Color32::from_rgb(161,88,34),"Taraudage conventionnel : avant-trou D − pas et désignation enregistrée. Le filet hélicoïdal n’est pas exporté en STL.");
                }
                ui.small(
                    "Dimensions libres ; aucune conformité à une norme de perçage n’est déclarée.",
                );
            }
        }
    }
    fn sketch_ui(&mut self, ui: &mut egui::Ui, sketch: &mut Sketch) {
        ui.separator();
        ui.strong("Esquisse paramétrique");
        ui.horizontal_wrapped(|ui| {
            scalar(ui, "L", &mut self.width, "");
            scalar(ui, "H", &mut self.height, "");
            if ui.button("Rectangle").clicked() {
                *sketch = Sketch::rectangle(self.width, self.height);
                self.point = 0;
            }
            scalar(ui, "R", &mut self.radius, "");
            if ui.button("Cercle").clicked() {
                *sketch = Sketch::circle(self.radius);
                self.point = 0;
            }
            if ui.button("Vider").clicked() {
                *sketch = Sketch::polygon(vec![]);
                self.point = 0;
            }
        });
        ui.small("Cliquez dans le dessin pour ajouter un sommet. Les coordonnées et les cotes sont modifiables ci-dessous.");
        let width = ui.available_width().clamp(300.0, 700.0);
        let (rect, response) =
            ui.allocate_exact_size(Vec2::new(width, 245.0), egui::Sense::click());
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 2.0, Color32::from_rgb(248, 250, 253));
        let origin = rect.center();
        let scale = 3.0;
        for i in -20..=20 {
            let x = origin.x + i as f32 * 15.0;
            p.line_segment(
                [egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())],
                egui::Stroke::new(0.6, Color32::from_rgb(218, 225, 234)),
            );
        }
        for i in -10..=10 {
            let y = origin.y + i as f32 * 15.0;
            p.line_segment(
                [egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)],
                egui::Stroke::new(0.6, Color32::from_rgb(218, 225, 234)),
            );
        }
        p.line_segment(
            [
                egui::pos2(origin.x, rect.top()),
                egui::pos2(origin.x, rect.bottom()),
            ],
            egui::Stroke::new(1.0, Color32::from_gray(150)),
        );
        p.line_segment(
            [
                egui::pos2(rect.left(), origin.y),
                egui::pos2(rect.right(), origin.y),
            ],
            egui::Stroke::new(1.0, Color32::from_gray(150)),
        );
        let screen = |a: [f64; 2]| origin + Vec2::new(a[0] as f32 * scale, -a[1] as f32 * scale);
        let sampled = sketch.preview_points();
        if sampled.len() > 1 {
            for i in 0..sampled.len() {
                p.line_segment(
                    [screen(sampled[i]), screen(sampled[(i + 1) % sampled.len()])],
                    egui::Stroke::new(1.5, Color32::from_rgb(40, 99, 169)),
                );
            }
        }
        for (i, pt) in sketch.points.iter().enumerate() {
            let at = screen(*pt);
            p.circle_filled(
                at,
                3.5,
                if i == self.point {
                    Color32::from_rgb(221, 123, 38)
                } else {
                    Color32::from_rgb(45, 103, 171)
                },
            );
            p.text(
                at + Vec2::new(5.0, -6.0),
                egui::Align2::LEFT_BOTTOM,
                format!("P{}", i + 1),
                egui::FontId::proportional(10.0),
                Color32::from_rgb(52, 75, 107),
            );
        }
        if response.clicked() {
            if let Some(at) = response.interact_pointer_pos() {
                if let Some((i, _)) = sketch
                    .points
                    .iter()
                    .enumerate()
                    .find(|(_, p)| screen(**p).distance(at) < 9.0)
                {
                    self.point = i;
                } else if sketch.points.len() < 48 {
                    sketch.points.push([
                        ((at.x - origin.x) / scale).round() as f64,
                        ((origin.y - at.y) / scale).round() as f64,
                    ]);
                    sketch.curves.push(Curve::Line);
                    self.point = sketch.points.len() - 1;
                }
            }
        }
        if sketch.points.is_empty() {
            return;
        }
        self.point = self.point.min(sketch.points.len() - 1);
        ui.horizontal(|ui| {
            egui::ComboBox::from_id_salt("sketch-point")
                .selected_text(format!("Sommet P{}", self.point + 1))
                .show_ui(ui, |ui| {
                    for i in 0..sketch.points.len() {
                        ui.selectable_value(
                            &mut self.point,
                            i,
                            format!("P{} / arête E{}", i + 1, i + 1),
                        );
                    }
                });
            scalar(ui, "X", &mut sketch.points[self.point][0], "");
            scalar(ui, "Y", &mut sketch.points[self.point][1], "");
            if ui.button("Retirer").clicked() {
                sketch.points.remove(self.point);
                sketch.curves.remove(self.point);
                sketch.constraints.clear();
                self.point = 0;
            }
        });
        if sketch.points.is_empty() {
            return;
        }
        self.point = self.point.min(sketch.points.len() - 1);
        let label = match sketch.curves[self.point] {
            Curve::Line => "Ligne",
            Curve::Arc { .. } => "Arc",
            Curve::Spline { .. } => "Spline Bézier",
        };
        ui.horizontal(|ui| {
            ui.label("Arête sortante");
            egui::ComboBox::from_id_salt("curve-kind")
                .selected_text(label)
                .show_ui(ui, |ui| {
                    if ui.selectable_label(label == "Ligne", "Ligne").clicked() {
                        sketch.curves[self.point] = Curve::Line;
                    }
                    if ui.selectable_label(label == "Arc", "Arc").clicked() {
                        sketch.curves[self.point] = Curve::Arc {
                            bulge: 0.41421356237,
                        };
                    }
                    if ui
                        .selectable_label(label == "Spline Bézier", "Spline Bézier")
                        .clicked()
                    {
                        let a = sketch.points[self.point];
                        let b = sketch.points[(self.point + 1) % sketch.points.len()];
                        sketch.curves[self.point] = Curve::Spline {
                            c1: [a[0] + (b[0] - a[0]) / 3.0, a[1] + 10.0],
                            c2: [a[0] + 2.0 * (b[0] - a[0]) / 3.0, b[1] + 10.0],
                        };
                    }
                });
        });
        match &mut sketch.curves[self.point] {
            Curve::Arc { bulge } => {
                let mut degrees = 4.0 * bulge.atan().to_degrees();
                if scalar(ui, "Angle de l’arc", &mut degrees, " °") {
                    *bulge = (degrees.to_radians() / 4.0).tan();
                }
            }
            Curve::Spline { c1, c2 } => {
                ui.horizontal(|ui| {
                    scalar(ui, "Contrôle 1 X", &mut c1[0], "");
                    scalar(ui, "Y", &mut c1[1], "");
                });
                ui.horizontal(|ui| {
                    scalar(ui, "Contrôle 2 X", &mut c2[0], "");
                    scalar(ui, "Y", &mut c2[1], "");
                });
            }
            _ => {}
        }
        ui.separator();
        ui.strong("Cotes et relations géométriques");
        let labels = [
            "Distance (points)",
            "Horizontal (arête A)",
            "Vertical (arête A)",
            "Parallèle (arêtes)",
            "Perpendiculaire (arêtes)",
            "Angle (arêtes)",
            "Coïncident (points)",
            "Fixer le point A",
            "Tangence (fin A / début B)",
        ];
        ui.horizontal_wrapped(|ui| {
            egui::ComboBox::from_id_salt("constraint-type")
                .selected_text(labels[self.rule])
                .show_ui(ui, |ui| {
                    for (i, label) in labels.iter().enumerate() {
                        ui.selectable_value(&mut self.rule, i, *label);
                    }
                });
            ui.label("A");
            ui.add(egui::DragValue::new(&mut self.a).range(1..=sketch.points.len()));
            ui.label("B");
            ui.add(egui::DragValue::new(&mut self.b).range(1..=sketch.points.len()));
            if self.rule == 0 || self.rule == 5 {
                ui.add(egui::DragValue::new(&mut self.value).speed(0.1));
            }
            if ui.button("Ajouter").clicked() {
                let a = self.a.saturating_sub(1).min(sketch.points.len() - 1);
                let b = self.b.saturating_sub(1).min(sketch.points.len() - 1);
                let constraint = match self.rule {
                    0 => Constraint::Distance {
                        a,
                        b,
                        value: self.value,
                    },
                    1 => Constraint::Horizontal { edge: a },
                    2 => Constraint::Vertical { edge: a },
                    3 => Constraint::Parallel { a, b },
                    4 => Constraint::Perpendicular { a, b },
                    5 => Constraint::Angle {
                        a,
                        b,
                        degrees: self.value,
                    },
                    6 => Constraint::Coincident { a, b },
                    7 => Constraint::Fixed {
                        point: a,
                        at: sketch.points[a],
                    },
                    _ => Constraint::Tangent { a, b },
                };
                sketch.constraints.push(constraint);
            }
        });
        let mut remove = None;
        for (i, c) in sketch.constraints.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(c.label());
                if ui.small_button("Retirer").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            sketch.constraints.remove(i);
        }
        if ui.button("Résoudre les contraintes").clicked() {
            self.message = match sketch.solve() {
                Ok(r) => format!(
                    "Résolu · {} degrés de liberté · écart {:.6} · {} itérations",
                    r.degrees_of_freedom, r.residual, r.iterations
                ),
                Err(e) => e,
            };
        }
    }

    fn assembly_ui(&mut self, ui: &mut egui::Ui, editor: &mut AssemblyEditor) {
        ui.strong("Composants et référence fixe");
        ui.small("Fixez une pièce de référence. Si aucune n’est fixée, le solveur fixe la première. Les contraintes portent sur des points et axes locaux explicites.");
        for b in &mut editor.draft.bodies {
            ui.push_id(b.id, |ui| {
                ui.horizontal(|ui| {
                    let mut fixed = editor.draft.assembly.fixed.contains(&b.id);
                    if ui.checkbox(&mut fixed, "Fixe").changed() {
                        if fixed {
                            editor.draft.assembly.fixed.push(b.id);
                        } else {
                            editor.draft.assembly.fixed.retain(|id| *id != b.id);
                        }
                    }
                    ui.label(&b.name);
                });
                ui.horizontal(|ui| {
                    ui.label("Référence");
                    ui.add(
                        egui::TextEdit::singleline(&mut b.reference)
                            .desired_width(100.0)
                            .char_limit(100),
                    );
                    ui.label("Matière");
                    ui.add(
                        egui::TextEdit::singleline(&mut b.material)
                            .desired_width(100.0)
                            .char_limit(100),
                    );
                });
                if !b.group.is_empty() {
                    ui.small(format!("Sous-ensemble : {}", b.group));
                }
            });
        }
        ui.separator();
        ui.strong("Ajouter ou modifier une contrainte");
        ui.horizontal(|ui| {
            for (salt, id, label) in [
                ("mate-a", &mut editor.mate.a, "Composant A"),
                ("mate-b", &mut editor.mate.b, "Composant B"),
            ] {
                egui::ComboBox::from_id_salt(salt)
                    .selected_text(
                        editor
                            .draft
                            .bodies
                            .iter()
                            .find(|b| b.id == *id)
                            .map(|b| b.name.as_str())
                            .unwrap_or(label),
                    )
                    .show_ui(ui, |ui| {
                        for b in &editor.draft.bodies {
                            ui.selectable_value(id, b.id, &b.name);
                        }
                    });
            }
            egui::ComboBox::from_id_salt("mate-kind")
                .selected_text(editor.mate.kind.label())
                .show_ui(ui, |ui| {
                    for kind in [
                        MateKind::Coincident,
                        MateKind::Concentric,
                        MateKind::Parallel,
                        MateKind::Perpendicular,
                        MateKind::Distance,
                        MateKind::Angle,
                    ] {
                        ui.selectable_value(&mut editor.mate.kind, kind, kind.label());
                    }
                });
        });
        ui.horizontal(|ui| {
            ui.label("Point A local");
            vector(ui, &mut editor.mate.anchor_a);
        });
        ui.horizontal(|ui| {
            ui.label("Point B local");
            vector(ui, &mut editor.mate.anchor_b);
        });
        ui.horizontal(|ui| {
            ui.label("Axe A local");
            axis(ui, "axis-a", &mut editor.mate.axis_a);
            ui.label("Axe B local");
            axis(ui, "axis-b", &mut editor.mate.axis_b);
        });
        if matches!(editor.mate.kind, MateKind::Distance | MateKind::Angle) {
            scalar(
                ui,
                "Valeur",
                &mut editor.mate.value,
                if editor.mate.kind == MateKind::Angle {
                    " °"
                } else {
                    " mm"
                },
            );
        }
        ui.horizontal(|ui| {
            if ui
                .button(if editor.editing.is_some() {
                    "Remplacer la contrainte"
                } else {
                    "Ajouter la contrainte"
                })
                .clicked()
            {
                let before = editor.draft.assembly.mates.clone();
                if let Some(i) = editor.editing {
                    if let Some(m) = editor.draft.assembly.mates.get_mut(i) {
                        *m = editor.mate.clone();
                    }
                } else {
                    editor.draft.assembly.mates.push(editor.mate.clone());
                }
                if let Err(e) = editor.draft.assembly.validate(&editor.draft) {
                    editor.draft.assembly.mates = before;
                    self.message = e;
                } else {
                    editor.editing = None;
                    self.message = "Contrainte ajoutée. Cliquez Résoudre.".into();
                }
            }
            if ui.button("Résoudre les contraintes").clicked() {
                self.message = match assembly::solve_mates(&mut editor.draft) {
                    Ok(r) => format!(
                        "Résolu · {} degrés de liberté restants · écart {:.6}",
                        r.degrees_of_freedom, r.residual
                    ),
                    Err(e) => e,
                };
            }
        });
        let mut remove = None;
        for (i, m) in editor.draft.assembly.mates.iter().enumerate() {
            ui.horizontal(|ui| {
                ui.label(format!("{} : #{} / #{}", m.kind.label(), m.a, m.b));
                if ui.small_button("Modifier").clicked() {
                    editor.editing = Some(i);
                    editor.mate = m.clone();
                }
                if ui.small_button("Retirer").clicked() {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            editor.draft.assembly.mates.remove(i);
            editor.editing = None;
        }
        ui.separator();
        ui.strong("Détection d’interférences");
        if ui.button("Contrôler tous les composants").clicked() {
            self.message = match editor
                .draft
                .meshes()
                .and_then(|m| assembly::interferences(&editor.draft, &m))
            {
                Ok(found) => {
                    if found.is_empty() {
                        "Aucune interférence volumique détectée.".into()
                    } else {
                        found
                            .iter()
                            .map(|i| format!("#{} / #{} : {:.3} mm³", i.a, i.b, i.volume))
                            .collect::<Vec<_>>()
                            .join("\n")
                    }
                }
                Err(e) => e,
            };
        }
        ui.separator();
        ui.strong("Vue éclatée");
        scalar(ui, "Distance automatique", &mut editor.distance, " mm");
        if ui.button("Créer les déplacements d’éclaté").clicked() {
            match editor.draft.meshes() {
                Ok(m) => {
                    editor.draft.assembly.explode =
                        assembly::auto_explode(&editor.draft, &m, editor.distance)
                }
                Err(e) => self.message = e,
            }
        }
        for offset in &mut editor.draft.assembly.explode {
            ui.horizontal(|ui| {
                ui.label(format!("#{}", offset.body));
                vector(ui, &mut offset.delta);
            });
        }
        ui.small("Appliquez l’assemblage, puis utilisez le curseur Éclaté et Lecture dans le ruban. La géométrie assemblée et l’export STL restent inchangés.");
    }
    fn drawing_ui(
        &mut self,
        ui: &mut egui::Ui,
        editor: &mut DrawingEditor,
        project: &Project,
        meshes: &[Mesh],
    ) {
        ui.strong("Cartouche");
        for (label, text) in [
            ("Titre", &mut editor.settings.title),
            ("Auteur", &mut editor.settings.author),
            ("Indice", &mut editor.settings.revision),
        ] {
            ui.horizontal(|ui| {
                ui.label(label);
                ui.add(egui::TextEdit::singleline(text).char_limit(100));
            });
        }
        scalar(
            ui,
            "Échelle papier / modèle",
            &mut editor.settings.scale,
            ":1",
        );
        ui.checkbox(
            &mut editor.settings.show_hidden,
            "Afficher les traits cachés",
        );
        ui.checkbox(
            &mut editor.settings.auto_dimensions,
            "Coter automatiquement les encombrements",
        );
        scalar(
            ui,
            "Tolérance des cotes automatiques",
            &mut editor.settings.tolerance,
            " mm",
        );
        ui.separator();
        ui.checkbox(
            &mut editor.settings.section,
            "Remplacer l’isométrie par une coupe horizontale A-A",
        );
        if editor.settings.section {
            scalar(ui, "Plan de coupe Z", &mut editor.settings.section_z, " mm");
        }
        ui.checkbox(
            &mut editor.settings.detail,
            "Ajouter un détail B de la vue de face",
        );
        if editor.settings.detail {
            scalar(
                ui,
                "Centre détail X",
                &mut editor.settings.detail_center[0],
                " mm",
            );
            scalar(
                ui,
                "Centre détail Z",
                &mut editor.settings.detail_center[1],
                " mm",
            );
            scalar(
                ui,
                "Échelle du détail",
                &mut editor.settings.detail_scale,
                ":1",
            );
        }
        ui.collapsing("Cotes de fabrication supplémentaires", |ui| {
            egui::ComboBox::from_id_salt("dimension-view")
                .selected_text(editor.dimension.view.label())
                .show_ui(ui, |ui| {
                    for v in [View::Front, View::Top, View::Right] {
                        ui.selectable_value(&mut editor.dimension.view, v, v.label());
                    }
                });
            ui.small("Coordonnées projetées : face X/Z, dessus X/Y, droite Y/Z.");
            ui.horizontal(|ui| {
                scalar(ui, "A x", &mut editor.dimension.a[0], "");
                scalar(ui, "A y", &mut editor.dimension.a[1], "");
            });
            ui.horizontal(|ui| {
                scalar(ui, "B x", &mut editor.dimension.b[0], "");
                scalar(ui, "B y", &mut editor.dimension.b[1], "");
            });
            scalar(
                ui,
                "Décalage sur le papier",
                &mut editor.dimension.offset,
                " mm",
            );
            ui.horizontal(|ui| {
                scalar(ui, "Tolérance -", &mut editor.dimension.minus, "");
                scalar(ui, "+", &mut editor.dimension.plus, "");
            });
            if ui.button("Ajouter la cote").clicked() {
                editor.settings.dimensions.push(editor.dimension.clone());
            }
            let mut remove = None;
            for (i, d) in editor.settings.dimensions.iter().enumerate() {
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "{} : {:.2} mm",
                        d.view.label(),
                        (d.a[0] - d.b[0]).hypot(d.a[1] - d.b[1])
                    ));
                    if ui.small_button("Retirer").clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                editor.settings.dimensions.remove(i);
            }
        });
        ui.separator();
        if ui.button("Générer / actualiser l’aperçu").clicked() {
            let mut next = project.clone();
            next.drawing = editor.settings.clone();
            match drawing::generate(&next, meshes) {
                Ok(s) => {
                    editor.sheet = Some(s);
                    self.message =
                        "Aperçu régénéré. Le SVG imprimé à 100 % conserve l’échelle.".into();
                }
                Err(e) => self.message = e,
            }
        }
        if ui.button("Exporter le plan SVG (vectoriel A3)").clicked() {
            let mut next = project.clone();
            next.drawing = editor.settings.clone();
            match drawing::generate(&next, meshes) {
                Ok(sheet) => {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Plan SVG", &["svg"])
                        .set_file_name("plan-solidcraft.svg")
                        .save_file()
                    {
                        self.message = match fs::write(&path, sheet.svg()) {
                            Ok(()) => format!("Plan exporté : {}", path.display()),
                            Err(e) => e.to_string(),
                        };
                    }
                    editor.sheet = Some(sheet);
                }
                Err(e) => self.message = e,
            }
        }
        if ui.button("Exporter la nomenclature CSV").clicked() {
            if let Some(path) = rfd::FileDialog::new()
                .add_filter("Nomenclature CSV", &["csv"])
                .set_file_name("nomenclature.csv")
                .save_file()
            {
                self.message = match fs::write(&path, drawing::bom_csv(project)) {
                    Ok(()) => "Nomenclature exportée.".into(),
                    Err(e) => e.to_string(),
                };
            }
        }
        ui.small("Feuille A3, jusqu’à 10 références. Les cotes sont calculées sur la géométrie assemblée. Les tolérances sont celles que vous saisissez.");
    }
}
fn scalar(ui: &mut egui::Ui, label: &str, value: &mut f64, suffix: &str) -> bool {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(
            egui::DragValue::new(value)
                .speed(0.1)
                .max_decimals(4)
                .suffix(suffix),
        )
        .changed()
    })
    .inner
}
fn vector(ui: &mut egui::Ui, value: &mut [f64; 3]) {
    for (i, label) in ["X", "Y", "Z"].iter().enumerate() {
        ui.add(
            egui::DragValue::new(&mut value[i])
                .speed(0.1)
                .prefix(format!("{label} "))
                .max_decimals(3),
        );
    }
}
fn axis(ui: &mut egui::Ui, id: &str, value: &mut [f64; 3]) {
    let label = if *value == [1.0, 0.0, 0.0] {
        "X"
    } else if *value == [0.0, 1.0, 0.0] {
        "Y"
    } else {
        "Z"
    };
    egui::ComboBox::from_id_salt(id)
        .selected_text(label)
        .show_ui(ui, |ui| {
            for (label, v) in [
                ("X", [1.0, 0.0, 0.0]),
                ("Y", [0.0, 1.0, 0.0]),
                ("Z", [0.0, 0.0, 1.0]),
            ] {
                ui.selectable_value(value, v, label);
            }
        });
}
fn hole_label(kind: HoleKind) -> &'static str {
    match kind {
        HoleKind::Simple => "Simple / borgne",
        HoleKind::Counterbore => "Lamage cylindrique",
        HoleKind::Countersink => "Fraisure conique",
        HoleKind::Threaded => "Taraudage conventionnel",
    }
}
fn preview(ui: &mut egui::Ui, sheet: &Sheet, scale: f32) {
    let (rect, _) = ui.allocate_exact_size(
        Vec2::new(420.0 * scale, 297.0 * scale),
        egui::Sense::hover(),
    );
    let p = ui.painter_at(rect);
    p.rect_filled(rect, 0.0, Color32::WHITE);
    let at = |a: [f64; 2]| rect.min + Vec2::new(a[0] as f32 * scale, a[1] as f32 * scale);
    let stroke = egui::Stroke::new(0.7, Color32::from_rgb(42, 53, 67));
    for e in &sheet.elements {
        match e {
            Element::Line { a, b, hidden } => {
                if *hidden {
                    let a = at(*a);
                    let b = at(*b);
                    let n = (a.distance(b) / 4.0).ceil().max(1.0) as usize;
                    for i in (0..n).step_by(2) {
                        p.line_segment(
                            [
                                a + (b - a) * (i as f32 / n as f32),
                                a + (b - a) * ((i + 1).min(n) as f32 / n as f32),
                            ],
                            egui::Stroke::new(0.5, Color32::from_gray(160)),
                        );
                    }
                } else {
                    p.line_segment([at(*a), at(*b)], stroke);
                }
            }
            Element::Rect { at: a, size } => {
                p.rect_stroke(
                    egui::Rect::from_min_size(
                        at(*a),
                        Vec2::new(size[0] as f32 * scale, size[1] as f32 * scale),
                    ),
                    0.0,
                    stroke,
                    egui::StrokeKind::Inside,
                );
            }
            Element::Circle { at: a, radius } => {
                p.circle(at(*a), *radius as f32 * scale, Color32::WHITE, stroke);
            }
            Element::Text { at: a, text, size } => {
                p.text(
                    at(*a),
                    egui::Align2::LEFT_BOTTOM,
                    text,
                    egui::FontId::proportional(*size as f32 * scale),
                    Color32::from_rgb(42, 53, 67),
                );
            }
        }
    }
}
pub fn inserted_project(project: &Project) -> Result<Option<Project>, String> {
    let Some(path) = rfd::FileDialog::new()
        .add_filter("Pièce ou sous-ensemble SolidCraft", &["solidcraft", "json"])
        .pick_file()
    else {
        return Ok(None);
    };
    if fs::metadata(&path).map_err(|e| e.to_string())?.len() > 2_000_000 {
        return Err("Le fichier dépasse 2 Mo.".into());
    }
    let source = Project::from_json(&fs::read_to_string(path).map_err(|e| e.to_string())?)?;
    let mut next = project.clone();
    assembly::insert_project(&mut next, &source)?;
    Ok(Some(next))
}
pub fn display_meshes(project: &Project, meshes: &[Mesh], amount: f64) -> Vec<Mesh> {
    project
        .bodies
        .iter()
        .zip(meshes)
        .map(|(body, m)| {
            let delta = project
                .assembly
                .explode
                .iter()
                .find(|e| e.body == body.id)
                .map(|e| Vec3::from_array(e.delta) * amount)
                .unwrap_or_default();
            m.transformed(delta.array(), [0.0; 3])
        })
        .collect()
}
