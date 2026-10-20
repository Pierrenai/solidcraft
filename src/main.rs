#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod cad;
mod sketcher;
mod viewport;
mod workbench;
use cad::{
    panels::{paint_glyph, Glyph},
    Action, Cmd, Mode, PartEditor, Sel,
};
use eframe::egui::{self, Color32, RichText, Vec2};
use sketcher::Tool;
use solidcraft::{
    features::FinishKind,
    geometry::{BooleanOp, Mesh, Vec3},
    model::{Body, Hole, Project, Shape},
    part::{self, EndCondition, Extrude, FeatureKind, HoleWizard, Part},
    psketch::{Dim, DimKind, Entity, ProfileSketch, Rel, SketchPlane},
    stl,
};
use std::{fs, path::PathBuf};
use viewport::{Style, Viewport};
const GOLD: Color32 = Color32::from_rgb(196, 215, 237);
const MUTED: Color32 = Color32::from_rgb(91, 103, 118);
const VERSION: &str = "0.3.0";
enum Dialog {
    None,
    New(String),
    Rename(String),
    Hole {
        body: u64,
        index: Option<usize>,
        hole: Hole,
    },
    Boolean {
        operation: BooleanOp,
        source: u64,
        target: u64,
    },
    Sketch {
        body: Option<u64>,
        points: Vec<[f64; 2]>,
        height: f64,
    },
    Export {
        all: bool,
    },
    Help,
}
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct RecentProject {
    path: PathBuf,
    name: String,
}
struct SolidCraft {
    home: bool,
    recent: Vec<RecentProject>,
    project: Project,
    meshes: Vec<Mesh>,
    selected: Option<u64>,
    draft: Option<Body>,
    undo: Vec<Project>,
    redo: Vec<Project>,
    view: Viewport,
    editor: PartEditor,
    dialog: Dialog,
    status: String,
    error: Option<String>,
    path: Option<PathBuf>,
    fit_next: bool,
    show_properties: bool,
    show_mass: bool,
    ribbon_tab: u8,
    workbench: workbench::Workbench,
    explode_amount: f64,
    explode_play: bool,
    explode_direction: f64,
    displayed: Vec<Mesh>,
    displayed_amount: f64,
    version: u64,
    reset_view: bool,
}
/// Parametric demo: plate sketched on the top plane, extruded, with counterbored holes and fillets.
fn demo_part() -> Project {
    let mut part = Part::default();
    let mut s = ProfileSketch::on(SketchPlane::top());
    let corners =
        [[-40.0, -25.0], [40.0, -25.0], [40.0, 25.0], [-40.0, 25.0]].map(|p| s.add_point(p));
    let mut lines = [0; 4];
    for i in 0..4 {
        lines[i] = s.add(Entity::Line {
            a: corners[i],
            b: corners[(i + 1) % 4],
            construction: false,
        });
    }
    let d1 = s.add(Entity::Line {
        a: corners[0],
        b: corners[2],
        construction: true,
    });
    let d2 = s.add(Entity::Line {
        a: corners[1],
        b: corners[3],
        construction: true,
    });
    s.rels.extend([
        Rel::Horizontal(lines[0]),
        Rel::Vertical(lines[1]),
        Rel::Horizontal(lines[2]),
        Rel::Vertical(lines[3]),
        Rel::Midpoint(0, d1),
        Rel::Midpoint(0, d2),
    ]);
    s.dims.push(Dim {
        kind: DimKind::Length(lines[0]),
        value: 80.0,
        offset: [0.0, -10.0],
        flip: false,
    });
    s.dims.push(Dim {
        kind: DimKind::Length(lines[1]),
        value: 50.0,
        offset: [10.0, 0.0],
        flip: false,
    });
    let sketch = part.push(FeatureKind::Sketch(s));
    let boss = part.push(FeatureKind::Extrude(Extrude {
        sketch,
        end: EndCondition::Blind,
        depth: 8.0,
        reverse: false,
        cut: false,
    }));
    let mut plane = SketchPlane::top().offset(8.0);
    plane.face = Some(part::tag(boss, 1));
    let m = part::METRIC[4];
    part.push(FeatureKind::Hole(HoleWizard {
        plane,
        points: vec![[-30.0, -15.0], [30.0, -15.0], [30.0, 15.0], [-30.0, 15.0]],
        kind: solidcraft::features::HoleKind::Counterbore,
        diameter: m.1,
        depth: 8.0,
        through: true,
        head_diameter: m.2,
        head_depth: m.3,
        standard: format!("{} – lamé CHC", m.0),
    }));
    let mesh = part.regenerate().mesh;
    let topo = solidcraft::topology::analyse(&mesh);
    let edges = topo
        .edges
        .iter()
        .filter(|e| {
            e.line
                .is_some_and(|[a, b]| (b - a).normalized().z.abs() > 0.99)
                && e.faces.iter().all(|f| part::feature_of(*f) == boss)
        })
        .map(|e| part::EdgeRef {
            faces: e.faces,
            point: e.midpoint().array(),
        })
        .collect();
    part.push(FeatureKind::Finish(part::Finish {
        kind: FinishKind::Fillet,
        size: 6.0,
        edges,
    }));
    let mut p = Project::new_part("Platine de fixation");
    if let Shape::Part(b) = &mut p.bodies[0].shape {
        **b = part;
    }
    p.version = 3;
    p
}
impl SolidCraft {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let mut style = (*cc.egui_ctx.style()).clone();
        style.visuals = egui::Visuals::light();
        style.visuals.panel_fill = Color32::from_rgb(242, 243, 245);
        style.visuals.window_fill = Color32::from_rgb(250, 251, 252);
        style.visuals.extreme_bg_color = Color32::WHITE;
        style.visuals.faint_bg_color = Color32::from_rgb(233, 236, 240);
        style.visuals.selection.bg_fill = Color32::from_rgb(200, 222, 245);
        style.visuals.selection.stroke = egui::Stroke::new(1.0, Color32::from_rgb(48, 99, 155));
        style.visuals.widgets.inactive.bg_fill = Color32::from_rgb(239, 241, 244);
        style.visuals.widgets.inactive.weak_bg_fill = Color32::from_rgb(239, 241, 244);
        style.visuals.widgets.inactive.bg_stroke =
            egui::Stroke::new(1.0, Color32::from_rgb(193, 199, 207));
        style.visuals.widgets.hovered.bg_fill = Color32::from_rgb(219, 233, 249);
        style.spacing.item_spacing = Vec2::new(6.0, 5.0);
        style.spacing.button_padding = Vec2::new(8.0, 4.0);
        cc.egui_ctx.set_style(style);
        let restored = cc
            .storage
            .and_then(|s| s.get_string("solidcraft-project"))
            .and_then(|s| Project::from_json(&s).ok());
        let cli_project = std::env::args()
            .position(|a| a == "--project")
            .and_then(|i| std::env::args().nth(i + 1))
            .and_then(|path| fs::read_to_string(path).ok())
            .and_then(|s| Project::from_json(&s).ok());
        let mut project = cli_project.or(restored).unwrap_or_else(demo_part);
        let meshes = project.meshes().unwrap_or_else(|_| {
            project = demo_part();
            project.meshes().expect("valid built-in example")
        });
        let recent = cc
            .storage
            .and_then(|s| s.get_string("solidcraft-recent"))
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default();
        let mut app = Self {
            home: std::env::args().len() == 1,
            recent,
            project,
            meshes,
            selected: None,
            draft: None,
            undo: Vec::new(),
            redo: Vec::new(),
            view: Viewport::default(),
            editor: PartEditor::default(),
            dialog: Dialog::None,
            status: "Prêt".into(),
            error: None,
            path: None,
            fit_next: true,
            show_properties: false,
            show_mass: false,
            ribbon_tab: 0,
            workbench: Default::default(),
            explode_amount: 0.0,
            explode_play: false,
            explode_direction: 1.0,
            displayed: vec![],
            displayed_amount: -1.0,
            version: 1,
            reset_view: false,
        };
        let args: Vec<String> = std::env::args().collect();
        if let Some(i) = args.iter().position(|a| a == "--show-part") {
            if let Some(kind) = args.get(i + 1) {
                app.workbench.open_part(kind, &app.project, app.selected);
            }
        }
        if args.iter().any(|a| a == "--show-assembly") {
            app.workbench.open_assembly(&app.project);
        }
        if args.iter().any(|a| a == "--show-drawing") {
            app.workbench.open_drawing(&app.project);
            if let workbench::Panel::Drawing(editor) = &mut app.workbench.panel {
                match solidcraft::drawing::generate(&app.project, &app.meshes) {
                    Ok(sheet) => editor.sheet = Some(sheet),
                    Err(e) => app.workbench.message = e,
                }
            }
        }
        app
    }
    fn remember(&mut self, path: &std::path::Path) {
        self.recent.retain(|r| {
            !r.path
                .to_string_lossy()
                .eq_ignore_ascii_case(&path.to_string_lossy())
        });
        self.recent.insert(
            0,
            RecentProject {
                path: path.to_path_buf(),
                name: self.project.name.clone(),
            },
        );
        self.recent.truncate(12);
    }
    fn open_new(&mut self, project: Project) {
        self.editor = PartEditor::default();
        if self.commit(project) {
            self.home = false;
            self.path = None;
            self.fit_next = true;
            self.reset_view = true;
        }
    }
    fn home_ui(&mut self, ctx: &egui::Context) {
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.add_space(48.0);
                ui.horizontal(|ui| {
                    ui.add_space(48.0);
                    ui.vertical(|ui| {
                        ui.label(RichText::new("SolidCraft").size(42.0).strong().color(Color32::from_rgb(155, 48, 43)));
                        ui.label(RichText::new("CAO paramétrique : esquisse, fonctions, arbre de construction").size(20.0).color(MUTED));
                        ui.add_space(28.0);
                        ui.horizontal(|ui| {
                            if ui.add_sized([180.0, 48.0], egui::Button::new(RichText::new("Nouvelle pièce").strong())).clicked() {
                                self.open_new(Project::new_part("Pièce1"));
                            }
                            if ui.add_sized([180.0, 48.0], egui::Button::new("Ouvrir un projet…")).clicked() {
                                self.open_project();
                            }
                            if ui.add_sized([220.0, 48.0], egui::Button::new("Reprendre la dernière session")).clicked() {
                                self.home = false;
                                self.fit_next = true;
                            }
                        });
                        ui.add_space(12.0);
                        ui.label(format!("Session : {}", self.project.name));
                        ui.add_space(24.0);
                        ui.heading("Comment ça marche");
                        for t in [
                            "1. Choisissez un plan (ou une face) puis Esquisse.",
                            "2. Dessinez (ligne, rectangle, cercle…), cotez avec Cotation intelligente (D) : l’esquisse devient noire quand elle est totalement contrainte.",
                            "3. Bossage extrudé, Enlèvement, Révolution, Congé, Perçage… : réglez dans le panneau de gauche, validez avec ✔.",
                            "4. Tout reste modifiable : double-cliquez une face ou une fonction de l’arbre.",
                        ] {
                            ui.label(t);
                        }
                        ui.add_space(24.0);
                        ui.heading("Récemment ouverts");
                        ui.add_space(8.0);
                        let mut open = None;
                        let mut remove = None;
                        if self.recent.is_empty() {
                            ui.label("Les projets ouverts ou enregistrés apparaîtront ici.");
                        }
                        for (i, r) in self.recent.iter().enumerate() {
                            ui.push_id(i, |ui| {
                                ui.horizontal(|ui| {
                                    if ui.add_sized([260.0, 32.0], egui::Button::new(&r.name)).clicked() {
                                        open = Some(r.path.clone());
                                    }
                                    ui.label(RichText::new(r.path.display().to_string()).size(11.0).color(MUTED));
                                    if !r.path.is_file() {
                                        ui.label("Introuvable");
                                    }
                                    if ui.small_button("Retirer").clicked() {
                                        remove = Some(i);
                                    }
                                });
                            });
                        }
                        if let Some(i) = remove {
                            self.recent.remove(i);
                        }
                        if let Some(path) = open {
                            self.load_path(path);
                        }
                        ui.add_space(24.0);
                        ui.heading("Exemples");
                        ui.horizontal(|ui| {
                            if ui.button("Platine paramétrique").clicked() {
                                self.open_new(demo_part());
                            }
                            for (name, file) in [("Assemblage (v0.2)", "02-Palier-assemblage"), ("Coque (v0.2)", "03-Coque"), ("Balayage (v0.2)", "04-Balayage"), ("Lissage (v0.2)", "05-Lissage")] {
                                if ui.button(name).clicked() {
                                    let base = std::env::current_exe().ok().and_then(|p| p.parent().map(|p| p.to_path_buf())).unwrap_or_default();
                                    let path = base.join("Exemples").join(format!("{file}.solidcraft"));
                                    let path = if path.exists() { path } else { PathBuf::from("Exemples").join(format!("{file}.solidcraft")) };
                                    self.load_path(path);
                                }
                            }
                        });
                        if let Some(e) = &self.error {
                            ui.colored_label(Color32::DARK_RED, e);
                        }
                    });
                });
            });
        });
    }
    fn select(&mut self, id: Option<u64>) {
        self.selected = id;
        self.draft = self
            .project
            .bodies
            .iter()
            .find(|b| Some(b.id) == id)
            .cloned();
        self.displayed.clear();
    }
    fn commit(&mut self, mut next: Project) -> bool {
        next.version = 3;
        let ids: Vec<_> = next.bodies.iter().map(|b| b.id).collect();
        next.assembly
            .mates
            .retain(|m| ids.contains(&m.a) && ids.contains(&m.b));
        next.assembly.fixed.retain(|id| ids.contains(id));
        next.assembly.explode.retain(|e| ids.contains(&e.body));
        if !next.assembly.mates.is_empty() && solidcraft::assembly::max_residual(&next) > 1e-4 {
            if let Err(e) = solidcraft::assembly::solve_mates(&mut next) {
                self.error = Some(e);
                return false;
            }
        }
        match next.meshes() {
            Ok(meshes) => {
                if next == self.project {
                    return true;
                }
                self.undo.push(self.project.clone());
                if self.undo.len() > 50 {
                    self.undo.remove(0);
                }
                self.redo.clear();
                self.project = next;
                self.meshes = meshes;
                let selection = self
                    .selected
                    .filter(|id| self.project.bodies.iter().any(|b| b.id == *id));
                self.select(selection);
                true
            }
            Err(e) => {
                self.error = Some(e);
                false
            }
        }
    }
    fn commit_part(&mut self, part: Part, label: &str) {
        let mut next = self.project.clone();
        match self.editor.body {
            Some(i) if i < next.bodies.len() => next.bodies[i].shape = Shape::Part(Box::new(part)),
            _ => {
                let id = next.next_id();
                let mut b = Body::new(id, Shape::Part(Box::new(part)));
                b.name = "Pièce1".into();
                next.bodies.insert(0, b);
                self.editor.body = Some(0);
            }
        }
        if self.commit(next) {
            self.status = format!("{label} : appliqué");
        }
    }
    fn history(&mut self, redo: bool) {
        if self.editor.active() {
            self.editor.cancel();
        }
        let next = if redo {
            self.redo.last()
        } else {
            self.undo.last()
        }
        .cloned();
        if let Some(next) = next {
            match next.meshes() {
                Ok(meshes) => {
                    if redo {
                        self.redo.pop();
                        self.undo.push(self.project.clone());
                    } else {
                        self.undo.pop();
                        self.redo.push(self.project.clone());
                    }
                    self.project = next;
                    self.meshes = meshes;
                    self.select(None);
                    self.editor.sel.clear();
                    self.status = if redo {
                        "Modification rétablie"
                    } else {
                        "Modification annulée"
                    }
                    .into();
                }
                Err(e) => self.error = Some(e),
            }
        }
    }
    fn add(&mut self, shape: Shape) {
        let mut next = self.project.clone();
        let id = next.next_id();
        next.bodies.push(Body::new(id, shape));
        if self.commit(next) {
            self.select(Some(id));
            self.fit_next = true;
        }
    }
    fn save_project(&mut self) {
        let path = rfd::FileDialog::new()
            .add_filter("Projet SolidCraft", &["solidcraft"])
            .set_file_name(format!("{}.solidcraft", safe_name(&self.project.name)))
            .save_file();
        if let Some(path) = path {
            let result = serde_json::to_string_pretty(&self.project)
                .map_err(|e| e.to_string())
                .and_then(|s| fs::write(&path, s).map_err(|e| e.to_string()));
            match result {
                Ok(()) => {
                    self.status = format!("Projet enregistré : {}", path.display());
                    self.remember(&path);
                    self.path = Some(path);
                }
                Err(e) => self.error = Some(format!("Enregistrement impossible : {e}")),
            }
        }
    }
    fn open_project(&mut self) {
        let path = rfd::FileDialog::new()
            .add_filter("Projet SolidCraft", &["solidcraft", "json"])
            .pick_file();
        if let Some(path) = path {
            self.load_path(path);
        }
    }
    fn load_path(&mut self, path: PathBuf) {
        let result = fs::metadata(&path)
            .map_err(|e| e.to_string())
            .and_then(|m| {
                if m.len() > 2_000_000 {
                    Err("Le fichier dépasse 2 Mo.".into())
                } else {
                    fs::read_to_string(&path).map_err(|e| e.to_string())
                }
            })
            .and_then(|s| Project::from_json(&s));
        match result {
            Ok(p) => {
                self.editor = PartEditor::default();
                if self.commit(p) {
                    self.remember(&path);
                    self.path = Some(path);
                    self.fit_next = true;
                    self.home = false;
                    self.status = "Projet ouvert. Annuler restaure le projet précédent.".into();
                }
            }
            Err(e) => self.error = Some(e),
        }
    }
    fn export(&mut self, all: bool) {
        let meshes: Vec<&Mesh> = self
            .project
            .bodies
            .iter()
            .zip(&self.meshes)
            .filter(|(b, m)| {
                !m.polygons.is_empty()
                    && if all {
                        b.visible
                    } else {
                        Some(b.id) == self.export_target()
                    }
            })
            .map(|(_, m)| m)
            .collect();
        if meshes.is_empty() {
            self.error = Some("Rien à exporter : la pièce est vide.".into());
            return;
        }
        let path = rfd::FileDialog::new()
            .add_filter("Maillage STL binaire", &["stl"])
            .set_file_name(format!("{}.stl", safe_name(&self.project.name)))
            .save_file();
        if let Some(path) = path {
            let result = fs::File::create(&path)
                .and_then(|f| stl::write_binary(std::io::BufWriter::new(f), &meshes));
            match result {
                Ok(()) => {
                    self.status =
                        format!("STL exporté : {} · importer en millimètres", path.display())
                }
                Err(e) => self.error = Some(format!("Export impossible : {e}")),
            }
        }
    }
    fn export_target(&self) -> Option<u64> {
        self.selected.or_else(|| {
            self.editor
                .body
                .and_then(|i| self.project.bodies.get(i))
                .map(|b| b.id)
        })
    }
    fn boolean_dialog(&mut self, operation: BooleanOp) {
        if let Some(source) = self.selected {
            if let Some(target) = self
                .project
                .bodies
                .iter()
                .find(|b| b.id != source)
                .map(|b| b.id)
            {
                self.dialog = Dialog::Boolean {
                    operation,
                    source,
                    target,
                };
            }
        }
    }
    /// Property panel of bodies created with the primitives of version 0.2.
    fn properties(&mut self, ui: &mut egui::Ui) {
        let Some(mut draft) = self.draft.clone() else {
            ui.label(RichText::new("Sélectionnez un corps dans « Autres corps ».").color(MUTED));
            return;
        };
        ui.horizontal(|ui| {
            ui.add(
                egui::TextEdit::singleline(&mut draft.name)
                    .desired_width(180.0)
                    .char_limit(100),
            );
            ui.color_edit_button_srgb(&mut draft.color);
        });
        ui.add_space(8.0);
        match &mut draft.shape {
            Shape::Part(_) => {
                ui.label("Pièce paramétrique : modifiez-la avec l’arbre de construction.");
            }
            Shape::Feature(f) => {
                ui.label(f.label());
                if ui.button("Modifier la fonction...").clicked() {
                    self.workbench
                        .open_part("edit", &self.project, Some(draft.id));
                }
            }
            Shape::Box {
                width,
                depth,
                height,
            } => {
                dimension(ui, "Largeur · X", width, " mm");
                dimension(ui, "Profondeur · Y", depth, " mm");
                dimension(ui, "Hauteur · Z", height, " mm");
            }
            Shape::Cylinder { radius, height } => {
                dimension(ui, "Rayon", radius, " mm");
                dimension(ui, "Hauteur", height, " mm");
            }
            Shape::Tube {
                radius,
                inner_radius,
                height,
            } => {
                dimension(ui, "Rayon extérieur", radius, " mm");
                dimension(ui, "Rayon intérieur", inner_radius, " mm");
                dimension(ui, "Hauteur", height, " mm");
            }
            Shape::Extrusion { points, height } => {
                dimension(ui, "Extrusion", height, " mm");
                if ui.button("Modifier le contour").clicked() {
                    self.dialog = Dialog::Sketch {
                        body: Some(draft.id),
                        points: points.clone(),
                        height: *height,
                    };
                }
            }
            Shape::Boolean { operation, .. } => {
                ui.label(RichText::new(format!("Résultat : {}", operation.label())).color(MUTED));
            }
        }
        ui.separator();
        ui.label(RichText::new("Position").strong());
        vector_fields(ui, &mut draft.position, "mm");
        ui.label(RichText::new("Rotation").strong());
        vector_fields(ui, &mut draft.rotation, "°");
        let changed = self.project.bodies.iter().find(|b| b.id == draft.id) != Some(&draft);
        if ui
            .add_enabled(
                changed,
                egui::Button::new(RichText::new("Appliquer les paramètres").color(Color32::BLACK))
                    .fill(GOLD)
                    .min_size(Vec2::new(ui.available_width(), 30.0)),
            )
            .clicked()
        {
            let mut next = self.project.clone();
            if let Some(b) = next.bodies.iter_mut().find(|b| b.id == draft.id) {
                *b = draft.clone();
            }
            self.commit(next);
        }
        if !matches!(draft.shape, Shape::Part(_)) {
            ui.separator();
            ui.label(
                RichText::new(format!("Perçages traversants · {}", draft.holes.len())).strong(),
            );
            let mut remove = None;
            for (i, hole) in draft.holes.iter().enumerate() {
                ui.horizontal(|ui| {
                    if ui
                        .button(format!(
                            "Ø {:.2}   X {:.1}  Y {:.1}",
                            hole.diameter, hole.x, hole.y
                        ))
                        .clicked()
                    {
                        self.dialog = Dialog::Hole {
                            body: draft.id,
                            index: Some(i),
                            hole: hole.clone(),
                        };
                    }
                    if ui.small_button("×").clicked() {
                        remove = Some(i);
                    }
                });
            }
            if let Some(i) = remove {
                let mut next = self.project.clone();
                if let Some(b) = next.bodies.iter_mut().find(|b| b.id == draft.id) {
                    b.holes.remove(i);
                }
                self.commit(next);
            }
            if ui.button("+ Ajouter un perçage").clicked() {
                self.dialog = Dialog::Hole {
                    body: draft.id,
                    index: None,
                    hole: Hole::default(),
                };
            }
        }
        ui.separator();
        let mut deleted = false;
        ui.horizontal(|ui| {
            if ui.button("Dupliquer").clicked() {
                let mut next = self.project.clone();
                let mut copy = next
                    .bodies
                    .iter()
                    .find(|b| b.id == draft.id)
                    .unwrap()
                    .clone();
                let id = next.next_id();
                copy.id = id;
                copy.name.push_str(" — copie");
                copy.position[0] += 15.0;
                next.bodies.push(copy);
                if self.commit(next) {
                    self.select(Some(id));
                }
            }
            if ui
                .button(RichText::new("Supprimer").color(Color32::from_rgb(200, 60, 60)))
                .clicked()
            {
                let mut next = self.project.clone();
                next.bodies.retain(|b| b.id != draft.id);
                deleted = self.commit(next);
            }
        });
        if !deleted {
            self.draft = Some(draft);
        } else {
            self.select(None);
        }
    }
    fn dialogs(&mut self, ctx: &egui::Context) {
        let mut dialog = std::mem::replace(&mut self.dialog, Dialog::None);
        let mut close = false;
        let title = match &dialog {
            Dialog::None => return,
            Dialog::New(_) => "Nouveau projet",
            Dialog::Rename(_) => "Renommer le projet",
            Dialog::Hole { .. } => "Perçage traversant",
            Dialog::Boolean { operation, .. } => operation.label(),
            Dialog::Sketch { .. } => "Contour simple sur le plan XY",
            Dialog::Export { .. } => "Exporter pour l’impression 3D",
            Dialog::Help => "Aide SolidCraft",
        };
        egui::Window::new(title).collapsible(false).resizable(false).anchor(egui::Align2::CENTER_CENTER, Vec2::ZERO).default_width(520.0).show(ctx, |ui| {
            ui.add_space(6.0);
            match &mut dialog {
                Dialog::New(name) | Dialog::Rename(name) => {
                    ui.label("Nom");
                    ui.add(egui::TextEdit::singleline(name).desired_width(400.0).char_limit(100));
                }
                Dialog::Hole { hole, .. } => {
                    ui.label("Trou traversant suivant l’axe Z local.");
                    dimension(ui, "Diamètre", &mut hole.diameter, " mm");
                    dimension(ui, "Centre X", &mut hole.x, " mm");
                    dimension(ui, "Centre Y", &mut hole.y, " mm");
                }
                Dialog::Boolean { source, target, operation } => {
                    let name = self.project.bodies.iter().find(|b| b.id == *source).map(|b| b.name.as_str()).unwrap_or("Solide");
                    ui.label(format!("Solide de départ : {name}"));
                    ui.label(if *operation == BooleanOp::Subtract { "Le second solide est retiré du premier." } else { "Les deux sources sont remplacées par le résultat." });
                    egui::ComboBox::from_id_salt("boolean-target").selected_text(self.project.bodies.iter().find(|b| b.id == *target).map(|b| b.name.as_str()).unwrap_or("Choisir")).show_ui(ui, |ui| {
                        for b in &self.project.bodies {
                            if b.id != *source {
                                ui.selectable_value(target, b.id, &b.name);
                            }
                        }
                    });
                }
                Dialog::Sketch { points, height, .. } => {
                    ui.label("Cliquez pour placer les sommets. Le contour se ferme automatiquement.");
                    let (rect, response) = ui.allocate_exact_size(Vec2::new(520.0, 300.0), egui::Sense::click());
                    let painter = ui.painter_at(rect);
                    painter.rect_filled(rect, 4.0, Color32::from_rgb(18, 25, 35));
                    let origin = rect.center();
                    let scale = 4.0;
                    for i in -13..=13 {
                        let x = origin.x + i as f32 * 20.0;
                        painter.line_segment([egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())], egui::Stroke::new(1.0, Color32::from_gray(46)));
                    }
                    for i in -7..=7 {
                        let y = origin.y + i as f32 * 20.0;
                        painter.line_segment([egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)], egui::Stroke::new(1.0, Color32::from_gray(46)));
                    }
                    let screen = |p: [f64; 2]| origin + Vec2::new(p[0] as f32 * scale, -p[1] as f32 * scale);
                    for i in 0..points.len() {
                        let a = screen(points[i]);
                        painter.circle_filled(a, 4.0, GOLD);
                        if points.len() > 1 {
                            painter.line_segment([a, screen(points[(i + 1) % points.len()])], egui::Stroke::new(2.0, GOLD));
                        }
                    }
                    if response.clicked() {
                        if let Some(p) = response.interact_pointer_pos() {
                            if points.len() < 100 {
                                points.push([((p.x - origin.x) / scale / 5.0).round() as f64 * 5.0, ((origin.y - p.y) / scale / 5.0).round() as f64 * 5.0]);
                            }
                        }
                    }
                    ui.horizontal(|ui| {
                        if ui.button("Retirer un point").clicked() {
                            points.pop();
                        }
                        if ui.button("Effacer").clicked() {
                            points.clear();
                        }
                    });
                    dimension(ui, "Hauteur d’extrusion", height, " mm");
                }
                Dialog::Export { all } => {
                    ui.label("STL binaire · coordonnées en millimètres");
                    ui.radio_value(all, false, "Pièce ou corps sélectionné");
                    ui.radio_value(all, true, "Tous les corps visibles");
                    ui.label("Le STL ne conserve pas l’historique. Enregistrez aussi le projet .solidcraft.");
                }
                Dialog::Help => {
                    ui.heading("Démarche (comme SolidWorks / Creo)");
                    for t in [
                        "1. Sélectionnez un plan (Face, Dessus, Droite) ou une face plane, puis Esquisse.",
                        "2. Dessinez : Ligne (L), Rectangle (R), Cercle (C), Arc (A), Rainure. Les relations horizontale/verticale sont déduites automatiquement.",
                        "3. Cotation intelligente (D) : cliquez une entité, placez la cote, tapez la valeur (calculs acceptés : 40/2+3).",
                        "   Bleu = encore libre, noir = totalement contraint.",
                        "4. Bossage extrudé / Enlèvement / Révolution : la prévisualisation est jaune, glissez la flèche pour la profondeur, ✔ pour valider.",
                        "5. Congé, chanfrein : cliquez les arêtes. Perçage : cliquez la face puis les positions.",
                        "6. Tout est modifiable : double-cliquez une face ou une fonction de l’arbre ; clic droit pour éditer l’esquisse, renommer, désactiver, supprimer.",
                    ] {
                        ui.label(t);
                    }
                    ui.separator();
                    ui.heading("Souris et clavier");
                    for t in [
                        "Bouton du milieu : rotation · Ctrl+milieu ou clic droit glissé : déplacement · Molette : zoom vers le curseur",
                        "Glisser avec le bouton gauche dans le vide : rotation · Flèches : rotation de 15°",
                        "F : zoom au cadre · Ctrl+1 à Ctrl+7 : vues standard · Ctrl+8 : normal à · Barre d’espace : orientation",
                        "Entrée : valider · Échap : annuler / terminer l’outil · Suppr : supprimer",
                        "Ctrl+Z / Ctrl+Y : annuler / rétablir · Ctrl+S : enregistrer · Ctrl+B : reconstruire",
                    ] {
                        ui.label(t);
                    }
                }
                Dialog::None => {}
            }
            ui.add_space(12.0);
            ui.separator();
            ui.horizontal(|ui| {
                if ui.button("Fermer").clicked() {
                    close = true;
                }
                if matches!(dialog, Dialog::Help) {
                    return;
                }
                if ui
                    .add(egui::Button::new(RichText::new(match dialog { Dialog::Sketch { .. } => "Extruder", Dialog::Export { .. } => "Exporter le STL", Dialog::New(_) => "Créer", _ => "Appliquer" }).color(Color32::BLACK)).fill(GOLD))
                    .clicked()
                {
                    close = match &dialog {
                        Dialog::New(name) => {
                            self.open_new(Project::new_part(&nonempty(name)));
                            true
                        }
                        Dialog::Rename(name) => {
                            let mut p = self.project.clone();
                            p.name = nonempty(name);
                            self.commit(p)
                        }
                        Dialog::Hole { body, index, hole } => {
                            let mut p = self.project.clone();
                            if let Some(b) = p.bodies.iter_mut().find(|b| b.id == *body) {
                                if let Some(i) = index {
                                    b.holes[*i] = hole.clone();
                                } else {
                                    b.holes.push(hole.clone());
                                }
                            }
                            self.commit(p)
                        }
                        Dialog::Boolean { source, target, operation } => {
                            let mut p = self.project.clone();
                            let a = p.bodies.iter().find(|b| b.id == *source).cloned();
                            let b = p.bodies.iter().find(|b| b.id == *target).cloned();
                            if let (Some(a), Some(b)) = (a, b) {
                                let id = p.next_id();
                                p.bodies.retain(|b| b.id != *source && b.id != *target);
                                p.bodies.push(Body::new(id, Shape::Boolean { operation: *operation, a: Box::new(a), b: Box::new(b) }));
                                let ok = self.commit(p);
                                if ok {
                                    self.select(Some(id));
                                }
                                ok
                            } else {
                                false
                            }
                        }
                        Dialog::Sketch { body, points, height } => {
                            let mut p = self.project.clone();
                            let shape = Shape::Extrusion { points: points.clone(), height: *height };
                            let id = body.unwrap_or(p.next_id());
                            if let Some(b) = p.bodies.iter_mut().find(|b| b.id == id) {
                                b.shape = shape;
                            } else {
                                p.bodies.push(Body::new(id, shape));
                            }
                            let ok = self.commit(p);
                            if ok {
                                self.select(Some(id));
                                self.fit_next = true;
                            }
                            ok
                        }
                        Dialog::Export { all } => {
                            self.export(*all);
                            true
                        }
                        _ => true,
                    };
                }
            });
        });
        if !close {
            self.dialog = dialog;
        }
    }
    fn shortcuts(&mut self, ctx: &egui::Context) {
        if !matches!(self.dialog, Dialog::None)
            || !matches!(self.workbench.panel, workbench::Panel::None)
            || ctx.wants_keyboard_input()
        {
            return;
        }
        let bodies = self.project.bodies.clone();
        let cmd = egui::Modifiers::COMMAND;
        let pressed = |k: egui::Key, m: egui::Modifiers| ctx.input_mut(|i| i.consume_key(m, k));
        if pressed(egui::Key::S, cmd) {
            self.save_project();
        }
        if pressed(egui::Key::Z, cmd | egui::Modifiers::SHIFT) || pressed(egui::Key::Y, cmd) {
            self.history(true);
        } else if pressed(egui::Key::Z, cmd) {
            self.history(false);
        }
        if pressed(egui::Key::B, cmd) {
            self.version += 1;
            self.status = "Reconstruction effectuée".into();
        }
        // Ctrl+digit by physical key, so that AZERTY keyboards work without Shift.
        let digit = ctx.input(|i| {
            i.events.iter().find_map(|e| match e {
                egui::Event::Key {
                    key,
                    physical_key,
                    pressed: true,
                    modifiers,
                    ..
                } if modifiers.command => {
                    let k = physical_key.unwrap_or(*key);
                    [
                        egui::Key::Num1,
                        egui::Key::Num2,
                        egui::Key::Num3,
                        egui::Key::Num4,
                        egui::Key::Num5,
                        egui::Key::Num6,
                        egui::Key::Num7,
                        egui::Key::Num8,
                    ]
                    .iter()
                    .position(|d| *d == k)
                }
                _ => None,
            })
        });
        match digit {
            Some(7) => self.editor.normal_to(&bodies, &mut self.view, ctx),
            Some(i) => {
                self.view.standard(i, ctx);
                self.fit_next = true;
            }
            None => {}
        }
        let sketch_busy = matches!(&self.editor.mode, Mode::Sketch(e, _) if e.dim_edit.is_some());
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && !sketch_busy {
            match &mut self.editor.mode {
                Mode::Sketch(edit, _) => edit.escape(),
                Mode::Idle => {
                    self.editor.sel.clear();
                    self.select(None);
                }
                _ => self.editor.cancel(),
            }
            self.editor.space_menu = false;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) && !sketch_busy {
            match &self.editor.mode {
                Mode::Sketch(edit, _) if edit.busy() => {}
                Mode::Idle => {}
                _ => self.editor.ok(&bodies, &mut self.view, ctx),
            }
        }
        if !self.editor.sketching() {
            if ctx.input(|i| i.key_pressed(egui::Key::F) && i.modifiers.is_none()) {
                self.fit_next = true;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
                self.editor.space_menu = !self.editor.space_menu;
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Delete))
                && matches!(self.editor.mode, Mode::Idle)
            {
                if self.editor.sel.iter().any(|s| matches!(s, Sel::Feature(_))) {
                    self.editor.delete_selected(&bodies);
                } else if let Some(id) = self.selected {
                    let mut next = self.project.clone();
                    next.bodies.retain(|b| b.id != id);
                    self.commit(next);
                }
            }
        } else if ctx.input(|i| i.key_pressed(egui::Key::F) && i.modifiers.is_none()) {
            self.fit_next = true;
        }
    }
    fn ribbon(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let bodies = self.project.bodies.clone();
        let has_part = self.editor.body.is_some();
        let sketching = self.editor.sketching();
        let has_geometry = !self.editor.mesh().polygons.is_empty();
        egui::ScrollArea::horizontal().show(ui, |ui| {
            ui.horizontal(|ui| match self.ribbon_tab {
                0 => {
                    let sketch_ready = sketching || has_part;
                    let run = |cmd: Cmd, this: &mut Self| this.editor.run(cmd, &bodies, &mut this.view, ctx);
                    if ribbon_button(ui, "Bossage extrudé", Glyph::Extrude, sketch_ready) {
                        run(Cmd::Extrude { cut: false }, self);
                    }
                    if ribbon_button(ui, "Bossage avec révolution", Glyph::Revolve, sketch_ready) {
                        run(Cmd::Revolve { cut: false }, self);
                    }
                    if ribbon_button(ui, "Enlèv. de matière extrudé", Glyph::Cut, sketch_ready && (has_geometry || sketching)) {
                        run(Cmd::Extrude { cut: true }, self);
                    }
                    if ribbon_button(ui, "Enlèv. avec révolution", Glyph::Revolve, sketch_ready && (has_geometry || sketching)) {
                        run(Cmd::Revolve { cut: true }, self);
                    }
                    ui.separator();
                    if ribbon_button(ui, "Congé", Glyph::Fillet, has_geometry) {
                        run(Cmd::Finish(FinishKind::Fillet), self);
                    }
                    if ribbon_button(ui, "Chanfrein", Glyph::Chamfer, has_geometry) {
                        run(Cmd::Finish(FinishKind::Chamfer), self);
                    }
                    if ribbon_button(ui, "Assistance pour le perçage", Glyph::Hole, has_geometry) {
                        run(Cmd::Hole, self);
                    }
                    ui.separator();
                    if ribbon_button(ui, "Répétition linéaire", Glyph::Pattern, has_geometry) {
                        run(Cmd::LinearPattern, self);
                    }
                    if ribbon_button(ui, "Répétition circulaire", Glyph::Pattern, has_geometry) {
                        run(Cmd::CircularPattern, self);
                    }
                    if ribbon_button(ui, "Symétrie", Glyph::Mirror, has_geometry) {
                        run(Cmd::Mirror, self);
                    }
                    ui.separator();
                    if ribbon_button(ui, "Plan", Glyph::Plane, has_part) {
                        run(Cmd::Plane, self);
                    }
                    if ribbon_button(ui, "Reconstruire (Ctrl+B)", Glyph::Rebuild, true) {
                        self.version += 1;
                    }
                }
                1 => {
                    if ribbon_button(ui, if sketching { "Quitter l’esquisse" } else { "Esquisse" }, Glyph::Sketch, has_part) {
                        self.editor.start_sketch(&bodies, &mut self.view, ctx, None);
                    }
                    if ribbon_button(ui, "Cotation intelligente", Glyph::Dimension, sketching) {
                        self.set_tool(Tool::Dimension);
                    }
                    ui.separator();
                    for (label, glyph, tool) in [
                        ("Ligne", Glyph::Line, Tool::Line),
                        ("Rectangle", Glyph::Rect, Tool::Rectangle),
                        ("Rectangle centré", Glyph::Rect, Tool::CenterRectangle),
                        ("Cercle", Glyph::Circle, Tool::Circle),
                        ("Arc centré", Glyph::Arc, Tool::Arc),
                        ("Rainure droite", Glyph::Slot, Tool::Slot),
                        ("Ligne de construction", Glyph::Centerline, Tool::Centerline),
                    ] {
                        if ribbon_button(ui, label, glyph, has_part) {
                            if !sketching {
                                self.editor.start_sketch(&bodies, &mut self.view, ctx, None);
                            }
                            self.set_tool(tool);
                        }
                    }
                    ui.separator();
                    if ribbon_button(ui, "Normal à (Ctrl+8)", Glyph::NormalTo, true) {
                        self.editor.normal_to(&bodies, &mut self.view, ctx);
                    }
                }
                2 => {
                    if ribbon_button(ui, "Mesurer", Glyph::Measure, has_geometry) {
                        self.editor.mode = Mode::Measure;
                    }
                    if ribbon_button(ui, "Propriétés de masse", Glyph::Mass, has_geometry) {
                        self.show_mass = !self.show_mass;
                    }
                    if ribbon_button(ui, "Exporter STL", Glyph::Export, !self.project.bodies.is_empty()) {
                        self.dialog = Dialog::Export { all: false };
                    }
                }
                3 => {
                    if cad_tool(ui, "Insérer", 0, true) {
                        match workbench::inserted_project(&self.project) {
                            Ok(Some(p)) => {
                                self.commit(p);
                                self.fit_next = true;
                            }
                            Ok(None) => {}
                            Err(e) => self.error = Some(e),
                        }
                    }
                    if cad_tool(ui, "Contraintes", 5, self.project.bodies.len() > 1) {
                        self.workbench.open_assembly(&self.project);
                    }
                    if cad_tool(ui, "Interférences", 7, self.project.bodies.len() > 1) {
                        self.workbench.open_assembly(&self.project);
                    }
                    if cad_tool(ui, "Préparer éclaté", 8, !self.project.bodies.is_empty()) {
                        let mut p = self.project.clone();
                        p.assembly.explode = solidcraft::assembly::auto_explode(&p, &self.meshes, 50.0);
                        self.commit(p);
                    }
                    ui.separator();
                    ui.vertical(|ui| {
                        ui.label("Vue éclatée (visuelle)");
                        ui.add_enabled(!self.project.assembly.explode.is_empty(), egui::Slider::new(&mut self.explode_amount, 0.0..=1.0));
                        ui.horizontal(|ui| {
                            ui.add_enabled_ui(!self.project.assembly.explode.is_empty(), |ui| {
                                ui.toggle_value(&mut self.explode_play, "Lecture");
                            });
                            if ui.button("Réassembler").clicked() {
                                self.explode_play = false;
                                self.explode_amount = 0.0;
                            }
                        });
                    });
                }
                4 => {
                    if cad_tool(ui, "Mise en plan", 9, !self.project.bodies.is_empty()) {
                        self.workbench.open_drawing(&self.project);
                    }
                    if cad_tool(ui, "Nomenclature", 8, !self.project.bodies.is_empty()) {
                        self.workbench.open_drawing(&self.project);
                    }
                    ui.label(RichText::new("Vues · coupes · détails · cotes et tolérances\nNomenclature · cartouche · export SVG / CSV").size(12.0).color(MUTED));
                }
                5 => {
                    for style in [Style::ShadedEdges, Style::Shaded, Style::HiddenLines, Style::Wireframe] {
                        if ui.add_sized([110.0, 60.0], egui::SelectableLabel::new(self.view.style == style, style.label())).clicked() {
                            self.view.style = style;
                            self.view.dirty = true;
                        }
                    }
                    ui.separator();
                    ui.vertical(|ui| {
                        ui.checkbox(&mut self.editor.show_planes, "Afficher les plans");
                        if ui.button("Zoom au cadre (F)").clicked() {
                            self.fit_next = true;
                        }
                    });
                    ui.separator();
                    ui.vertical(|ui| {
                        ui.horizontal(|ui| {
                            for (i, (name, _)) in viewport::VIEWS.iter().enumerate().take(4) {
                                if ui.button(*name).clicked() {
                                    self.view.standard(i, ctx);
                                    self.fit_next = true;
                                }
                            }
                        });
                        ui.horizontal(|ui| {
                            for (i, (name, _)) in viewport::VIEWS.iter().enumerate().skip(4) {
                                if ui.button(*name).clicked() {
                                    self.view.standard(i, ctx);
                                    self.fit_next = true;
                                }
                            }
                        });
                    });
                }
                _ => {
                    ui.label(RichText::new("Primitives de la version 0.2 (corps séparés)").size(11.0).color(MUTED));
                    if cad_tool(ui, "Bloc", 0, true) {
                        self.add(Shape::Box { width: 60.0, depth: 40.0, height: 12.0 });
                    }
                    if cad_tool(ui, "Cylindre", 1, true) {
                        self.add(Shape::Cylinder { radius: 15.0, height: 30.0 });
                    }
                    if cad_tool(ui, "Tube", 2, true) {
                        self.add(Shape::Tube { radius: 20.0, inner_radius: 15.0, height: 30.0 });
                    }
                    if cad_tool(ui, "Contour", 3, true) {
                        self.dialog = Dialog::Sketch { body: None, points: Vec::new(), height: 12.0 };
                    }
                    ui.separator();
                    for (i, op) in [BooleanOp::Union, BooleanOp::Subtract, BooleanOp::Intersect].into_iter().enumerate() {
                        if cad_tool(ui, op.label(), 5 + i as u8, self.selected.is_some() && self.project.bodies.len() > 1) {
                            self.boolean_dialog(op);
                        }
                    }
                    ui.separator();
                    for (label, kind, icon) in [("Révolution", "revolve", 1), ("Balayage", "sweep", 2), ("Lissage", "loft", 5), ("Coque", "shell", 2)] {
                        let needs = kind == "shell";
                        if cad_tool(ui, label, icon, !needs || self.selected.is_some()) {
                            self.workbench.open_part(kind, &self.project, self.selected);
                        }
                    }
                    if cad_tool(ui, "Propriétés", 8, self.selected.is_some()) {
                        self.show_properties = !self.show_properties;
                    }
                }
            });
        });
    }
    fn set_tool(&mut self, tool: Tool) {
        if let Mode::Sketch(edit, _) = &mut self.editor.mode {
            edit.set_tool(tool);
        }
    }
    fn heads_up(&mut self, ctx: &egui::Context) {
        let rect = self.view.rect;
        if !rect.is_positive() {
            return;
        }
        let bodies = self.project.bodies.clone();
        egui::Area::new(egui::Id::new("heads-up"))
            .fixed_pos(rect.center_top() + Vec2::new(-170.0, 8.0))
            .order(egui::Order::Middle)
            .show(ctx, |ui| {
                egui::Frame::popup(ui.style())
                    .inner_margin(egui::Margin::symmetric(6, 3))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if ui.button("⛶").on_hover_text("Zoom au cadre (F)").clicked() {
                                self.fit_next = true;
                            }
                            if ui
                                .button("Normal à")
                                .on_hover_text("Normal à (Ctrl+8)")
                                .clicked()
                            {
                                self.editor.normal_to(&bodies, &mut self.view, ctx);
                            }
                            ui.menu_button("Vues", |ui| {
                                for (i, (name, key)) in viewport::VIEWS.iter().enumerate() {
                                    if ui.button(format!("{name}    {key}")).clicked() {
                                        self.view.standard(i, ctx);
                                        self.fit_next = true;
                                        ui.close_menu();
                                    }
                                }
                            });
                            ui.menu_button("Affichage", |ui| {
                                for style in [
                                    Style::ShadedEdges,
                                    Style::Shaded,
                                    Style::HiddenLines,
                                    Style::Wireframe,
                                ] {
                                    if ui
                                        .selectable_label(self.view.style == style, style.label())
                                        .clicked()
                                    {
                                        self.view.style = style;
                                        self.view.dirty = true;
                                        ui.close_menu();
                                    }
                                }
                            });
                            ui.toggle_value(&mut self.editor.show_planes, "Plans");
                        });
                    });
            });
        // Confirmation corner.
        if self.editor.active() {
            egui::Area::new(egui::Id::new("confirmation-corner"))
                .fixed_pos(rect.right_top() + Vec2::new(-96.0, 10.0))
                .order(egui::Order::Middle)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        let sketch = self.editor.sketching();
                        if !matches!(self.editor.mode, Mode::PickPlane(_))
                            && ui
                                .add(
                                    egui::Button::new(
                                        RichText::new("✔")
                                            .size(22.0)
                                            .color(Color32::from_rgb(30, 150, 60)),
                                    )
                                    .min_size(Vec2::new(40.0, 36.0)),
                                )
                                .on_hover_text(if sketch { "Quitter l’esquisse" } else { "OK" })
                                .clicked()
                        {
                            self.editor.ok(&bodies, &mut self.view, ctx);
                        }
                        if ui
                            .add(
                                egui::Button::new(
                                    RichText::new("✖")
                                        .size(22.0)
                                        .color(Color32::from_rgb(200, 40, 40)),
                                )
                                .min_size(Vec2::new(40.0, 36.0)),
                            )
                            .on_hover_text(if sketch {
                                "Annuler les modifications de l’esquisse"
                            } else {
                                "Annuler"
                            })
                            .clicked()
                        {
                            if sketch {
                                self.editor.exit_sketch(&bodies, &mut self.view, ctx, false);
                            } else {
                                self.editor.cancel();
                            }
                        }
                    });
                });
        }
        if self.editor.space_menu {
            egui::Window::new("Orientation")
                .collapsible(false)
                .resizable(false)
                .fixed_pos(rect.center() - Vec2::new(90.0, 120.0))
                .show(ctx, |ui| {
                    for (i, (name, key)) in viewport::VIEWS.iter().enumerate() {
                        if ui
                            .add_sized([170.0, 24.0], egui::Button::new(format!("{name}   {key}")))
                            .clicked()
                        {
                            self.view.standard(i, ctx);
                            self.fit_next = true;
                            self.editor.space_menu = false;
                        }
                    }
                    if ui
                        .add_sized([170.0, 24.0], egui::Button::new("Normal à   Ctrl+8"))
                        .clicked()
                    {
                        self.editor.normal_to(&bodies, &mut self.view, ctx);
                        self.editor.space_menu = false;
                    }
                });
        }
        if self.show_mass {
            let mesh = self.editor.mesh().clone();
            let (mut open, mat) = (
                true,
                self.editor
                    .part(&bodies)
                    .map(|p| p.material.clone())
                    .unwrap_or_default(),
            );
            egui::Window::new("Propriétés de masse").open(&mut open).default_pos(rect.right_top() + Vec2::new(-340.0, 60.0)).show(ctx, |ui| {
                let (v, a, c) = part::mass_properties(&mesh);
                ui.label(format!("Matériau : {} ({} g/cm³)", mat.name, mat.density));
                ui.label(RichText::new(format!("Masse : {:.2} g", v / 1000.0 * mat.density)).strong());
                ui.label(format!("Volume : {:.2} mm³ ({:.3} cm³)", v, v / 1000.0));
                ui.label(format!("Aire de surface : {:.2} mm²", a));
                ui.label(format!("Centre de gravité : X {:.2}  Y {:.2}  Z {:.2} mm", c.x, c.y, c.z));
                if let Some((lo, hi)) = mesh.bounds() {
                    let s = hi - lo;
                    ui.label(format!("Encombrement : {:.2} × {:.2} × {:.2} mm", s.x, s.y, s.z));
                }
                ui.small("Pièce pleine (remplissage 100 %). La masse imprimée dépend du remplissage du trancheur.");
            });
            self.show_mass = open;
        }
    }
}
impl eframe::App for SolidCraft {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self.home {
            self.home_ui(ctx);
            return;
        }
        self.shortcuts(ctx);
        if std::mem::take(&mut self.reset_view) {
            self.view.standard(6, ctx);
            self.fit_next = true;
        }
        let bodies = self.project.bodies.clone();
        if self.editor.sync(&bodies) {
            self.displayed.clear();
        }
        let modal = !matches!(self.workbench.panel, workbench::Panel::None)
            || !matches!(self.dialog, Dialog::None);
        egui::TopBottomPanel::top("header")
            .exact_height(34.0)
            .show(ctx, |ui| {
                if modal {
                    ui.disable();
                }
                ui.horizontal_centered(|ui| {
                    ui.add_space(8.0);
                    ui.label(
                        RichText::new("SolidCraft")
                            .strong()
                            .italics()
                            .size(19.0)
                            .color(Color32::from_rgb(155, 48, 43)),
                    );
                    ui.separator();
                    ui.menu_button("Fichier", |ui| {
                        if ui.button("Nouvelle pièce…").clicked() {
                            self.dialog = Dialog::New("Pièce1".into());
                            ui.close_menu();
                        }
                        if ui.button("Ouvrir…  Ctrl+O").clicked() {
                            self.open_project();
                            ui.close_menu();
                        }
                        if ui.button("Enregistrer…  Ctrl+S").clicked() {
                            self.save_project();
                            ui.close_menu();
                        }
                        if ui.button("Exporter STL…").clicked() {
                            self.dialog = Dialog::Export { all: false };
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Accueil").clicked() {
                            self.home = true;
                            ui.close_menu();
                        }
                    });
                    ui.menu_button("Édition", |ui| {
                        if ui
                            .add_enabled(
                                !self.undo.is_empty(),
                                egui::Button::new("Annuler  Ctrl+Z"),
                            )
                            .clicked()
                        {
                            self.history(false);
                            ui.close_menu();
                        }
                        if ui
                            .add_enabled(
                                !self.redo.is_empty(),
                                egui::Button::new("Rétablir  Ctrl+Y"),
                            )
                            .clicked()
                        {
                            self.history(true);
                            ui.close_menu();
                        }
                        if ui.button("Renommer le projet").clicked() {
                            self.dialog = Dialog::Rename(self.project.name.clone());
                            ui.close_menu();
                        }
                    });
                    if ui.button("Aide").clicked() {
                        self.dialog = Dialog::Help;
                    }
                    ui.separator();
                    for (label, tip, f) in [
                        ("📄", "Nouvelle pièce", 0),
                        ("📂", "Ouvrir", 1),
                        ("💾", "Enregistrer (Ctrl+S)", 2),
                        ("Annuler", "Annuler (Ctrl+Z)", 3),
                        ("Rétablir", "Rétablir (Ctrl+Y)", 4),
                    ] {
                        let enabled = match f {
                            3 => !self.undo.is_empty(),
                            4 => !self.redo.is_empty(),
                            _ => true,
                        };
                        if ui
                            .add_enabled(enabled, egui::Button::new(label))
                            .on_hover_text(tip)
                            .clicked()
                        {
                            match f {
                                0 => self.dialog = Dialog::New("Pièce1".into()),
                                1 => self.open_project(),
                                2 => self.save_project(),
                                3 => self.history(false),
                                _ => self.history(true),
                            }
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("{}.solidcraft", self.project.name))
                                .size(11.0)
                                .color(MUTED),
                        );
                    });
                });
            });
        egui::TopBottomPanel::top("command-manager")
            .exact_height(124.0)
            .show(ctx, |ui| {
                if modal {
                    ui.disable();
                }
                ui.add_space(2.0);
                ui.horizontal(|ui| {
                    for (id, label) in [
                        (0, "Fonctions"),
                        (1, "Esquisse"),
                        (2, "Évaluer"),
                        (3, "Assemblage"),
                        (4, "Mise en plan"),
                        (5, "Affichage"),
                        (6, "Primitives v0.2"),
                    ] {
                        ui.selectable_value(
                            &mut self.ribbon_tab,
                            id,
                            RichText::new(label).size(13.0),
                        );
                    }
                });
                ui.separator();
                self.ribbon(ui, ctx);
            });
        egui::TopBottomPanel::bottom("status")
            .exact_height(24.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    let hint = match &self.editor.mode {
                        Mode::Sketch(edit, _) => edit.hint(),
                        _ if !self.editor.message.is_empty() => self.editor.message.clone(),
                        Mode::Feature { .. } => {
                            "Réglez la fonction à gauche, puis ✔ (Entrée) ou ✖ (Échap).".into()
                        }
                        _ => self.status.clone(),
                    };
                    ui.label(RichText::new(hint).size(11.0).color(MUTED));
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            RichText::new(format!("MMGS · v{VERSION}"))
                                .size(11.0)
                                .color(MUTED),
                        );
                        if let Mode::Sketch(edit, _) = &self.editor.mode {
                            if edit.status.fully_defined() {
                                ui.label(
                                    RichText::new("Totalement contrainte").size(11.0).strong(),
                                );
                            } else {
                                ui.label(
                                    RichText::new("Sous-contrainte")
                                        .size(11.0)
                                        .color(sketcher::SKETCH_BLUE),
                                );
                            }
                            if let Some(c) = edit.cursor {
                                ui.label(
                                    RichText::new(format!("X {:.2}  Y {:.2} mm", c[0], c[1]))
                                        .size(11.0)
                                        .color(MUTED),
                                );
                            }
                        }
                    });
                });
            });
        egui::SidePanel::left("feature-manager")
            .default_width(270.0)
            .width_range(220.0..=420.0)
            .resizable(true)
            .show(ctx, |ui| {
                if modal {
                    ui.disable();
                }
                egui::ScrollArea::vertical()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let title = self
                            .editor
                            .body
                            .and_then(|i| bodies.get(i))
                            .map(|b| b.name.clone())
                            .unwrap_or_else(|| self.project.name.clone());
                        self.editor.left_panel(ui, &bodies, &title, &mut self.view);
                    });
            });
        // Legacy selection follows the editor's body selection.
        if let Some(Sel::Body(i)) = self.editor.sel.first().copied() {
            let id = bodies.get(i).map(|b| b.id);
            if id != self.selected {
                self.select(id);
            }
        }
        if self.show_properties && self.selected.is_some() {
            egui::SidePanel::right("properties")
                .exact_width(280.0)
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.strong("Propriétés du corps");
                        if ui.small_button("Fermer").clicked() {
                            self.show_properties = false;
                        }
                    });
                    egui::ScrollArea::vertical().show(ui, |ui| self.properties(ui));
                });
        }
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.fill(Color32::from_rgb(247, 248, 250)))
            .show(ctx, |ui| {
                if modal {
                    ui.disable();
                }
                if self.explode_play {
                    let dt = ctx.input(|i| i.stable_dt).min(0.1) as f64;
                    self.explode_amount += dt * 0.35 * self.explode_direction;
                    if self.explode_amount >= 1.0 {
                        self.explode_amount = 1.0;
                        self.explode_direction = -1.0;
                    }
                    if self.explode_amount <= 0.0 {
                        self.explode_amount = 0.0;
                        self.explode_direction = 1.0;
                    }
                    ctx.request_repaint();
                }
                if self.displayed.is_empty() || self.displayed_amount != self.explode_amount {
                    let mut d =
                        workbench::display_meshes(&self.project, &self.meshes, self.explode_amount);
                    if let Some(i) = self.editor.body {
                        if let Some(b) = self.project.bodies.get(i) {
                            if i < d.len() {
                                d[i] = self.editor.mesh().transformed(b.position, b.rotation);
                            }
                        }
                    }
                    self.displayed = d;
                    self.displayed_amount = self.explode_amount;
                    self.version += 1;
                }
                let colors: Vec<[u8; 3]> = self
                    .project
                    .bodies
                    .iter()
                    .map(|b| {
                        if b.color == [224, 169, 103] {
                            [195, 199, 205]
                        } else {
                            b.color
                        }
                    })
                    .collect();
                let visible: Vec<bool> = self.project.bodies.iter().map(|b| b.visible).collect();
                self.view.left_orbit = self.editor.left_orbit(&self.view);
                self.view.tints(self.editor.tints());
                let response = self
                    .view
                    .show(ui, &self.displayed, &colors, &visible, self.version);
                if self.fit_next && !self.view.animating() {
                    let extra: Vec<Vec3> = if self.displayed.iter().all(|m| m.polygons.is_empty()) {
                        vec![Vec3::new(-50.0, -50.0, -50.0), Vec3::new(50.0, 50.0, 50.0)]
                    } else {
                        vec![]
                    };
                    self.view.fit(&self.displayed, &extra);
                    self.fit_next = false;
                }
                self.editor.viewport(ui, &bodies, &mut self.view, &response);
                response.context_menu(|ui| {
                    if ui.button("Zoom au cadre (F)").clicked() {
                        self.fit_next = true;
                        ui.close_menu();
                    }
                    if !self.editor.sel.is_empty() {
                        if ui.button("Esquisse").clicked() {
                            self.editor.start_sketch(&bodies, &mut self.view, ctx, None);
                            ui.close_menu();
                        }
                        if ui.button("Normal à").clicked() {
                            self.editor.normal_to(&bodies, &mut self.view, ctx);
                            ui.close_menu();
                        }
                        if let Some(Sel::Face(t)) = self.editor.sel.first().copied() {
                            if ui.button("Éditer la fonction").clicked() {
                                self.editor.edit_feature(
                                    &bodies,
                                    part::feature_of(t),
                                    &mut self.view,
                                    ctx,
                                );
                                ui.close_menu();
                            }
                        }
                        if ui.button("Mesurer").clicked() {
                            self.editor.mode = Mode::Measure;
                            ui.close_menu();
                        }
                    }
                    ui.menu_button("Vues", |ui| {
                        for (i, (name, _)) in viewport::VIEWS.iter().enumerate() {
                            if ui.button(*name).clicked() {
                                self.view.standard(i, ctx);
                                self.fit_next = true;
                                ui.close_menu();
                            }
                        }
                    });
                });
            });
        self.heads_up(ctx);
        for action in self.editor.take_actions() {
            match action {
                Action::Commit(part, label) => self.commit_part(part, &label),
            }
        }
        if let Some((next, select)) = self.workbench.show(ctx, &self.project, &self.meshes) {
            if self.commit(next) {
                if let Some(id) = select {
                    self.select(Some(id));
                }
                self.fit_next = true;
            }
        }
        if !matches!(self.dialog, Dialog::None) {
            self.dialogs(ctx);
        }
        if let Some(error) = self.error.clone() {
            egui::Window::new("Opération impossible")
                .collapsible(false)
                .anchor(egui::Align2::CENTER_CENTER, Vec2::new(0.0, 120.0))
                .show(ctx, |ui| {
                    ui.colored_label(Color32::from_rgb(180, 50, 50), error);
                    if ui.button("Compris").clicked() {
                        self.error = None;
                    }
                });
        }
    }
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        if let Ok(json) = serde_json::to_string(&self.recent) {
            storage.set_string("solidcraft-recent", json);
        }
        if let Ok(json) = serde_json::to_string(&self.project) {
            storage.set_string("solidcraft-project", json);
        }
    }
}
/// Command button of the ribbon: icon above a label on up to two lines.
fn ribbon_button(ui: &mut egui::Ui, label: &str, glyph: Glyph, enabled: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(80.0, 82.0),
        if enabled {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        },
    );
    let p = ui.painter_at(rect);
    if response.hovered() && enabled {
        p.rect_filled(rect, 3.0, Color32::from_rgb(216, 232, 248));
        p.rect_stroke(
            rect,
            3.0,
            egui::Stroke::new(1.0, Color32::from_rgb(160, 195, 235)),
            egui::StrokeKind::Inside,
        );
    }
    let icon =
        egui::Rect::from_center_size(rect.center_top() + Vec2::new(0.0, 19.0), Vec2::splat(30.0));
    paint_glyph(&p, icon, glyph, !enabled);
    let color = if enabled {
        Color32::from_rgb(40, 46, 56)
    } else {
        Color32::from_gray(165)
    };
    let galley = ui.painter().layout(
        label.to_string(),
        egui::FontId::proportional(10.5),
        color,
        74.0,
    );
    let pos = egui::pos2(rect.center().x - galley.size().x / 2.0, rect.top() + 38.0);
    p.galley(pos, galley, color);
    response.on_hover_text(label).clicked() && enabled
}
fn dimension(ui: &mut egui::Ui, label: &str, value: &mut f64, suffix: &str) {
    ui.horizontal(|ui| {
        ui.add_sized(
            [113.0, 25.0],
            egui::Label::new(RichText::new(label).size(12.0).color(MUTED)),
        );
        ui.add(
            egui::DragValue::new(value)
                .speed(0.1)
                .suffix(suffix)
                .max_decimals(3),
        );
    });
}
fn vector_fields(ui: &mut egui::Ui, values: &mut [f64; 3], unit: &str) {
    ui.horizontal(|ui| {
        for (i, axis) in ["X", "Y", "Z"].iter().enumerate() {
            ui.vertical(|ui| {
                ui.label(
                    RichText::new(format!("{axis} · {unit}"))
                        .size(10.0)
                        .color(MUTED),
                );
                ui.add_sized(
                    [68.0, 27.0],
                    egui::DragValue::new(&mut values[i])
                        .speed(0.1)
                        .max_decimals(3),
                );
            });
        }
    });
}
fn nonempty(s: &str) -> String {
    if s.trim().is_empty() {
        "Sans titre".into()
    } else {
        s.trim().to_owned()
    }
}
fn safe_name(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect()
}
fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1500.0, 920.0])
            .with_min_inner_size([1080.0, 680.0]),
        ..Default::default()
    };
    eframe::run_native(
        "SolidCraft — CAO paramétrique",
        options,
        Box::new(|cc| Ok(Box::new(SolidCraft::new(cc)))),
    )
}

/// Small vector icons of the legacy toolbars (assembly, drawing, primitives).
fn cad_tool(ui: &mut egui::Ui, label: &str, kind: u8, enabled: bool) -> bool {
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(78.0, 70.0),
        if enabled {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        },
    );
    let p = ui.painter_at(rect);
    if response.hovered() && enabled {
        p.rect_filled(rect, 3.0, Color32::from_rgb(216, 232, 248));
    }
    let center = rect.center_top() + Vec2::new(0.0, 22.0);
    let line = if enabled {
        Color32::from_rgb(81, 103, 123)
    } else {
        Color32::from_gray(177)
    };
    let fill = if enabled {
        Color32::from_rgb(181, 204, 222)
    } else {
        Color32::from_gray(219)
    };
    let stroke = egui::Stroke::new(1.25, line);
    let at = |x: f32, y: f32| center + Vec2::new(x, y);
    match kind {
        1 | 2 => {
            p.rect_filled(
                egui::Rect::from_min_max(at(-11.0, -9.0), at(11.0, 11.0)),
                0.0,
                fill,
            );
            p.add(egui::Shape::ellipse_filled(
                at(0.0, 11.0),
                Vec2::new(11.0, 4.0),
                fill,
            ));
            p.add(egui::Shape::ellipse_stroke(
                at(0.0, 11.0),
                Vec2::new(11.0, 4.0),
                stroke,
            ));
            p.line_segment([at(-11.0, -9.0), at(-11.0, 11.0)], stroke);
            p.line_segment([at(11.0, -9.0), at(11.0, 11.0)], stroke);
            p.add(egui::Shape::ellipse_filled(
                at(0.0, -9.0),
                Vec2::new(11.0, 4.0),
                Color32::from_rgb(226, 236, 243),
            ));
            p.add(egui::Shape::ellipse_stroke(
                at(0.0, -9.0),
                Vec2::new(11.0, 4.0),
                stroke,
            ));
            if kind == 2 {
                p.add(egui::Shape::ellipse_filled(
                    at(0.0, -9.0),
                    Vec2::new(6.0, 2.5),
                    line,
                ));
            }
        }
        3 => {
            p.add(egui::Shape::closed_line(
                vec![
                    at(-15.0, 10.0),
                    at(-8.0, -10.0),
                    at(13.0, -6.0),
                    at(5.0, 14.0),
                ],
                stroke,
            ));
        }
        8 | 9 => {
            let r = egui::Rect::from_min_max(at(-11.0, -16.0), at(11.0, 15.0));
            p.rect_filled(r, 1.0, Color32::WHITE);
            p.rect_stroke(r, 1.0, stroke, egui::StrokeKind::Inside);
            for y in [-8.0, 0.0, 8.0] {
                p.line_segment([at(-6.0, y), at(6.0, y)], stroke);
            }
        }
        _ => {
            p.add(egui::Shape::convex_polygon(
                vec![
                    at(-13.0, -7.0),
                    at(0.0, -14.0),
                    at(13.0, -7.0),
                    at(0.0, 0.0),
                ],
                Color32::from_rgb(221, 232, 240),
                stroke,
            ));
            p.add(egui::Shape::convex_polygon(
                vec![at(-13.0, -7.0), at(0.0, 0.0), at(0.0, 15.0), at(-13.0, 8.0)],
                fill,
                stroke,
            ));
            p.add(egui::Shape::convex_polygon(
                vec![at(0.0, 0.0), at(13.0, -7.0), at(13.0, 8.0), at(0.0, 15.0)],
                Color32::from_rgb(149, 177, 199),
                stroke,
            ));
            if kind >= 5 {
                p.rect_filled(
                    egui::Rect::from_min_max(at(3.0, 1.0), at(18.0, 15.0)),
                    1.0,
                    if kind == 6 {
                        Color32::from_rgb(240, 242, 246)
                    } else {
                        Color32::from_rgb(207, 189, 129)
                    },
                );
            }
        }
    }
    let galley = ui.painter().layout(
        label.to_string(),
        egui::FontId::proportional(10.5),
        if enabled {
            Color32::from_rgb(48, 54, 64)
        } else {
            Color32::from_gray(162)
        },
        74.0,
    );
    let pos = egui::pos2(rect.center().x - galley.size().x / 2.0, rect.top() + 44.0);
    p.galley(pos, galley, Color32::BLACK);
    response.on_hover_text(label).clicked() && enabled
}
