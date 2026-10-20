//! FeatureManager tree and PropertyManager.
use super::*;

#[derive(Clone, Copy)]
pub enum Glyph {
    Part,
    Material,
    Plane,
    Origin,
    Sketch,
    Extrude,
    Cut,
    Revolve,
    Fillet,
    Chamfer,
    Hole,
    Pattern,
    Mirror,
    Line,
    Rect,
    Circle,
    Arc,
    Slot,
    Dimension,
    Centerline,
    Measure,
    Mass,
    Rebuild,
    Export,
    NormalTo,
}
pub fn glyph_of(kind: &FeatureKind) -> Glyph {
    match kind {
        FeatureKind::Sketch(_) => Glyph::Sketch,
        FeatureKind::Extrude(e) if e.cut => Glyph::Cut,
        FeatureKind::Extrude(_) => Glyph::Extrude,
        FeatureKind::Revolve(_) => Glyph::Revolve,
        FeatureKind::Finish(f) if f.kind == FinishKind::Chamfer => Glyph::Chamfer,
        FeatureKind::Finish(_) => Glyph::Fillet,
        FeatureKind::Hole(_) => Glyph::Hole,
        FeatureKind::Pattern(p) if matches!(p.kind, PatternKind::Mirror { .. }) => Glyph::Mirror,
        FeatureKind::Pattern(_) => Glyph::Pattern,
        FeatureKind::Plane(_) => Glyph::Plane,
    }
}
/// Small vector icons for the tree.
pub fn paint_glyph(p: &egui::Painter, rect: egui::Rect, g: Glyph, dim: bool) {
    let c = rect.center();
    let line = if dim {
        Color32::from_gray(160)
    } else {
        Color32::from_rgb(60, 80, 105)
    };
    let fill = if dim {
        Color32::from_gray(225)
    } else {
        Color32::from_rgb(190, 210, 232)
    };
    let accent = if dim {
        Color32::from_gray(190)
    } else {
        Color32::from_rgb(230, 170, 40)
    };
    let k = rect.width() / 16.0;
    let st = Stroke::new(1.1 * k.sqrt(), line);
    let at = |x: f32, y: f32| c + Vec2::new(x, y) * k;
    let blue = Stroke::new(1.6 * k.sqrt(), Color32::from_rgb(40, 90, 200));
    match g {
        Glyph::Part | Glyph::Extrude | Glyph::Cut => {
            p.add(egui::Shape::convex_polygon(
                vec![at(-6.0, -2.0), at(0.0, -5.0), at(6.0, -2.0), at(0.0, 1.0)],
                Color32::from_rgb(230, 238, 246),
                st,
            ));
            p.add(egui::Shape::convex_polygon(
                vec![at(-6.0, -2.0), at(0.0, 1.0), at(0.0, 7.0), at(-6.0, 4.0)],
                fill,
                st,
            ));
            p.add(egui::Shape::convex_polygon(
                vec![at(0.0, 1.0), at(6.0, -2.0), at(6.0, 4.0), at(0.0, 7.0)],
                fill,
                st,
            ));
            if matches!(g, Glyph::Cut) {
                p.add(egui::Shape::convex_polygon(
                    vec![at(1.0, -4.0), at(4.0, -2.5), at(1.0, -1.0), at(-2.0, -2.5)],
                    Color32::from_rgb(120, 130, 145),
                    Stroke::NONE,
                ));
            }
            if matches!(g, Glyph::Extrude) {
                p.line_segment([at(6.0, -6.0), at(6.0, -11.0)], Stroke::new(1.4, accent));
            }
        }
        Glyph::Material => {
            for k in 0..3 {
                let y = -4.0 + k as f32 * 4.0;
                p.add(egui::Shape::convex_polygon(
                    vec![at(-6.0, y), at(0.0, y - 2.0), at(6.0, y), at(0.0, y + 2.0)],
                    fill,
                    st,
                ));
            }
        }
        Glyph::Plane => {
            p.add(egui::Shape::convex_polygon(
                vec![at(-7.0, 4.0), at(-3.0, -4.0), at(7.0, -4.0), at(3.0, 4.0)],
                Color32::from_rgba_unmultiplied(150, 190, 230, 120),
                st,
            ));
        }
        Glyph::Origin => {
            p.line_segment(
                [at(-6.0, 0.0), at(6.0, 0.0)],
                Stroke::new(1.2, Color32::from_rgb(200, 60, 60)),
            );
            p.line_segment(
                [at(0.0, -6.0), at(0.0, 6.0)],
                Stroke::new(1.2, Color32::from_rgb(60, 140, 60)),
            );
            p.circle_stroke(c, 2.5 * k, st);
        }
        Glyph::Sketch => {
            p.rect_stroke(
                egui::Rect::from_center_size(c, Vec2::splat(11.0 * k)),
                1.0,
                Stroke::new(1.0, Color32::from_rgb(70, 110, 190)),
                egui::StrokeKind::Inside,
            );
            p.line_segment([at(-3.0, 4.0), at(5.0, -4.0)], Stroke::new(2.0, accent));
        }
        Glyph::Revolve => {
            p.add(egui::Shape::ellipse_stroke(c, Vec2::new(6.5, 3.0) * k, st));
            p.line_segment([at(0.0, -7.0), at(0.0, 7.0)], Stroke::new(1.0, line));
            p.add(egui::Shape::convex_polygon(
                vec![at(5.0, 1.0), at(8.0, 3.0), at(4.0, 4.0)],
                accent,
                Stroke::NONE,
            ));
        }
        Glyph::Fillet => {
            let pts: Vec<Pos2> = (0..=8)
                .map(|k| {
                    let t = std::f32::consts::FRAC_PI_2 * k as f32 / 8.0;
                    at(-5.0 + 10.0 * t.cos(), 5.0 - 10.0 * t.sin())
                })
                .collect();
            p.add(egui::Shape::line(pts, Stroke::new(1.6, line)));
            p.line_segment([at(-5.0, 5.0), at(5.0, 5.0)], st);
        }
        Glyph::Chamfer => {
            p.add(egui::Shape::line(
                vec![
                    at(-6.0, -6.0),
                    at(-6.0, 6.0),
                    at(6.0, 6.0),
                    at(6.0, 0.0),
                    at(0.0, -6.0),
                    at(-6.0, -6.0),
                ],
                st,
            ));
            p.line_segment([at(6.0, 0.0), at(0.0, -6.0)], Stroke::new(1.8, accent));
        }
        Glyph::Hole => {
            p.circle_filled(c, 6.0 * k, fill);
            p.circle_stroke(c, 6.0 * k, st);
            p.circle_filled(c, 2.5 * k, line);
        }
        Glyph::Pattern => {
            for (x, y) in [(-4.0, -4.0), (4.0, -4.0), (-4.0, 4.0), (4.0, 4.0)] {
                p.rect_filled(
                    egui::Rect::from_center_size(at(x, y), Vec2::splat(5.0 * k)),
                    1.0,
                    fill,
                );
                p.rect_stroke(
                    egui::Rect::from_center_size(at(x, y), Vec2::splat(5.0 * k)),
                    1.0,
                    st,
                    egui::StrokeKind::Inside,
                );
            }
        }
        Glyph::Mirror => {
            p.add(egui::Shape::convex_polygon(
                vec![at(-1.5, -6.0), at(-1.5, 6.0), at(-7.0, 6.0)],
                fill,
                st,
            ));
            p.add(egui::Shape::convex_polygon(
                vec![at(1.5, -6.0), at(7.0, 6.0), at(1.5, 6.0)],
                fill,
                st,
            ));
            p.line_segment([at(0.0, -8.0), at(0.0, 8.0)], Stroke::new(1.0, accent));
        }
        Glyph::Line => {
            p.line_segment([at(-6.0, 5.0), at(6.0, -5.0)], blue);
            p.circle_filled(at(-6.0, 5.0), 1.6 * k, line);
            p.circle_filled(at(6.0, -5.0), 1.6 * k, line);
        }
        Glyph::Rect => {
            p.rect_stroke(
                egui::Rect::from_center_size(c, Vec2::new(13.0, 9.0) * k),
                0.0,
                blue,
                egui::StrokeKind::Inside,
            );
        }
        Glyph::Circle => {
            p.circle_stroke(c, 6.0 * k, blue);
            p.circle_filled(c, 1.3 * k, line);
        }
        Glyph::Arc => {
            let pts: Vec<Pos2> = (0..=10)
                .map(|i| {
                    let t = std::f32::consts::PI * i as f32 / 10.0;
                    at(6.0 * t.cos(), 3.0 - 6.0 * t.sin())
                })
                .collect();
            p.add(egui::Shape::line(pts, blue));
            p.circle_filled(at(0.0, 3.0), 1.3 * k, line);
        }
        Glyph::Slot => {
            p.rect_stroke(
                egui::Rect::from_center_size(c, Vec2::new(14.0, 7.0) * k),
                3.5 * k,
                blue,
                egui::StrokeKind::Inside,
            );
        }
        Glyph::Dimension => {
            p.line_segment([at(-7.0, -5.0), at(-7.0, 5.0)], st);
            p.line_segment([at(7.0, -5.0), at(7.0, 5.0)], st);
            p.line_segment([at(-7.0, 2.0), at(7.0, 2.0)], blue);
            p.text(
                at(0.0, -3.0),
                egui::Align2::CENTER_CENTER,
                "25",
                egui::FontId::proportional(6.5 * k),
                line,
            );
        }
        Glyph::Centerline => {
            crate::sketcher::dashed(
                p,
                &[at(-7.0, 6.0), at(7.0, -6.0)],
                Stroke::new(1.2 * k.sqrt(), Color32::from_rgb(40, 90, 200)),
            );
        }
        Glyph::Measure => {
            p.rect_stroke(
                egui::Rect::from_center_size(c, Vec2::new(15.0, 6.0) * k),
                1.0,
                st,
                egui::StrokeKind::Inside,
            );
            for i in 0..5 {
                let x = -6.0 + i as f32 * 3.0;
                p.line_segment(
                    [at(x, -3.0), at(x, if i % 2 == 0 { 0.0 } else { -1.5 })],
                    st,
                );
            }
        }
        Glyph::Mass => {
            p.add(egui::Shape::convex_polygon(
                vec![at(-6.0, 6.0), at(-3.0, -3.0), at(3.0, -3.0), at(6.0, 6.0)],
                fill,
                st,
            ));
            p.circle_stroke(at(0.0, -5.0), 2.0 * k, st);
        }
        Glyph::Rebuild => {
            let pts: Vec<Pos2> = (0..=12)
                .map(|i| {
                    let t = 0.3 + 5.0 * i as f32 / 12.0;
                    at(6.0 * t.cos(), 6.0 * t.sin())
                })
                .collect();
            p.add(egui::Shape::line(
                pts,
                Stroke::new(1.6 * k.sqrt(), Color32::from_rgb(40, 150, 60)),
            ));
            p.circle_filled(
                at(6.0 * 5.3f32.cos(), 6.0 * 5.3f32.sin()),
                2.0 * k,
                Color32::from_rgb(40, 150, 60),
            );
        }
        Glyph::Export => {
            p.rect_filled(
                egui::Rect::from_center_size(at(-2.0, 0.0), Vec2::new(9.0, 13.0) * k),
                1.0,
                Color32::WHITE,
            );
            p.rect_stroke(
                egui::Rect::from_center_size(at(-2.0, 0.0), Vec2::new(9.0, 13.0) * k),
                1.0,
                st,
                egui::StrokeKind::Inside,
            );
            p.line_segment(
                [at(0.0, 0.0), at(8.0, 0.0)],
                Stroke::new(1.8 * k.sqrt(), Color32::from_rgb(40, 150, 60)),
            );
            p.line_segment([at(5.0, -3.0), at(8.0, 0.0)], st);
            p.line_segment([at(5.0, 3.0), at(8.0, 0.0)], st);
        }
        Glyph::NormalTo => {
            p.add(egui::Shape::convex_polygon(
                vec![at(-7.0, 5.0), at(-3.0, -1.0), at(7.0, -1.0), at(3.0, 5.0)],
                fill,
                st,
            ));
            p.line_segment([at(0.0, 2.0), at(0.0, -8.0)], blue);
        }
    }
}
struct Row {
    clicked: bool,
    double: bool,
    hovered: bool,
    response: egui::Response,
}
// Independent visual attributes keep the immediate-mode tree row call sites explicit.
#[allow(clippy::too_many_arguments)]
fn tree_row(
    ui: &mut egui::Ui,
    indent: f32,
    glyph: Glyph,
    text: &str,
    selected: bool,
    error: Option<&str>,
    faded: bool,
    toggle: Option<bool>,
) -> (Row, bool) {
    let h = 22.0;
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(ui.available_width(), h), egui::Sense::click());
    let p = ui.painter_at(rect);
    if selected {
        p.rect_filled(rect, 2.0, Color32::from_rgb(204, 228, 252));
    } else if response.hovered() {
        p.rect_filled(rect, 2.0, Color32::from_rgb(232, 241, 251));
    }
    let mut toggled = false;
    let x0 = rect.left() + 4.0 + indent;
    if let Some(open) = toggle {
        let tri =
            egui::Rect::from_center_size(egui::pos2(x0 + 5.0, rect.center().y), Vec2::splat(10.0));
        let pts = if open {
            vec![
                tri.left_top() + Vec2::new(1.0, 2.0),
                tri.right_top() + Vec2::new(-1.0, 2.0),
                tri.center_bottom() + Vec2::new(0.0, -2.0),
            ]
        } else {
            vec![
                tri.left_top() + Vec2::new(2.0, 1.0),
                tri.right_center() + Vec2::new(-2.0, 0.0),
                tri.left_bottom() + Vec2::new(2.0, -1.0),
            ]
        };
        p.add(egui::Shape::convex_polygon(pts, MUTED, Stroke::NONE));
        if response.clicked()
            && ui
                .input(|i| i.pointer.interact_pos())
                .is_some_and(|q| q.x < x0 + 12.0)
        {
            toggled = true;
        }
    }
    let icon =
        egui::Rect::from_center_size(egui::pos2(x0 + 22.0, rect.center().y), Vec2::splat(16.0));
    paint_glyph(&p, icon, glyph, faded);
    let color = if error.is_some() {
        ERROR
    } else if faded {
        Color32::from_gray(150)
    } else {
        Color32::from_rgb(30, 34, 40)
    };
    let mut label = text.to_string();
    if error.is_some() {
        label = format!("⚠ {label}");
    }
    p.text(
        egui::pos2(x0 + 34.0, rect.center().y),
        egui::Align2::LEFT_CENTER,
        label,
        egui::FontId::proportional(13.0),
        color,
    );
    let response = match error {
        Some(e) => response.on_hover_text(e),
        None => response,
    };
    (
        Row {
            clicked: response.clicked() && !toggled,
            double: response.double_clicked(),
            hovered: response.hovered(),
            response,
        },
        toggled,
    )
}

impl PartEditor {
    /// Left panel: the PropertyManager while a command runs, the FeatureManager otherwise.
    pub fn left_panel(
        &mut self,
        ui: &mut egui::Ui,
        bodies: &[Body],
        title: &str,
        view: &mut Viewport,
    ) {
        let ctx = ui.ctx().clone();
        self.tree_plane = None;
        self.tree_hover = None;
        match &self.mode {
            Mode::Idle => self.tree(ui, bodies, title, view),
            Mode::PickPlane(then) => {
                let then = *then;
                if self.pm_header(
                    ui,
                    then.map(|c| c.label()).unwrap_or("Esquisse"),
                    Glyph::Sketch,
                    false,
                ) == Some(false)
                {
                    self.cancel();
                }
                ui.add_space(6.0);
                group(ui, "Message", |ui| {
                    ui.label("Sélectionnez un plan dans la vue ou dans la liste, ou une face plane de la pièce.");
                });
                group(ui, "Plans de référence", |ui| {
                    for r in [PlaneRef::Front, PlaneRef::Top, PlaneRef::Right] {
                        let resp = ui.selectable_label(
                            false,
                            r.label(self.part(bodies).unwrap_or(&Part::default())),
                        );
                        if resp.hovered() {
                            self.tree_plane = Some(r);
                        }
                        if resp.clicked() {
                            let plane = std_plane(r);
                            self.new_sketch(bodies, plane, view, &ctx, then);
                        }
                    }
                });
            }
            Mode::Measure => {
                if self.pm_header(ui, "Mesurer", Glyph::Origin, false) == Some(false) {
                    self.mode = Mode::Idle;
                }
                group(ui, "Résultats", |ui| {
                    for line in self.measure_text() {
                        ui.label(line);
                        ui.add_space(4.0);
                    }
                    ui.small("Ctrl+clic pour sélectionner une seconde entité.");
                });
            }
            Mode::Sketch(..) => self.sketch_pm(ui, bodies, view),
            Mode::Feature { .. } => self.feature_pm(ui, bodies, view),
        }
    }
    /// Title with OK and Cancel. Returns Some(true) for OK, Some(false) for Cancel.
    fn pm_header(&self, ui: &mut egui::Ui, title: &str, glyph: Glyph, ok: bool) -> Option<bool> {
        let mut out = None;
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            let (r, _) = ui.allocate_exact_size(Vec2::splat(20.0), egui::Sense::hover());
            paint_glyph(ui.painter(), r, glyph, false);
            ui.label(RichText::new(title).strong().size(15.0));
        });
        ui.horizontal(|ui| {
            if ok
                && ui
                    .add(
                        egui::Button::new(
                            RichText::new("✔")
                                .size(18.0)
                                .color(Color32::from_rgb(30, 150, 60)),
                        )
                        .min_size(Vec2::new(34.0, 28.0)),
                    )
                    .on_hover_text("OK (Entrée)")
                    .clicked()
            {
                out = Some(true);
            }
            if ui
                .add(
                    egui::Button::new(RichText::new("✖").size(18.0).color(ERROR))
                        .min_size(Vec2::new(34.0, 28.0)),
                )
                .on_hover_text("Annuler (Échap)")
                .clicked()
            {
                out = Some(false);
            }
        });
        if !self.message.is_empty() {
            ui.colored_label(Color32::from_rgb(170, 80, 20), &self.message);
        }
        ui.separator();
        out
    }
    fn tree(&mut self, ui: &mut egui::Ui, bodies: &[Body], title: &str, view: &mut Viewport) {
        let ctx = ui.ctx().clone();
        self.tree_hover = None;
        self.tree_plane = None;
        let Some(part) = self.part(bodies).cloned() else {
            ui.add_space(10.0);
            ui.label("Ce projet ne contient pas de pièce paramétrique.");
            if ui.button("Créer une pièce paramétrique").clicked() {
                self.pending
                    .push(Action::Commit(Part::default(), "Nouvelle pièce".into()));
            }
            return;
        };
        ui.add_space(4.0);
        let (row, _) = tree_row(ui, 0.0, Glyph::Part, title, false, None, false, None);
        let _ = row;
        let mut material = part.material.clone();
        let (mrow, _) = tree_row(
            ui,
            10.0,
            Glyph::Material,
            &format!("Matériau : {}", material.name),
            false,
            None,
            false,
            None,
        );
        mrow.response.context_menu(|ui| {
            for (name, density) in part::MATERIALS {
                if ui.button(format!("{name} · {density} g/cm³")).clicked() {
                    material = part::Material {
                        name: name.into(),
                        density,
                    };
                    ui.close_menu();
                }
            }
        });
        if mrow.clicked {
            self.message = "Clic droit sur Matériau pour le changer.".into();
        }
        if material != part.material {
            let mut p = part.clone();
            p.material = material;
            self.pending.push(Action::Commit(p, "Matériau".into()));
        }
        for r in [PlaneRef::Front, PlaneRef::Top, PlaneRef::Right] {
            let (row, _) = tree_row(
                ui,
                10.0,
                Glyph::Plane,
                &r.label(&part),
                self.sel.contains(&Sel::Plane(r)),
                None,
                false,
                None,
            );
            if row.hovered {
                self.tree_plane = Some(r);
            }
            if row.clicked {
                self.select_tree(Sel::Plane(r), ui);
            }
            row.response.context_menu(|ui| {
                if ui.button("Esquisse").clicked() {
                    self.sel = vec![Sel::Plane(r)];
                    self.start_sketch(bodies, view, &ctx, None);
                    ui.close_menu();
                }
                if ui.button("Normal à").clicked() {
                    self.sel = vec![Sel::Plane(r)];
                    self.normal_to(bodies, view, &ctx);
                    ui.close_menu();
                }
            });
        }
        tree_row(ui, 10.0, Glyph::Origin, "Origine", false, None, false, None);
        let consumed: std::collections::HashMap<u32, u32> = part
            .features
            .iter()
            .filter_map(|f| f.kind.sketch().map(|s| (s, f.id)))
            .collect();
        let end = part.rollback.unwrap_or(part.features.len());
        let mut action: Option<(u32, &str)> = None;
        for (i, f) in part.features.iter().enumerate() {
            if i == end {
                self.rollback_bar(ui, &part);
            }
            if consumed.contains_key(&f.id) {
                continue;
            }
            let faded = f.suppressed || i >= end;
            let child = f.kind.sketch();
            let open = self.expanded.contains(&f.id);
            let (row, toggled) = tree_row(
                ui,
                10.0,
                glyph_of(&f.kind),
                &f.name,
                self.sel.contains(&Sel::Feature(f.id)),
                self.regen.error(f.id),
                faded,
                child.map(|_| open),
            );
            if toggled {
                if open {
                    self.expanded.remove(&f.id);
                } else {
                    self.expanded.insert(f.id);
                }
            }
            if row.hovered {
                self.tree_hover = Some(f.id);
            }
            if row.clicked {
                self.select_tree(Sel::Feature(f.id), ui);
            }
            if row.double {
                action = Some((f.id, "edit"));
            }
            self.feature_menu(&row.response, &part, f.id, i, &mut action);
            if let (Some(s), true) = (child, open) {
                if let Some(sf) = part.feature(s) {
                    let (row, _) = tree_row(
                        ui,
                        30.0,
                        Glyph::Sketch,
                        &sf.name,
                        self.sel.contains(&Sel::Feature(s)),
                        self.regen.error(s),
                        faded || sf.suppressed,
                        None,
                    );
                    if row.hovered {
                        self.tree_hover = Some(s);
                    }
                    if row.clicked {
                        self.select_tree(Sel::Feature(s), ui);
                    }
                    if row.double {
                        action = Some((s, "edit"));
                    }
                    let idx = part.index(s).unwrap_or(0);
                    self.feature_menu(&row.response, &part, s, idx, &mut action);
                }
            }
            if let Some((id, name)) = &mut self.renaming {
                if *id == f.id {
                    let r = ui.add(
                        egui::TextEdit::singleline(name).desired_width(ui.available_width() - 10.0),
                    );
                    r.request_focus();
                    if r.lost_focus() {
                        let (id, name) = self.renaming.take().unwrap();
                        let mut p = part.clone();
                        if let Some(f) = p.feature_mut(id) {
                            if !name.trim().is_empty() {
                                f.name = name.trim().chars().take(100).collect();
                            }
                        }
                        self.pending.push(Action::Commit(p, "Renommer".into()));
                    }
                }
            }
        }
        if end >= part.features.len() && part.rollback.is_some() {
            self.rollback_bar(ui, &part);
        }
        if part.features.is_empty() {
            ui.add_space(12.0);
            ui.label(
                RichText::new(
                    "Pour commencer : choisissez un plan (Dessus par exemple), puis Esquisse.",
                )
                .color(MUTED),
            );
        }
        if let Some((id, what)) = action {
            match what {
                "edit" => self.edit_feature(bodies, id, view, &ctx),
                "sketch" => self.edit_sketch(bodies, id, view, &ctx),
                "suppress" | "delete" | "rollback" | "rollend" => {
                    let mut p = part.clone();
                    match what {
                        "suppress" => {
                            if let Some(f) = p.feature_mut(id) {
                                f.suppressed = !f.suppressed;
                            }
                        }
                        "delete" => p.delete(id),
                        "rollback" => p.rollback = p.index(id),
                        _ => p.rollback = None,
                    }
                    self.sel.clear();
                    self.pending
                        .push(Action::Commit(p, "Arbre de construction".into()));
                }
                _ => {}
            }
        }
        // Other bodies of the project (imported components, primitives of version 0.2).
        let others: Vec<usize> = (0..bodies.len())
            .filter(|i| Some(*i) != self.body)
            .collect();
        if !others.is_empty() {
            ui.add_space(8.0);
            ui.label(
                RichText::new(format!("Autres corps ({})", others.len()))
                    .size(11.0)
                    .color(MUTED),
            );
            for i in others {
                let (row, _) = tree_row(
                    ui,
                    10.0,
                    Glyph::Part,
                    &bodies[i].name,
                    self.sel.contains(&Sel::Body(i)),
                    None,
                    !bodies[i].visible,
                    None,
                );
                if row.clicked {
                    self.select_tree(Sel::Body(i), ui);
                }
            }
        }
    }
    fn rollback_bar(&self, ui: &mut egui::Ui, part: &Part) {
        let (rect, resp) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 10.0), egui::Sense::click());
        ui.painter().line_segment(
            [
                rect.left_center() + Vec2::new(8.0, 0.0),
                rect.right_center() - Vec2::new(8.0, 0.0),
            ],
            Stroke::new(3.0, Color32::from_rgb(40, 110, 220)),
        );
        let resp = resp.on_hover_text("Barre de reprise : les fonctions dessous ne sont pas reconstruites. Clic droit sur une fonction › Reprendre à la fin.");
        let _ = (resp, part);
    }
    fn select_tree(&mut self, s: Sel, ui: &egui::Ui) {
        if ui.input(|i| i.modifiers.ctrl || i.modifiers.shift) {
            if let Some(i) = self.sel.iter().position(|x| *x == s) {
                self.sel.remove(i);
            } else {
                self.sel.push(s);
            }
        } else {
            self.sel = vec![s];
        }
    }
    fn feature_menu(
        &mut self,
        response: &egui::Response,
        part: &Part,
        id: u32,
        index: usize,
        action: &mut Option<(u32, &'static str)>,
    ) {
        let f = part.feature(id).cloned();
        response.context_menu(|ui| {
            let Some(f) = f else {
                return;
            };
            if ui.button("⚙ Éditer la fonction").clicked() {
                *action = Some((id, "edit"));
                ui.close_menu();
            }
            if let Some(s) = f.kind.sketch() {
                if ui.button("✏ Éditer l’esquisse").clicked() {
                    *action = Some((s, "sketch"));
                    ui.close_menu();
                }
            }
            if ui.button("Renommer").clicked() {
                self.renaming = Some((id, f.name.clone()));
                ui.close_menu();
            }
            if ui
                .button(if f.suppressed {
                    "Réactiver"
                } else {
                    "Désactiver (supprimer temporairement)"
                })
                .clicked()
            {
                *action = Some((id, "suppress"));
                ui.close_menu();
            }
            ui.separator();
            if ui.button("Reprendre avant cette fonction").clicked() {
                *action = Some((id, "rollback"));
                ui.close_menu();
            }
            if part.rollback.is_some() && ui.button("Reprendre à la fin").clicked() {
                *action = Some((id, "rollend"));
                ui.close_menu();
            }
            ui.separator();
            if ui
                .button(RichText::new("🗑 Supprimer").color(ERROR))
                .clicked()
            {
                *action = Some((id, "delete"));
                ui.close_menu();
            }
            let _ = index;
        });
    }
    fn sketch_pm(&mut self, ui: &mut egui::Ui, bodies: &[Body], view: &mut Viewport) {
        let ctx = ui.ctx().clone();
        let Mode::Sketch(edit, _) = &self.mode else {
            return;
        };
        let name = self
            .draft
            .as_ref()
            .and_then(|d| d.feature(edit.feature))
            .map(|f| f.name.clone())
            .unwrap_or_default();
        match self.pm_header(ui, &name, Glyph::Sketch, true) {
            Some(true) => {
                self.exit_sketch(bodies, view, &ctx, true);
                return;
            }
            Some(false) => {
                self.exit_sketch(bodies, view, &ctx, false);
                return;
            }
            None => {}
        }
        let Mode::Sketch(edit, _) = &mut self.mode else {
            return;
        };
        let id = edit.feature;
        let Some(FeatureKind::Sketch(sketch)) = self
            .draft
            .as_mut()
            .and_then(|d| d.feature_mut(id))
            .map(|f| &mut f.kind)
        else {
            return;
        };
        egui::ScrollArea::vertical().show(ui, |ui| {
            group(ui, "Outils d’esquisse", |ui| {
                egui::Grid::new("sketch-tools").num_columns(2).spacing([4.0, 4.0]).show(ui, |ui| {
                    for (k, tool) in [Tool::Select, Tool::Dimension, Tool::Line, Tool::Rectangle, Tool::CenterRectangle, Tool::Circle, Tool::Arc, Tool::Slot, Tool::Centerline].into_iter().enumerate() {
                        let key = match tool {
                            Tool::Line => " (L)",
                            Tool::Rectangle => " (R)",
                            Tool::Circle => " (C)",
                            Tool::Dimension => " (D)",
                            Tool::Arc => " (A)",
                            _ => "",
                        };
                        if ui.add_sized([128.0, 24.0], egui::SelectableLabel::new(edit.tool == tool, format!("{}{key}", tool.label()))).clicked() {
                            edit.set_tool(tool);
                        }
                        if k % 2 == 1 {
                            ui.end_row();
                        }
                    }
                });
                ui.add_space(4.0);
                ui.label(RichText::new(edit.hint()).color(MUTED).size(12.0));
            });
            if !edit.message.is_empty() {
                ui.colored_label(Color32::from_rgb(170, 80, 20), &edit.message);
            }
            let rels = edit.possible_relations(sketch);
            if !edit.sel.is_empty() {
                group(ui, "Sélection", |ui| {
                    let names: Vec<String> = edit
                        .sel
                        .iter()
                        .map(|s| match *s {
                            sketcher::Item::Point(0) => "Origine".into(),
                            sketcher::Item::Point(p) => format!("Point{p}"),
                            sketcher::Item::Entity(e) => format!("{}{}", sketch.entities[e].label(), e + 1),
                            sketcher::Item::Dim(d) => format!("Cote {}", sketch.dims[d].text()),
                        })
                        .collect();
                    ui.label(names.join(", "));
                    if !rels.is_empty() {
                        ui.label(RichText::new("Ajouter des relations").strong());
                        ui.horizontal_wrapped(|ui| {
                            for r in rels {
                                if ui.button(r.label()).clicked() && edit.add_relation(sketch, r) {
                                    edit.changed();
                                }
                            }
                        });
                    }
                    let ents: Vec<usize> = edit.sel.iter().filter_map(|s| if let sketcher::Item::Entity(e) = s { Some(*e) } else { None }).collect();
                    if !ents.is_empty() {
                        let all = ents.iter().all(|e| sketch.entities[*e].construction());
                        let mut c = all;
                        if ui.checkbox(&mut c, "Géométrie de construction").changed() {
                            for e in &ents {
                                sketch.entities[*e].set_construction(c);
                            }
                        }
                    }
                    // Existing relations on the selection.
                    let existing: Vec<usize> = sketch
                        .rels
                        .iter()
                        .enumerate()
                        .filter(|(_, r)| {
                            r.entities().iter().any(|e| edit.sel.contains(&sketcher::Item::Entity(*e))) || r.points().iter().any(|p| edit.sel.contains(&sketcher::Item::Point(*p)))
                        })
                        .map(|(i, _)| i)
                        .collect();
                    if !existing.is_empty() {
                        ui.label(RichText::new("Relations existantes").strong());
                        let mut remove = None;
                        for i in existing {
                            ui.horizontal(|ui| {
                                ui.label(sketch.rels[i].label());
                                if ui.small_button("✖").on_hover_text("Supprimer la relation").clicked() {
                                    remove = Some(i);
                                }
                            });
                        }
                        if let Some(i) = remove {
                            sketch.rels.remove(i);
                            edit.changed();
                        }
                    }
                    if ui.button(RichText::new("🗑 Supprimer la sélection (Suppr)").color(ERROR)).clicked() {
                        edit.delete_selection(sketch);
                    }
                });
            }
            group(ui, "Options", |ui| {
                ui.checkbox(&mut edit.grid, "Accrocher à la grille");
                ui.checkbox(&mut edit.relations, "Afficher les relations");
                if ui.button("Normal à (Ctrl+8)").clicked() {
                    if let Some(frame) = self.regen.planes.get(&id).copied().or(sketch.plane) {
                        let rot = self.body.and_then(|i| bodies.get(i)).map(|b| b.rotation).unwrap_or([0.0; 3]);
                        view.normal_to(frame.u().rotate(rot), frame.v().rotate(rot), &ctx);
                    }
                }
            });
            group(ui, "État", |ui| {
                let st = &edit.status;
                if st.fully_defined() {
                    ui.colored_label(Color32::from_rgb(20, 20, 20), "Totalement contrainte");
                } else {
                    ui.colored_label(sketcher::SKETCH_BLUE, format!("Sous-contrainte · {} degré(s) de liberté", st.dof));
                }
                ui.small("Bleu : libre · Noir : entièrement défini. Ajoutez des cotes (D) et des relations pour tout définir.");
                match sketch.contours() {
                    Ok(c) => {
                        let holes = c.iter().filter(|c| c.depth % 2 == 1).count();
                        ui.small(format!("{} contour(s) fermé(s){}", c.len(), if holes > 0 { format!(", dont {holes} trou(s)") } else { String::new() }));
                    }
                    Err(e) => {
                        if !sketch.entities.is_empty() {
                            ui.colored_label(Color32::from_rgb(170, 80, 20), e);
                        }
                    }
                }
            });
        });
    }
    fn feature_pm(&mut self, ui: &mut egui::Ui, bodies: &[Body], view: &mut Viewport) {
        let ctx = ui.ctx().clone();
        let Mode::Feature { id, cmd, .. } = self.mode else {
            return;
        };
        let glyph = self
            .draft
            .as_ref()
            .and_then(|d| d.feature(id))
            .map(|f| glyph_of(&f.kind))
            .unwrap_or(Glyph::Extrude);
        match self.pm_header(ui, cmd.label(), glyph, true) {
            Some(true) => {
                self.ok(bodies, view, &ctx);
                return;
            }
            Some(false) => {
                self.cancel();
                return;
            }
            None => {}
        }
        if let Some(e) = self.regen.error(id) {
            ui.colored_label(ERROR, format!("⚠ {e}"));
            ui.separator();
        }
        let hole_face = self.hole_face;
        let topo_edges = self.topo.edges.len();
        let _ = topo_edges;
        let Some(draft) = self.draft.as_mut() else {
            return;
        };
        let sketches: Vec<(u32, String)> = draft
            .features
            .iter()
            .filter(|f| matches!(f.kind, FeatureKind::Sketch(_)))
            .map(|f| (f.id, f.name.clone()))
            .collect();
        let feature_names: Vec<(u32, String)> = draft
            .features
            .iter()
            .filter(|f| {
                !matches!(f.kind, FeatureKind::Sketch(_) | FeatureKind::Plane(_)) && f.id != id
            })
            .map(|f| (f.id, f.name.clone()))
            .collect();
        let axes: Vec<(usize, String)> = match draft.feature(id).map(|f| &f.kind) {
            Some(FeatureKind::Revolve(r)) => match draft.feature(r.sketch).map(|f| &f.kind) {
                Some(FeatureKind::Sketch(s)) => s
                    .entities
                    .iter()
                    .enumerate()
                    .filter(|(_, e)| matches!(e, Entity::Line { .. }))
                    .map(|(i, e)| (i, format!("{}{}", e.label(), i + 1)))
                    .collect(),
                _ => vec![],
            },
            _ => vec![],
        };
        let Some(f) = draft.feature_mut(id) else {
            return;
        };
        egui::ScrollArea::vertical().show(ui, |ui| match &mut f.kind {
            FeatureKind::Extrude(e) => {
                group(ui, "Depuis", |ui| {
                    combo(ui, "extrude-sketch", &mut e.sketch, &sketches);
                    ui.small("Plan d’esquisse");
                });
                group(ui, "Direction 1", |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("Inverser").on_hover_text("Inverser la direction").clicked() {
                            e.reverse = !e.reverse;
                        }
                        egui::ComboBox::from_id_salt("end-condition").selected_text(e.end.label()).show_ui(ui, |ui| {
                            for c in [EndCondition::Blind, EndCondition::ThroughAll, EndCondition::MidPlane] {
                                ui.selectable_value(&mut e.end, c, c.label());
                            }
                        });
                    });
                    if e.end != EndCondition::ThroughAll {
                        length(ui, "Profondeur D1", &mut e.depth);
                        ui.small("Astuce : glissez la flèche dans la vue.");
                    }
                });
                if !e.cut {
                    ui.small("Le résultat est fusionné avec le corps existant.");
                }
            }
            FeatureKind::Revolve(r) => {
                group(ui, "Axe de révolution", |ui| {
                    let mut axis = r.axis.unwrap_or(usize::MAX);
                    let mut choices = vec![(usize::MAX, "Première ligne de construction".to_string())];
                    choices.extend(axes.clone());
                    combo(ui, "revolve-axis", &mut axis, &choices);
                    r.axis = (axis != usize::MAX).then_some(axis);
                    ui.small("Tracez une ligne de construction dans l’esquisse pour définir l’axe.");
                });
                group(ui, "Paramètres", |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("Inverser").on_hover_text("Inverser le sens").clicked() {
                            r.reverse = !r.reverse;
                        }
                        angle(ui, "Angle", &mut r.angle);
                    });
                    combo(ui, "revolve-sketch", &mut r.sketch, &sketches);
                });
            }
            FeatureKind::Finish(fi) => {
                group(ui, "Type", |ui| {
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut fi.kind, FinishKind::Fillet, "Congé");
                        ui.selectable_value(&mut fi.kind, FinishKind::Chamfer, "Chanfrein");
                    });
                });
                group(ui, "Éléments", |ui| {
                    length(ui, if fi.kind == FinishKind::Fillet { "Rayon" } else { "Distance" }, &mut fi.size);
                    ui.label(format!("{} arête(s)", fi.edges.len()));
                    let mut remove = None;
                    for (i, _) in fi.edges.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(format!("Arête<{}>", i + 1));
                            if ui.small_button("✖").clicked() {
                                remove = Some(i);
                            }
                        });
                    }
                    if let Some(i) = remove {
                        fi.edges.remove(i);
                    }
                    ui.small("Cliquez des arêtes droites dans la vue pour les ajouter ou les retirer. Cliquer une face prend toutes ses arêtes droites.");
                });
            }
            FeatureKind::Hole(h) => {
                group(ui, "Type de trou", |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for (k, label) in [(HoleKind::Simple, "Simple"), (HoleKind::Counterbore, "Lamé"), (HoleKind::Countersink, "Fraisé"), (HoleKind::Threaded, "Taraudé")] {
                            if ui.selectable_label(h.kind == k, label).clicked() {
                                h.kind = k;
                                apply_standard(h);
                            }
                        }
                    });
                    let current = h.standard.split(' ').next().unwrap_or("M5").to_string();
                    egui::ComboBox::from_id_salt("hole-size").selected_text(format!("Taille : {current}")).show_ui(ui, |ui| {
                        for m in part::METRIC {
                            if ui.selectable_label(current == m.0, m.0).clicked() {
                                h.standard = m.0.into();
                                apply_standard(h);
                            }
                        }
                    });
                    ui.small(match h.kind {
                        HoleKind::Threaded => "Avant-trou de taraudage (ISO). Pour l’impression 3D, on peut aussi visser directement.",
                        HoleKind::Counterbore => "Passage de vis CHC avec lamage pour la tête.",
                        HoleKind::Countersink => "Passage de vis à tête fraisée 90°.",
                        _ => "Passage de vis, série normale (ISO 273).",
                    });
                });
                group(ui, "Spécification", |ui| {
                    length(ui, "Diamètre", &mut h.diameter);
                    if matches!(h.kind, HoleKind::Counterbore | HoleKind::Countersink) {
                        length(ui, "Ø de tête", &mut h.head_diameter);
                        if h.kind == HoleKind::Counterbore {
                            length(ui, "Profondeur de tête", &mut h.head_depth);
                        }
                    }
                    ui.checkbox(&mut h.through, "À travers tout");
                    if !h.through {
                        length(ui, "Profondeur", &mut h.depth);
                    }
                });
                group(ui, "Positions", |ui| {
                    if !hole_face {
                        ui.colored_label(Color32::from_rgb(170, 80, 20), "Cliquez une face plane dans la vue.");
                    } else {
                        ui.small("Cliquez sur la face pour placer d’autres trous. Coordonnées dans le plan de la face.");
                    }
                    let mut remove = None;
                    for (i, p) in h.points.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(format!("{}", i + 1));
                            ui.add(egui::DragValue::new(&mut p[0]).speed(0.1).prefix("X ").max_decimals(3));
                            ui.add(egui::DragValue::new(&mut p[1]).speed(0.1).prefix("Y ").max_decimals(3));
                            if ui.small_button("✖").clicked() {
                                remove = Some(i);
                            }
                        });
                    }
                    if let Some(i) = remove {
                        h.points.remove(i);
                    }
                });
            }
            FeatureKind::Pattern(p) => {
                match &mut p.kind {
                    PatternKind::Linear { direction, spacing, count } => {
                        group(ui, "Direction 1", |ui| {
                            axis_buttons(ui, direction);
                            length(ui, "Espacement", spacing);
                            count_field(ui, count);
                            ui.small("Cliquez une arête droite dans la vue pour donner la direction.");
                        });
                    }
                    PatternKind::Circular { origin, axis, angle: a, count } => {
                        group(ui, "Axe", |ui| {
                            axis_buttons(ui, axis);
                            ui.horizontal(|ui| {
                                ui.label("Passant par");
                                for (k, l) in ["X", "Y", "Z"].iter().enumerate() {
                                    ui.add(egui::DragValue::new(&mut origin[k]).speed(0.1).prefix(format!("{l} ")).max_decimals(3));
                                }
                            });
                            ui.small("Ou cliquez une arête droite (axe passant par son milieu).");
                        });
                        group(ui, "Paramètres", |ui| {
                            angle(ui, "Angle total", a);
                            count_field(ui, count);
                        });
                    }
                    PatternKind::Mirror { plane } => {
                        group(ui, "Plan de symétrie", |ui| {
                            for r in [PlaneRef::Front, PlaneRef::Top, PlaneRef::Right] {
                                let candidate = std_plane(r);
                                if ui.selectable_label(plane.face.is_none() && plane.normal().dot(candidate.normal()).abs() > 0.9999 && plane.origin().length() < 1e-9, r.label(&Part::default())).clicked() {
                                    *plane = candidate;
                                }
                            }
                            if plane.face.is_some() {
                                ui.label("Face de la pièce");
                            }
                            ui.small("Ou cliquez une face plane ou un plan dans la vue.");
                        });
                    }
                }
                group(ui, "Fonctions à répéter", |ui| {
                    let mut remove = None;
                    for (i, s) in p.seeds.iter().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(feature_names.iter().find(|f| f.0 == *s).map(|f| f.1.as_str()).unwrap_or("?"));
                            if ui.small_button("✖").clicked() {
                                remove = Some(i);
                            }
                        });
                    }
                    if let Some(i) = remove {
                        p.seeds.remove(i);
                    }
                    let mut add = u32::MAX;
                    let mut choices = vec![(u32::MAX, "Ajouter…".to_string())];
                    choices.extend(feature_names.iter().filter(|f| !p.seeds.contains(&f.0)).cloned());
                    combo(ui, "pattern-add", &mut add, &choices);
                    if add != u32::MAX {
                        p.seeds.push(add);
                    }
                    ui.small("Ou cliquez une face d’une fonction dans la vue.");
                });
            }
            FeatureKind::Plane(rp) => {
                group(ui, "Première référence", |ui| {
                    for r in [PlaneRef::Front, PlaneRef::Top, PlaneRef::Right] {
                        if ui.button(r.label(&Part::default())).clicked() {
                            rp.base = std_plane(r);
                        }
                    }
                    if rp.base.face.is_some() {
                        ui.label("Face de la pièce");
                    }
                    ui.small("Ou cliquez une face plane dans la vue.");
                });
                group(ui, "Décalage", |ui| {
                    ui.horizontal(|ui| {
                        if ui.button("Inverser").on_hover_text("Inverser").clicked() {
                            rp.offset = -rp.offset;
                        }
                        ui.add(egui::DragValue::new(&mut rp.offset).speed(0.1).suffix(" mm").max_decimals(3));
                    });
                });
            }
            FeatureKind::Sketch(_) => {}
        });
    }
}
fn apply_standard(h: &mut HoleWizard) {
    let name = h.standard.split(' ').next().unwrap_or("M5").to_string();
    let Some(m) = part::METRIC.iter().find(|m| m.0 == name) else {
        return;
    };
    match h.kind {
        HoleKind::Threaded => {
            h.diameter = m.5;
            h.standard = format!("{} – taraudé", m.0);
        }
        HoleKind::Counterbore => {
            h.diameter = m.1;
            h.head_diameter = m.2;
            h.head_depth = m.3;
            h.standard = format!("{} – lamé CHC", m.0);
        }
        HoleKind::Countersink => {
            h.diameter = m.1;
            h.head_diameter = m.4;
            h.standard = format!("{} – fraisé 90°", m.0);
        }
        HoleKind::Simple => {
            h.diameter = m.1;
            h.standard = format!("{} – passage normal", m.0);
        }
    }
}
pub fn group(ui: &mut egui::Ui, title: &str, add: impl FnOnce(&mut egui::Ui)) {
    ui.add_space(4.0);
    egui::Frame::group(ui.style())
        .fill(Color32::from_rgb(250, 251, 253))
        .inner_margin(8.0)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(title)
                    .strong()
                    .color(Color32::from_rgb(40, 70, 120)),
            );
            ui.add_space(2.0);
            add(ui);
        });
}
fn length(ui: &mut egui::Ui, label: &str, v: &mut f64) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(
            egui::DragValue::new(v)
                .speed(0.1)
                .range(0.01..=10_000.0)
                .suffix(" mm")
                .max_decimals(3),
        );
    });
}
fn angle(ui: &mut egui::Ui, label: &str, v: &mut f64) {
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(
            egui::DragValue::new(v)
                .speed(0.5)
                .range(0.1..=360.0)
                .suffix(" °")
                .max_decimals(2),
        );
    });
}
fn count_field(ui: &mut egui::Ui, v: &mut usize) {
    ui.horizontal(|ui| {
        ui.label("Nombre d’occurrences");
        ui.add(egui::DragValue::new(v).range(2..=200));
    });
}
fn axis_buttons(ui: &mut egui::Ui, v: &mut [f64; 3]) {
    ui.horizontal(|ui| {
        for (label, a) in [
            ("X", [1.0, 0.0, 0.0]),
            ("Y", [0.0, 1.0, 0.0]),
            ("Z", [0.0, 0.0, 1.0]),
        ] {
            if ui
                .selectable_label(*v == a || *v == a.map(|x: f64| -x), label)
                .clicked()
            {
                *v = a;
            }
        }
        if ui
            .button("Inverser")
            .on_hover_text("Inverser la direction")
            .clicked()
        {
            *v = v.map(|x| -x);
        }
    });
}
fn combo<T: PartialEq + Copy>(ui: &mut egui::Ui, id: &str, v: &mut T, choices: &[(T, String)]) {
    let current = choices
        .iter()
        .find(|c| c.0 == *v)
        .map(|c| c.1.clone())
        .unwrap_or_else(|| "—".into());
    egui::ComboBox::from_id_salt(id)
        .selected_text(current)
        .width(200.0)
        .show_ui(ui, |ui| {
            for (k, label) in choices {
                ui.selectable_value(v, *k, label);
            }
        });
}
