//! The Anwendungen tab: every button opens a working tool. Welding, mould and casting design,
//! the mould base, generative design, the simulation entries (Ansys Simulation, Simulate
//! convergence study, Simulate Ergebnisse, Flow Analysis), the filling simulation, the
//! Mathcad-style worksheet, the GD&T Advisor, EZ Tolerance Analysis and Render Studio.
//! The engines live in `crate::apps`; this file holds the dialogs (with `apps2.rs`).

use super::dlgkit::{btn, combo, cwin, hline, lbl, lbl_bold, lbl_dim, pad};
use super::icons::Icon;
use super::ribbon::{drop_menu, group, large, menu_row, rich_tip, split};
use super::*;
use crate::apps::{self, gdt as adv, moldflow, raytrace, stackup, topopt, worksheet};
use crate::model::sim::StudyKind;
use crate::sim::run::{self as simrun, Shared, SimInput, SimOutput};
use eframe::egui::{self, pos2, vec2, Color32, RichText, Sense, Stroke};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

/// Work on a background thread with progress, cancel and a typed result.
pub struct Job<T> {
    pub shared: Arc<Shared>,
    pub result: Arc<Mutex<Option<Result<T, String>>>>,
    pub started: std::time::Instant,
}

impl<T: Send + 'static> Job<T> {
    pub fn spawn(f: impl FnOnce(&Shared) -> Result<T, String> + Send + 'static) -> Job<T> {
        let shared =
            Arc::new(Shared { progress: AtomicU32::new(0), cancel: AtomicBool::new(false), stage: Mutex::new(String::new()), partial: Mutex::new(None), done: Mutex::new(None) });
        let result = Arc::new(Mutex::new(None));
        let (s2, r2) = (shared.clone(), result.clone());
        std::thread::spawn(move || {
            let r = f(&s2);
            *r2.lock().unwrap() = Some(r);
        });
        Job { shared, result, started: std::time::Instant::now() }
    }
    pub fn take(&self) -> Option<Result<T, String>> {
        self.result.lock().unwrap().take()
    }
    pub fn cancel(&self) {
        self.shared.cancel.store(true, Ordering::Relaxed);
    }
    pub fn progress(&self) -> f32 {
        self.shared.progress()
    }
    pub fn stage(&self) -> String {
        self.shared.stage.lock().unwrap().clone()
    }
    pub fn partial(&self) -> Option<SimOutput> {
        self.shared.partial.lock().unwrap().take()
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Launch {
    Ansys,
    Flow,
}

pub struct ConvRow {
    pub fidelity: f32,
    pub mesh: String,
    pub metrics: Vec<(String, f64, String)>,
    pub seconds: f64,
    pub out: Arc<SimOutput>,
}

pub struct FillShown {
    pub out: Arc<SimOutput>,
    pub fill: Vec<f32>,
    pub time: f64,
}

pub struct AppsUi {
    // Schweißen
    pub weld_open: bool,
    pub weld_sel: Option<usize>,
    pub weld_kind: usize,
    pub weld_a: f64,
    pub weld_both: bool,
    pub weld_proc: usize,
    pub weld_mat: usize,
    pub weld_force: f64,
    pub weld_plate: f64,
    pub weld_rate: f64,
    // Spritzguss/Guss and Expert Moldbase
    pub mold_open: bool,
    pub mold_dir: usize,
    pub mold_mat: usize,
    pub mold_draft: f64,
    pub mold_cav: usize,
    pub mold_res: Option<super::apps2::MoldShown>,
    pub base_open: bool,
    // Generatives Design
    pub gen_open: bool,
    pub gen_volume: f64,
    pub gen_iter: usize,
    pub gen_res: usize,
    pub gen_sym: usize,
    pub gen_skin: bool,
    pub gen_job: Option<Job<topopt::OptResult>>,
    pub gen_stl: Option<Vec<u8>>,
    pub gen_history: Vec<(f64, f64)>,
    // simulation entries
    pub launch: Option<Launch>,
    pub launch_kind: StudyKind,
    pub launch_fid: f32,
    pub flow_fluid: usize,
    pub flow_v: f64,
    pub flow_d: f64,
    pub flow_l: f64,
    pub conv_open: bool,
    pub conv_job: Option<Job<()>>,
    pub conv_rows: Arc<Mutex<Vec<ConvRow>>>,
    pub results_open: bool,
    pub res_a: usize,
    pub res_b: usize,
    // Spritzgussanalyse
    pub fill_open: bool,
    pub fill_mat: usize,
    pub fill_time: f64,
    pub fill_res: usize,
    pub fill_gate_sel: bool,
    pub fill_job: Option<Job<moldflow::FillResult>>,
    pub fill_shown: Option<FillShown>,
    pub fill_t: f32,
    pub fill_play: bool,
    // Mathcad
    pub math_open: bool,
    pub math_export: std::collections::BTreeSet<String>,
    // GD&T Advisor
    pub gdt_open: bool,
    pub gdt_class: usize,
    pub gdt_items: Vec<adv::Item>,
    pub gdt_checks: Vec<String>,
    pub gdt_done: bool,
    // EZ Tolerance Analysis
    pub tol_open: bool,
    pub tol_sel: usize,
    pub tol_class: usize,
    pub tol_cache: Option<(u64, stackup::Result)>,
    // Render Studio
    pub render_open: bool,
    pub render: Option<Arc<raytrace::Render>>,
    pub render_tex: Option<egui::TextureHandle>,
    pub render_shown: u32,
    pub render_scene: usize,
    pub render_mat: usize,
    pub render_size: usize,
    pub render_ground: bool,
    pub render_mirror: bool,
    pub render_exposure: f64,
    pub render_az: f64,
    pub render_el: f64,
    pub render_quality: usize,
    /// start rendering once the view is fitted (screenshot scenes)
    pub render_auto: bool,
}

impl Default for AppsUi {
    fn default() -> Self {
        AppsUi {
            weld_open: false,
            weld_sel: None,
            weld_kind: 0,
            weld_a: 4.0,
            weld_both: false,
            weld_proc: 0,
            weld_mat: 0,
            weld_force: 10000.0,
            weld_plate: 8.0,
            weld_rate: 65.0,
            mold_open: false,
            mold_dir: 0,
            mold_mat: 0,
            mold_draft: 1.0,
            mold_cav: 1,
            mold_res: None,
            base_open: false,
            gen_open: false,
            gen_volume: 0.35,
            gen_iter: 40,
            gen_res: 1,
            gen_sym: 0,
            gen_skin: false,
            gen_job: None,
            gen_stl: None,
            gen_history: vec![],
            launch: None,
            launch_kind: StudyKind::Structural,
            launch_fid: 0.9,
            flow_fluid: 1,
            flow_v: 2.0,
            flow_d: 20.0,
            flow_l: 1000.0,
            conv_open: false,
            conv_job: None,
            conv_rows: Arc::new(Mutex::new(vec![])),
            results_open: false,
            res_a: 0,
            res_b: 1,
            fill_open: false,
            fill_mat: 0,
            fill_time: 1.0,
            fill_res: 1,
            fill_gate_sel: false,
            fill_job: None,
            fill_shown: None,
            fill_t: 1.0,
            fill_play: false,
            math_open: false,
            math_export: Default::default(),
            gdt_open: false,
            gdt_class: 1,
            gdt_items: vec![],
            gdt_checks: vec![],
            gdt_done: false,
            tol_open: false,
            tol_sel: 0,
            tol_class: 1,
            tol_cache: None,
            render_open: false,
            render: None,
            render_tex: None,
            render_shown: 0,
            render_scene: 0,
            render_mat: 0,
            render_size: 1,
            render_ground: true,
            render_mirror: false,
            render_exposure: 1.0,
            render_az: -55.0,
            render_el: 55.0,
            render_quality: 1,
            render_auto: false,
        }
    }
}

/// A progress bar with stage text and elapsed time; returns true when Abbrechen was clicked.
pub fn progress_row(ui: &mut egui::Ui, frac: f32, stage: &str, secs: f64) -> bool {
    let mut stop = false;
    ui.horizontal(|ui| {
        ui.add(egui::ProgressBar::new(frac.clamp(0.0, 1.0)).desired_width(ui.available_width() - 110.0).text(format!("{} – {:.0} %, {:.0} s", stage, frac * 100.0, secs)));
        if ui.button("Abbrechen").clicked() {
            stop = true;
        }
    });
    stop
}

/// Label / value rows in two columns.
pub fn kv(ui: &mut egui::Ui, id: &str, rows: &[(String, String)]) {
    egui::Grid::new(id).num_columns(2).spacing(vec2(16.0, 5.0)).show(ui, |ui| {
        for (k, v) in rows {
            ui.label(RichText::new(k).color(dlgkit_dim()));
            ui.label(RichText::new(v).color(Color32::from_rgb(30, 34, 40)));
            ui.end_row();
        }
    });
}

pub fn dlgkit_dim() -> Color32 {
    Color32::from_rgb(92, 98, 108)
}

pub fn warn_rows(ui: &mut egui::Ui, w: &[String]) {
    for t in w {
        ui.horizontal_wrapped(|ui| {
            ui.label(RichText::new("⚠").color(Color32::from_rgb(214, 120, 0)));
            ui.label(RichText::new(t).color(Color32::from_rgb(120, 70, 0)));
        });
    }
}

/// Leading number of a result string ("12.5 MPa" → 12.5).
pub fn lead_num(s: &str) -> Option<f64> {
    let t: String = s.trim().chars().take_while(|c| c.is_ascii_digit() || matches!(c, '.' | ',' | '-' | '+' | 'e' | 'E')).collect();
    t.replace(',', ".").parse().ok()
}

impl App {
    // ------------------------------------------------------------ ribbon

    pub fn apps_ribbon_ui(&mut self, ui: &mut egui::Ui) {
        let part = !self.in_asm() && !self.result.topo.polys.is_empty();
        group(ui, "Konstruktion", |ui| {
            if rich_tip(
                large(ui, Icon::Weld, "Schweißen", part, self.apps.weld_open),
                Icon::Weld,
                "Schweißen",
                "Schweißnähte an Kanten definieren: Nahtgeometrie, Zusatzwerkstoff, Schweißzeit, Kosten und Festigkeitsnachweis nach EN 1993-1-8.",
            )
            .clicked()
            {
                self.apps.weld_open = !self.apps.weld_open;
            }
            if rich_tip(
                large(ui, Icon::Mold, "Spritzguss/\nGuss", part, self.apps.mold_open),
                Icon::Mold,
                "Spritzguss/Guss",
                "Entformbarkeit prüfen: Schrägen und Hinterschnitte, Teilungsebene, projizierte Fläche, Schließkraft, Schwindung und Kühlzeit.",
            )
            .clicked()
            {
                self.apps.mold_open = !self.apps.mold_open;
                if self.apps.mold_open && self.apps.mold_res.is_none() {
                    self.mold_analyze();
                }
            }
            if rich_tip(
                large(ui, Icon::Moldbase, "Expert\nMoldbase", part, self.apps.base_open),
                Icon::Moldbase,
                "Expert Moldbase",
                "Normalien-Formaufbau für das Teil: Plattengrößen, Führungen, Auswerferhub, Werkzeughöhe, Stahlgewicht und passende Spritzgießmaschine.",
            )
            .clicked()
            {
                self.open_moldbase();
            }
            if rich_tip(
                large(ui, Icon::GenDesign, "Generatives\nDesign", part, self.apps.gen_open),
                Icon::GenDesign,
                "Generatives Design",
                "Topologieoptimierung: aus dem Teil als Bauraum und den Lasten der Struktur-Studie die steifste Form mit dem gewählten Materialanteil berechnen.",
            )
            .clicked()
            {
                self.apps.gen_open = !self.apps.gen_open;
            }
        });
        group(ui, "Simulation", |ui| {
            if rich_tip(
                large(ui, Icon::Simulate, "Ansys\nSimulation", part, self.apps.launch == Some(Launch::Ansys)),
                Icon::Simulate,
                "Ansys Simulation",
                "Simulationsstudie mit hoher Genauigkeit anlegen (Struktur, thermisch, modal, Fluid) und in der Live-Simulation lösen.",
            )
            .clicked()
            {
                self.open_launch(Launch::Ansys);
            }
            if rich_tip(
                large(ui, Icon::Simulate, "Simulate", part, self.apps.conv_open),
                Icon::Simulate,
                "Simulate",
                "Konvergenzstudie der aktiven Studie: das Netz schrittweise verfeinern, bis sich die Ergebnisse nicht mehr ändern.",
            )
            .clicked()
            {
                self.apps.conv_open = !self.apps.conv_open;
            }
            if rich_tip(
                large(ui, Icon::SimResults, "Simulate\nErgebnisse", true, self.apps.results_open),
                Icon::SimResults,
                "Simulate Ergebnisse",
                "Ergebnisse anzeigen, vergleichen, als Bericht oder CSV ausgeben.",
            )
            .clicked()
            {
                self.apps.results_open = !self.apps.results_open;
            }
            if rich_tip(
                large(ui, Icon::Flow, "Flow\nAnalysis", part, self.apps.launch == Some(Launch::Flow)),
                Icon::Flow,
                "Flow Analysis",
                "Strömungsstudie anlegen (Innen- oder Umströmung) und Rohrströmung überschlagen: Reynolds-Zahl, Strömungsform, Druckverlust.",
            )
            .clicked()
            {
                self.open_launch(Launch::Flow);
            }
            if rich_tip(
                large(ui, Icon::MoldFlow, "Spritzgussanalyse", part, self.apps.fill_open),
                Icon::MoldFlow,
                "Spritzgussanalyse",
                "Füllsimulation: Füllbild, Druck, Wanddicke, Kühlzeit, Bindenähte und Lufteinschlüsse ab einem Anschnitt.",
            )
            .clicked()
            {
                self.apps.fill_open = !self.apps.fill_open;
            }
        });
        group(ui, "Berechnung", |ui| {
            let (main, arrow) = split(ui, Icon::Mathcad, "Mathcad", true, self.apps.math_open);
            if rich_tip(
                main,
                Icon::Mathcad,
                "Mathcad",
                "Berechnungsblatt im Teil: Formeln mit Modellparametern und Masseneigenschaften, Ergebnisse als Parameter zurück ins Modell.",
            )
            .clicked()
            {
                self.apps.math_open = !self.apps.math_open;
            }
            let mut pick = None;
            drop_menu(ui, "apps_mathcad", &arrow, |ui| {
                if menu_row(ui, Some(Icon::Mathcad), "Berechnungsblatt öffnen", "", true, false).clicked() {
                    pick = Some(usize::MAX);
                }
                for (i, (n, _)) in worksheet::TEMPLATES.iter().enumerate() {
                    if menu_row(ui, None, &format!("Vorlage: {}", n), "", true, false).clicked() {
                        pick = Some(i);
                    }
                }
            });
            if let Some(i) = pick {
                self.apps.math_open = true;
                if let Some((n, t)) = worksheet::TEMPLATES.get(i) {
                    self.math_insert_template(n, t);
                }
            }
        });
        group(ui, "GD&T", |ui| {
            if rich_tip(
                large(ui, Icon::GdtAdvisor, "GD&T\nAdvisor", part, self.apps.gdt_open),
                Icon::GdtAdvisor,
                "GD&T Advisor",
                "Bezugssystem und Form- und Lagetoleranzen aus der Geometrie vorschlagen und die vorhandenen Angaben prüfen.",
            )
            .clicked()
            {
                self.apps.gdt_open = !self.apps.gdt_open;
                if self.apps.gdt_open {
                    self.gdt_analyze();
                }
            }
            if rich_tip(
                large(ui, Icon::EzTol, "EZ Tolerance\nAnalysis", true, self.apps.tol_open),
                Icon::EzTol,
                "EZ Tolerance Analysis",
                "Maßketten: Worst Case, statistisch (RSS) und Monte Carlo, Beiträge, Cp/Cpk und Ausschuss.",
            )
            .clicked()
            {
                self.apps.tol_open = !self.apps.tol_open;
            }
        });
        group(ui, "Rendern", |ui| {
            if rich_tip(
                large(ui, Icon::Render, "Render\nStudio", part, self.apps.render_open),
                Icon::Render,
                "Render Studio",
                "Fotorealistisches Bild der aktuellen Ansicht: Materialien, Studio- oder Außenlicht, weiche Schatten und Spiegelungen.",
            )
            .clicked()
            {
                self.apps.render_open = !self.apps.render_open;
            }
        });
    }

    /// All windows of the Anwendungen tab and their background work.
    pub fn apps_windows(&mut self, ctx: &egui::Context) {
        self.apps_tick(ctx);
        if self.apps.weld_open {
            self.weld_window(ctx);
        }
        if self.apps.mold_open {
            self.mold_window(ctx);
        }
        if self.apps.base_open {
            self.moldbase_window(ctx);
        }
        if self.apps.gen_open {
            self.gen_window(ctx);
        }
        if self.apps.launch.is_some() {
            self.launch_window(ctx);
        }
        if self.apps.conv_open {
            self.conv_window(ctx);
        }
        if self.apps.results_open {
            self.results_window(ctx);
        }
        if self.apps.fill_open {
            self.fill_window(ctx);
        }
        if self.apps.math_open {
            self.math_window(ctx);
        }
        if self.apps.gdt_open {
            self.gdt_window(ctx);
        }
        if self.apps.tol_open {
            self.tol_window(ctx);
        }
        if self.apps.render_open {
            self.render_window(ctx);
        }
    }

    fn apps_tick(&mut self, ctx: &egui::Context) {
        let busy = self.apps.gen_job.is_some() || self.apps.fill_job.is_some() || self.apps.conv_job.is_some() || self.apps.fill_play;
        if busy {
            ctx.request_repaint_after(std::time::Duration::from_millis(80));
        }
        self.gen_tick();
        self.fill_tick();
        if let Some(j) = &self.apps.conv_job {
            if let Some(r) = j.take() {
                self.apps.conv_job = None;
                match r {
                    Ok(()) => {
                        let best = self.apps.conv_rows.lock().unwrap().last().map(|r| r.out.clone());
                        if let Some(o) = best {
                            self.apps_show(o, "Simulate – Konvergenzstudie");
                        }
                        self.say("Konvergenzstudie abgeschlossen.", MsgKind::Success);
                    }
                    Err(e) if e == "abgebrochen" => {}
                    Err(e) => self.say(format!("Simulate: {}", e), MsgKind::Error),
                }
            }
        }
    }

    /// Puts a result into the Live-Simulation display (and the list of saved analyses).
    pub fn apps_show(&mut self, out: Arc<SimOutput>, name: &str) {
        if let Some(j) = self.sim.job.take() {
            j.shared.cancel.store(true, Ordering::Relaxed);
        }
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) % 86400;
        if !out.partial {
            self.sim.saved.retain(|s| s.name != name);
            self.sim.saved.push(livesim::SavedAnalysis { name: name.to_string(), when: format!("{:02}:{:02} UTC", secs / 3600, secs / 60 % 60), out: out.clone() });
        }
        self.apps_display(out);
    }

    /// Shows a result without saving it (animation frames, partial results).
    pub fn apps_display(&mut self, out: Arc<SimOutput>) {
        if self.sim.out.as_ref().is_none_or(|o| o.surface.pos.len() != out.surface.pos.len()) {
            self.sim.probes.clear();
            self.sim.field = 0;
        }
        if self.sim.field >= out.fields.len() {
            self.sim.field = 0;
        }
        self.sim.out = Some(out);
        self.sim.viewing_saved = true;
        self.sim.show_results = true;
        self.tab = Tab::LiveSim;
    }

    /// Extra lines for the graphics window: weld beads and the mould's parting plane.
    pub fn apps_lines(&self, lines: &mut Vec<f32>) {
        if self.in_asm() || self.sim_showing() {
            return;
        }
        let topo = &self.result.topo;
        let push = |v: &mut Vec<f32>, a: V3, b: V3, c: [f32; 4]| {
            v.extend_from_slice(&[a.x as f32, a.y as f32, a.z as f32, c[0], c[1], c[2], c[3], b.x as f32, b.y as f32, b.z as f32, c[0], c[1], c[2], c[3]]);
        };
        for (i, w) in self.data.apps.welds.iter().enumerate() {
            let sel = self.apps.weld_open && self.apps.weld_sel == Some(i);
            let c = if sel { [0.95, 0.15, 0.1, 1.0] } else { [0.93, 0.48, 0.05, 1.0] };
            for (a, b) in crate::apps::weld::bead_lines(w, topo) {
                push(lines, a, b, c);
            }
            for ch in w.chains(topo) {
                for s in topo.chains[ch].points.windows(2) {
                    push(lines, s[0], s[1], c);
                }
            }
        }
        if self.apps.mold_open || self.apps.base_open {
            if let Some(m) = &self.apps.mold_res {
                if m.generation == self.result.generation {
                    for (a, b) in m.plane_lines() {
                        push(lines, a, b, [0.1, 0.45, 0.85, 1.0]);
                    }
                }
            }
        }
    }

    // ------------------------------------------------------------ Ansys Simulation / Flow Analysis

    fn open_launch(&mut self, l: Launch) {
        if self.apps.launch == Some(l) {
            self.apps.launch = None;
            return;
        }
        self.apps.launch = Some(l);
        self.apps.launch_kind =
            if l == Launch::Flow { StudyKind::Fluid } else { self.sim_study().map(|s| s.kind).filter(|k| *k != StudyKind::Fluid).unwrap_or(StudyKind::Structural) };
        self.apps.launch_fid = if l == Launch::Ansys { 0.9 } else { 0.6 };
    }

    fn launch_window(&mut self, ctx: &egui::Context) {
        let Some(l) = self.apps.launch else { return };
        let title = if l == Launch::Ansys { "Ansys Simulation" } else { "Flow Analysis" };
        let existing: Vec<(usize, String, StudyKind)> = self.data.sims.iter().enumerate().map(|(i, s)| (i, s.name.clone(), s.kind)).collect();
        let mut go: Option<Option<usize>> = None;
        let a = &mut self.apps;
        let open = cwin(ctx, "apps_launch", title, false, pos2(430.0, 170.0), 520.0, |ui| {
            pad(ui, 12.0, |ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                lbl_bold(ui, "Studientyp");
                let kinds: Vec<StudyKind> = if l == Launch::Flow { vec![StudyKind::Fluid] } else { StudyKind::ALL.to_vec() };
                for k in kinds {
                    ui.horizontal(|ui| {
                        if ui.radio(a.launch_kind == k, RichText::new(k.title()).strong()).clicked() {
                            a.launch_kind = k;
                        }
                    });
                    ui.label(RichText::new(k.text()).size(12.5).color(dlgkit_dim()));
                }
                ui.add_space(4.0);
                lbl_bold(ui, "Genauigkeit");
                ui.horizontal(|ui| {
                    ui.add(egui::Slider::new(&mut a.launch_fid, 0.0..=1.0).show_value(false));
                    lbl_dim(
                        ui,
                        &format!(
                            "{:.0} % – {}",
                            a.launch_fid * 100.0,
                            if a.launch_fid > 0.75 {
                                "fein (Ansys-Niveau)"
                            } else if a.launch_fid > 0.4 {
                                "mittel"
                            } else {
                                "schnell"
                            }
                        ),
                    );
                });
                let same: Vec<&(usize, String, StudyKind)> = existing.iter().filter(|s| s.2 == a.launch_kind).collect();
                if !same.is_empty() {
                    ui.add_space(4.0);
                    lbl_bold(ui, "Vorhandene Studie öffnen");
                    for (i, n, _) in same {
                        if ui.link(n).clicked() {
                            go = Some(Some(*i));
                        }
                    }
                }
                if l == Launch::Flow {
                    ui.add_space(6.0);
                    hline(ui);
                    ui.add_space(4.0);
                    lbl_bold(ui, "Rohrströmung überschlagen");
                    let names: Vec<&str> = crate::model::sim::FLUIDS.iter().map(|f| f.name).collect();
                    ui.horizontal(|ui| {
                        lbl(ui, "Medium");
                        combo(ui, "flow_fluid", 170.0, &mut a.flow_fluid, &names, true, None);
                    });
                    egui::Grid::new("flow_in").num_columns(2).spacing(vec2(10.0, 4.0)).show(ui, |ui| {
                        ui.label("Geschwindigkeit");
                        ui.add(egui::DragValue::new(&mut a.flow_v).speed(0.05).range(0.001..=500.0).suffix(" m/s"));
                        ui.end_row();
                        ui.label("Innendurchmesser");
                        ui.add(egui::DragValue::new(&mut a.flow_d).speed(0.2).range(0.1..=5000.0).suffix(" mm"));
                        ui.end_row();
                        ui.label("Rohrlänge");
                        ui.add(egui::DragValue::new(&mut a.flow_l).speed(5.0).range(1.0..=1e6).suffix(" mm"));
                        ui.end_row();
                    });
                    let f = crate::model::sim::FLUIDS[a.flow_fluid.min(crate::model::sim::FLUIDS.len() - 1)];
                    let d = a.flow_d / 1000.0;
                    let re = a.flow_v * d / f.nu;
                    // Darcy friction: laminar 64/Re, turbulent Blasius / Haaland (smooth pipe)
                    let lam = if re < 2300.0 { 64.0 / re.max(1e-9) } else { (1.0 / (-1.8 * (6.9 / re).log10())).powi(2) };
                    let dp = lam * (a.flow_l / 1000.0) / d * f.rho * a.flow_v * a.flow_v / 2.0;
                    let q = a.flow_v * std::f64::consts::PI * d * d / 4.0 * 1000.0 * 60.0;
                    kv(
                        ui,
                        "flow_out",
                        &[
                            ("Reynolds-Zahl".into(), apps::num(re)),
                            (
                                "Strömungsform".into(),
                                if re < 2300.0 {
                                    "laminar".into()
                                } else if re < 4000.0 {
                                    "Übergangsbereich".into()
                                } else {
                                    "turbulent".into()
                                },
                            ),
                            ("Volumenstrom".into(), format!("{} l/min", apps::num(q))),
                            ("Rohrreibungszahl λ".into(), apps::num(lam)),
                            ("Druckverlust".into(), format!("{} Pa ({} mbar)", apps::num(dp), apps::num(dp / 100.0))),
                        ],
                    );
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if btn(ui, "Neue Studie anlegen und lösen", 230.0, 1) {
                        go = Some(None);
                    }
                });
            });
        });
        if !open {
            self.apps.launch = None;
        }
        if let Some(which) = go {
            let kind = self.apps.launch_kind;
            self.sim.fidelity = self.apps.launch_fid;
            match which {
                Some(i) => {
                    self.tab = Tab::LiveSim;
                    self.sim_activate(i);
                }
                None => {
                    self.tab = Tab::LiveSim;
                    self.sim_new_study(kind);
                }
            }
            self.sim.viewing_saved = false;
            self.sim_run_now();
            self.apps.launch = None;
            let hint = match kind {
                StudyKind::Fluid => "Einlass und Auslass (Innenströmung) oder eine Anströmung (Umströmung) im Register Live-Simulation hinzufügen.",
                StudyKind::Thermal => "Temperaturen, Wärmeströme oder Konvektion im Register Live-Simulation hinzufügen.",
                StudyKind::Modal => "Randbedingungen hinzufügen: die Eigenfrequenzen werden sofort berechnet.",
                StudyKind::Structural => "Randbedingungen und Lasten im Register Live-Simulation hinzufügen: gelöst wird automatisch.",
            };
            self.say(hint, MsgKind::Prompt);
        }
    }

    // ------------------------------------------------------------ Simulate: convergence study

    fn conv_start(&mut self) {
        let Some(study) = self.sim_study().cloned() else {
            self.say("Simulate: zuerst eine Studie anlegen (Ansys Simulation oder Register Live-Simulation).", MsgKind::Warning);
            return;
        };
        if study.items.is_empty() {
            self.say("Simulate: die aktive Studie hat noch keine Randbedingungen oder Lasten.", MsgKind::Warning);
            return;
        }
        let topo = self.result.topo.clone();
        let rows = self.apps.conv_rows.clone();
        rows.lock().unwrap().clear();
        self.apps.conv_job = Some(Job::spawn(move |sh| {
            let passes = [0.1f32, 0.35, 0.6, 0.85];
            for (i, &f) in passes.iter().enumerate() {
                if sh.cancel.load(Ordering::Relaxed) {
                    return Err("abgebrochen".into());
                }
                *sh.stage.lock().unwrap() = format!("Durchgang {} von {}", i + 1, passes.len());
                sh.progress.store((i as f32 / passes.len() as f32).to_bits(), Ordering::Relaxed);
                let out = simrun::run_blocking(&SimInput { topo: topo.clone(), study: study.clone(), fidelity: f })?;
                let mut metrics = vec![];
                match out.kind {
                    StudyKind::Modal => {
                        for fl in out.fields.iter().take(3) {
                            if let Some(hz) = fl.name.split(':').nth(1).and_then(lead_num) {
                                metrics.push((fl.name.split(':').next().unwrap_or("").to_string(), hz, "Hz".to_string()));
                            }
                        }
                    }
                    StudyKind::Structural => {
                        for fl in out.fields.iter().take(2) {
                            metrics.push((format!("Max. {}", fl.name), fl.max as f64, fl.unit.clone()));
                        }
                    }
                    _ => {
                        if let Some(fl) = out.fields.first() {
                            metrics.push((format!("Max. {}", fl.name), fl.max as f64, fl.unit.clone()));
                            metrics.push((format!("Min. {}", fl.name), fl.min as f64, fl.unit.clone()));
                        }
                    }
                }
                let mesh = out.summary.iter().find(|s| s.0 == "Netz").map(|s| s.1.split(',').next().unwrap_or("").to_string()).unwrap_or_default();
                let seconds = out.seconds;
                rows.lock().unwrap().push(ConvRow { fidelity: f, mesh, metrics, seconds, out: Arc::new(out) });
            }
            sh.progress.store(1f32.to_bits(), Ordering::Relaxed);
            Ok(())
        }));
    }

    fn conv_window(&mut self, ctx: &egui::Context) {
        let study = self.sim_study().map(|s| format!("{} ({}, {} Objekte)", s.name, s.kind.short(), s.items.len()));
        let mut start = false;
        let mut stop = false;
        let mut show: Option<Arc<SimOutput>> = None;
        let job = self.apps.conv_job.as_ref().map(|j| (j.progress(), j.stage(), j.started.elapsed().as_secs_f64()));
        let rows = self.apps.conv_rows.clone();
        let open = cwin(ctx, "apps_conv", "Simulate – Konvergenzstudie", false, pos2(440.0, 160.0), 620.0, |ui| {
            pad(ui, 12.0, |ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                ui.label(RichText::new("Die aktive Studie wird in vier Durchgängen mit immer feinerem Netz gelöst (wie die Mehrfach-Pass-Konvergenz in Creo Simulate). Ändert sich das Ergebnis zwischen den letzten Durchgängen um weniger als 5 %, gilt es als konvergiert.").color(dlgkit_dim()));
                match &study {
                    Some(s) => lbl(ui, &format!("Studie: {}", s)),
                    None => lbl_dim(ui, "Keine Studie vorhanden: zuerst mit Ansys Simulation eine Studie anlegen."),
                };
                if let Some((p, s, t)) = &job {
                    stop = progress_row(ui, *p, s, *t);
                }
                let rows = rows.lock().unwrap();
                if !rows.is_empty() {
                    let names: Vec<(String, String)> = rows[0].metrics.iter().map(|m| (m.0.clone(), m.2.clone())).collect();
                    egui::Grid::new("conv_tab").striped(true).num_columns(3 + names.len()).spacing(vec2(14.0, 4.0)).show(ui, |ui| {
                        ui.label(RichText::new("Pass").strong());
                        ui.label(RichText::new("Netz").strong());
                        for (n, u) in &names {
                            ui.label(RichText::new(format!("{} [{}]", n, u)).strong());
                        }
                        ui.label(RichText::new("Zeit").strong());
                        ui.end_row();
                        for (i, r) in rows.iter().enumerate() {
                            ui.label(format!("{}", i + 1));
                            ui.label(&r.mesh);
                            for (k, m) in r.metrics.iter().enumerate() {
                                let prev = if i > 0 { rows[i - 1].metrics.get(k).map(|p| p.1) } else { None };
                                let d = prev.map(|p| (m.1 - p) / p.abs().max(1e-12) * 100.0);
                                let txt = match d {
                                    Some(d) => format!("{}  ({:+.1} %)", apps::num(m.1), d),
                                    None => apps::num(m.1),
                                };
                                let ok = d.is_some_and(|d| d.abs() < 5.0);
                                ui.label(RichText::new(txt).color(if ok { Color32::from_rgb(20, 130, 50) } else { Color32::from_rgb(30, 34, 40) }));
                            }
                            ui.label(format!("{:.1} s", r.seconds));
                            if ui.small_button("Anzeigen").clicked() {
                                show = Some(r.out.clone());
                            }
                            ui.end_row();
                        }
                    });
                    if rows.len() >= 2 && job.is_none() {
                        let (a, b) = (&rows[rows.len() - 2], &rows[rows.len() - 1]);
                        let worst = a.metrics.iter().zip(&b.metrics).map(|(x, y)| ((y.1 - x.1) / x.1.abs().max(1e-12)).abs() * 100.0).fold(0.0, f64::max);
                        if worst < 5.0 {
                            ui.label(RichText::new(format!("✔ Konvergiert: letzte Änderung {:.1} %.", worst)).color(Color32::from_rgb(20, 130, 50)).strong());
                        } else {
                            ui.label(RichText::new(format!("✖ Nicht konvergiert: letzte Änderung {:.1} %. Spannungsspitzen an scharfen Innenkanten konvergieren nie – dort eine Rundung vorsehen oder den Wert in etwas Abstand lesen.", worst)).color(Color32::from_rgb(190, 60, 20)));
                        }
                    }
                }
                ui.add_space(6.0);
                if btn(ui, if rows.is_empty() { "Konvergenzstudie starten" } else { "Erneut starten" }, 210.0, if job.is_none() && study.is_some() { 1 } else { 2 })
                    && job.is_none()
                    && study.is_some()
                {
                    start = true;
                }
            });
        });
        if stop {
            if let Some(j) = &self.apps.conv_job {
                j.cancel();
            }
        }
        if start {
            self.conv_start();
        }
        if let Some(o) = show {
            self.apps_display(o);
        }
        if !open {
            self.apps.conv_open = false;
        }
    }

    // ------------------------------------------------------------ Simulate Ergebnisse

    fn results_window(&mut self, ctx: &egui::Context) {
        // the current live result first, then the saved ones
        let mut list: Vec<(String, Arc<SimOutput>)> = vec![];
        if let Some(o) = self.sim.out.clone().filter(|o| !o.partial && !self.sim.viewing_saved) {
            list.push((format!("Aktuell: {}", self.sim_study().map(|s| s.name.clone()).unwrap_or_default()), o));
        }
        for s in &self.sim.saved {
            list.push((format!("{} ({})", s.name, s.when), s.out.clone()));
        }
        let a = &mut self.apps;
        a.res_a = a.res_a.min(list.len().saturating_sub(1));
        a.res_b = a.res_b.min(list.len().saturating_sub(1));
        let mut show = None;
        let (mut report, mut csv, mut save_cur) = (false, false, false);
        let open = cwin(ctx, "apps_results", "Simulate Ergebnisse", false, pos2(460.0, 150.0), 680.0, |ui| {
            pad(ui, 12.0, |ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                if list.is_empty() {
                    ui.label(
                        RichText::new(
                            "Noch keine Ergebnisse. Eine Studie lösen (Ansys Simulation, Flow Analysis, Spritzgussanalyse, Generatives Design) – jedes Ergebnis erscheint hier.",
                        )
                        .color(dlgkit_dim()),
                    );
                    return;
                }
                egui::Grid::new("res_list").num_columns(4).spacing(vec2(10.0, 4.0)).show(ui, |ui| {
                    ui.label(RichText::new("A").strong());
                    ui.label(RichText::new("B").strong());
                    ui.label(RichText::new("Ergebnis").strong());
                    ui.label("");
                    ui.end_row();
                    for (i, (n, _)) in list.iter().enumerate() {
                        if ui.radio(a.res_a == i, "").clicked() {
                            a.res_a = i;
                        }
                        if ui.radio(a.res_b == i, "").clicked() {
                            a.res_b = i;
                        }
                        ui.label(n);
                        if ui.small_button("Anzeigen").clicked() {
                            show = Some(i);
                        }
                        ui.end_row();
                    }
                });
                hline(ui);
                let (oa, ob) = (&list[a.res_a].1, &list[a.res_b].1);
                let mut keys: Vec<String> = oa.summary.iter().map(|s| s.0.clone()).collect();
                for s in &ob.summary {
                    if !keys.contains(&s.0) {
                        keys.push(s.0.clone());
                    }
                }
                egui::ScrollArea::vertical().max_height(300.0).show(ui, |ui| {
                    egui::Grid::new("res_cmp").striped(true).num_columns(4).spacing(vec2(14.0, 4.0)).show(ui, |ui| {
                        ui.label(RichText::new("Größe").strong());
                        ui.label(RichText::new("A").strong());
                        ui.label(RichText::new("B").strong());
                        ui.label(RichText::new("Δ").strong());
                        ui.end_row();
                        for k in &keys {
                            let va = oa.summary.iter().find(|s| &s.0 == k).map(|s| s.1.clone()).unwrap_or("–".into());
                            let vb = ob.summary.iter().find(|s| &s.0 == k).map(|s| s.1.clone()).unwrap_or("–".into());
                            let d = match (lead_num(&va), lead_num(&vb)) {
                                (Some(x), Some(y)) if a.res_a != a.res_b && x.abs() > 1e-12 => format!("{:+.1} %", (y - x) / x.abs() * 100.0),
                                _ => String::new(),
                            };
                            ui.label(RichText::new(k).color(dlgkit_dim()));
                            ui.label(va);
                            ui.label(vb);
                            ui.label(d);
                            ui.end_row();
                        }
                    });
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if btn(ui, "Bericht von A", 130.0, 0) {
                        report = true;
                    }
                    if btn(ui, "CSV-Export von A", 150.0, 0) {
                        csv = true;
                    }
                    if btn(ui, "Aktuelles Ergebnis speichern", 220.0, 0) {
                        save_cur = true;
                    }
                });
            });
        });
        if let Some(i) = show {
            self.apps_display(list[i].1.clone());
        }
        if report || csv {
            let (n, o) = list[self.apps.res_a].clone();
            if report {
                let mut body = format!("Ergebnis: {}\n\n", n);
                for (k, v) in &o.summary {
                    body += &format!("  {:<40}{}\n", k, v);
                }
                for f in &o.fields {
                    body += &format!("\n  {:<40}{} … {} {}", f.name, apps::num(f.min as f64), apps::num(f.max as f64), f.unit);
                }
                if !o.warnings.is_empty() {
                    body += "\n\nHinweise:\n";
                    for w in &o.warnings {
                        body += &format!("  - {}\n", w);
                    }
                }
                self.apps_report("simulate_ergebnisse.inf", "SIMULATE ERGEBNISSE", body);
            }
            if csv {
                if o.table.1.is_empty() {
                    self.say("Dieses Ergebnis hat keine Knotentabelle.", MsgKind::Warning);
                } else {
                    let mut s = o.table.0.join(";") + "\n";
                    for r in &o.table.1 {
                        s += &r.iter().map(|v| format!("{}", v)).collect::<Vec<_>>().join(";");
                        s += "\n";
                    }
                    self.apps_save_file("ergebnis.csv", s.into_bytes());
                }
            }
        }
        if save_cur {
            if let (Some(o), false) = (self.sim.out.clone(), self.sim.viewing_saved) {
                let n = format!("{} – Analyse {}", self.sim_study().map(|s| s.name.clone()).unwrap_or_default(), self.sim.saved.len() + 1);
                self.apps_show(o, &n);
                self.say("Ergebnis gespeichert.", MsgKind::Success);
            } else {
                self.say("Es gibt kein aktuelles, noch nicht gespeichertes Ergebnis.", MsgKind::Info);
            }
        }
        if !open {
            self.apps.results_open = false;
        }
    }

    /// Text in the Informationsfenster (and the list of analyses).
    pub fn apps_report(&mut self, file: &str, title: &str, body: String) {
        let text = format!("{} FÜR TEIL {}\n================================================\n\n{}", title, self.data.name.to_uppercase(), body);
        self.creo.tools.analyses.push((title.to_string(), text.clone()));
        if self.creo.tools.analyses.len() > 40 {
            self.creo.tools.analyses.remove(0);
        }
        self.creo.tools.info = Some((format!("Informationsfenster ({})", file), text));
    }

    /// Asks where to save `bytes` (default: the part's folder) and writes them.
    pub fn apps_save_file(&mut self, name: &str, bytes: Vec<u8>) {
        let clean: String = self.data.name.to_lowercase().chars().map(|c| if c.is_alphanumeric() { c } else { '_' }).collect();
        let file = format!("{}_{}", clean, name);
        let dir = self.save_dir(self.path.as_ref());
        let target = if cfg!(test) || std::env::var_os("SOLIDWORK_NO_DIALOGS").is_some() {
            Some(dir.join(&file))
        } else {
            rfd::FileDialog::new().set_directory(&dir).set_file_name(&file).save_file()
        };
        let Some(p) = target else { return };
        match std::fs::write(&p, bytes) {
            Ok(()) => self.say(format!("Gespeichert: {}", p.display()), MsgKind::Success),
            Err(e) => self.say(format!("Speichern fehlgeschlagen: {}", e), MsgKind::Error),
        }
    }

    // ------------------------------------------------------------ Mathcad

    fn math_vars(&self) -> crate::model::expr::Vars {
        let mut v = self.result.vars.clone();
        if !self.result.polys.is_empty() {
            let m = export::mass_properties(&self.result, self.data.density);
            v.insert("MASSE".into(), m.mass);
            v.insert("VOLUMEN".into(), m.volume);
            v.insert("OBERFLAECHE".into(), m.area);
            let s = m.bbox.size();
            v.insert("BREITE_X".into(), s.x);
            v.insert("HOEHE_Y".into(), s.y);
            v.insert("TIEFE_Z".into(), s.z);
        }
        v
    }

    pub(super) fn math_insert_template(&mut self, name: &str, text: &str) {
        let ws = &mut self.data.apps.worksheet;
        if !ws.trim().is_empty() {
            ws.push_str("\n\n");
        }
        ws.push_str(&format!("// ---- {}\n", name));
        ws.push_str(text);
        self.dirty = true;
    }

    fn math_window(&mut self, ctx: &egui::Context) {
        let vars = self.math_vars();
        let before = self.data.apps.worksheet.clone();
        let (lines, defined) = worksheet::evaluate(&before, &vars);
        let params: Vec<String> = self.data.parameters.iter().map(|p| p.name.to_uppercase()).collect();
        let mut transfer = false;
        let mut template = None;
        let mut save = false;
        let ws = &mut self.data.apps.worksheet;
        let export = &mut self.apps.math_export;
        let open = cwin(ctx, "apps_math", "Mathcad – Berechnungsblatt", true, pos2(380.0, 120.0), 860.0, |ui| {
            pad(ui, 10.0, |ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                ui.horizontal(|ui| {
                    lbl_dim(ui, "Vorlage einfügen:");
                    for (i, (n, _)) in worksheet::TEMPLATES.iter().enumerate() {
                        if ui.small_button(*n).clicked() {
                            template = Some(i);
                        }
                    }
                });
                let font = egui::FontId::monospace(14.0);
                let row_h = ui.fonts(|f| f.row_height(&font));
                let n_rows = ws.lines().count().max(18) + 2;
                egui::ScrollArea::vertical().max_height(460.0).show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        let te = egui::TextEdit::multiline(ws).font(font.clone()).desired_width(520.0).desired_rows(n_rows).margin(vec2(6.0, 4.0));
                        let resp = ui.add(te);
                        let top = resp.rect.top() + 0.5;
                        let (r, _) = ui.allocate_exact_size(vec2(300.0, resp.rect.height()), Sense::hover());
                        let p = ui.painter_at(r);
                        p.rect_filled(r, 2.0, Color32::from_rgb(248, 249, 250));
                        for (i, l) in lines.iter().enumerate() {
                            let y = top + i as f32 * row_h + row_h * 0.5;
                            let (txt, col) = match (&l.error, l.value, &l.kind) {
                                (Some(e), _, _) => (format!("⚠ {}", e), Color32::from_rgb(196, 40, 30)),
                                (None, Some(v), worksheet::LineKind::Define { name, show }) => {
                                    let s = format!("{} = {} {}", name, worksheet::show(v), l.unit);
                                    (s, if *show { Color32::from_rgb(20, 70, 150) } else { Color32::from_rgb(140, 146, 156) })
                                }
                                (None, Some(v), worksheet::LineKind::Eval) => (format!("= {} {}", worksheet::show(v), l.unit), Color32::from_rgb(20, 70, 150)),
                                _ => continue,
                            };
                            p.text(pos2(r.left() + 8.0, y), egui::Align2::LEFT_CENTER, txt, font.clone(), col);
                        }
                    });
                });
                hline(ui);
                ui.label(RichText::new("Variablen aus dem Modell: alle Parameter sowie MASSE (kg), VOLUMEN (mm³), OBERFLAECHE (mm²), BREITE_X, HOEHE_Y, TIEFE_Z (mm). Funktionen: sin cos tan (Grad), sqrt, abs, min, max, log, ln, exp, pi.").size(12.0).color(dlgkit_dim()));
                if !defined.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        lbl_bold(ui, "An das Modell übergeben:");
                        for k in defined.keys() {
                            let mut on = export.contains(k);
                            let tag = if params.contains(&k.to_uppercase()) { format!("{} ↔", k) } else { k.clone() };
                            if ui.checkbox(&mut on, tag).changed() {
                                if on {
                                    export.insert(k.clone());
                                } else {
                                    export.remove(k);
                                }
                            }
                        }
                    });
                }
                ui.horizontal(|ui| {
                    if btn(ui, "Ausgewählte als Parameter übernehmen", 270.0, if export.is_empty() { 2 } else { 1 }) && !export.is_empty() {
                        transfer = true;
                    }
                    if btn(ui, "Als Text speichern", 150.0, 0) {
                        save = true;
                    }
                });
            });
        });
        if self.data.apps.worksheet != before {
            self.dirty = true;
        }
        if let Some(i) = template {
            let (n, t) = worksheet::TEMPLATES[i];
            self.math_insert_template(n, t);
        }
        if save {
            let mut s = String::new();
            for (raw, l) in self.data.apps.worksheet.lines().zip(&lines) {
                s += raw;
                if let Some(v) = l.value {
                    s += &format!("    → {} {}", worksheet::show(v), l.unit);
                }
                s += "\n";
            }
            self.apps_save_file("berechnung.txt", s.into_bytes());
        }
        if transfer {
            let vals: Vec<(String, f64)> = self.apps.math_export.iter().filter_map(|k| defined.get(k).map(|v| (k.clone(), *v))).collect();
            let n = vals.len();
            self.commit(&format!("{} Wert(e) aus dem Berechnungsblatt als Parameter übernommen.", n), |d| {
                for (k, v) in vals {
                    let txt = format!("{}", (v * 1e9).round() / 1e9);
                    if let Some(p) = d.parameters.iter_mut().find(|p| p.name.eq_ignore_ascii_case(&k)) {
                        p.expr = txt;
                    } else {
                        d.parameters.push(crate::model::features::Parameter { name: k.to_uppercase(), expr: txt, note: "Mathcad".into() });
                    }
                }
            });
        }
        if !open {
            self.apps.math_open = false;
        }
    }

    // ------------------------------------------------------------ EZ Tolerance Analysis

    /// Adds the distance between two selected parallel planes, or a selected cylinder's
    /// diameter, as a chain member with the ISO 2768 general tolerance.
    fn tol_from_selection(&mut self) {
        let topo = self.result.topo.clone();
        let faces: Vec<usize> = self.selection.iter().filter_map(|p| if let Pick::Face(f) = p { Some(*f) } else { None }).collect();
        let class = self.apps.tol_class;
        let item = match faces.as_slice() {
            [a, b] if topo.faces[*a].planar && topo.faces[*b].planar && topo.faces[*a].normal.cross(topo.faces[*b].normal).length() < 0.01 => {
                let (pa, na) = adv::face_anchor(&topo, *a);
                let (pb, _) = adv::face_anchor(&topo, *b);
                let d = (pb - pa).dot(na).abs();
                Some((format!("Abstand F{}–F{}", a, b), d))
            }
            [f] if !topo.faces[*f].planar => topo.surface_axis(*f).map(|(_, _, r)| (format!("⌀ F{}", f), 2.0 * r)),
            _ => None,
        };
        let Some((name, nominal)) = item else {
            self.say("Zwei parallele ebene Flächen (Abstand) oder eine Zylinderfläche (Durchmesser) auswählen.", MsgKind::Prompt);
            return;
        };
        let t = stackup::iso2768(nominal, class);
        let i = self.apps.tol_sel;
        if self.data.apps.stackups.is_empty() {
            self.data.apps.stackups.push(stackup::Stackup::default());
        }
        let i = i.min(self.data.apps.stackups.len() - 1);
        self.data.apps.stackups[i].items.push(stackup::Contributor { name, nominal: (nominal * 1e4).round() / 1e4, upper: t, lower: -t, dir: 1, dist: stackup::Dist::Normal });
        self.dirty = true;
    }

    fn tol_window(&mut self, ctx: &egui::Context) {
        if self.data.apps.stackups.is_empty() {
            self.data.apps.stackups.push(stackup::Stackup::default());
        }
        let a = &mut self.apps;
        let sts = &mut self.data.apps.stackups;
        a.tol_sel = a.tol_sel.min(sts.len() - 1);
        let before = sts.clone();
        let (mut add_sel, mut report, mut remove_stack) = (false, false, false);
        // the analysis, again only when the chain changed
        let key = {
            use std::hash::{Hash, Hasher};
            let mut h = std::collections::hash_map::DefaultHasher::new();
            serde_json::to_string(&sts[a.tol_sel]).unwrap_or_default().hash(&mut h);
            h.finish()
        };
        if a.tol_cache.as_ref().is_none_or(|c| c.0 != key) {
            a.tol_cache = Some((key, stackup::analyze(&sts[a.tol_sel], 200_000, 7)));
        }
        let res = a.tol_cache.as_ref().unwrap().1.clone();
        let open = cwin(ctx, "apps_tol", "EZ Tolerance Analysis", true, pos2(360.0, 120.0), 900.0, |ui| {
            pad(ui, 10.0, |ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                ui.horizontal(|ui| {
                    lbl(ui, "Maßkette");
                    let names: Vec<String> = sts.iter().map(|s| s.name.clone()).collect();
                    let refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
                    combo(ui, "tol_stack", 220.0, &mut a.tol_sel, &refs, true, None);
                    ui.add(egui::TextEdit::singleline(&mut sts[a.tol_sel].name).desired_width(180.0));
                    if ui.button("Neue Kette").clicked() {
                        sts.push(stackup::Stackup { name: format!("Maßkette {}", sts.len() + 1), ..Default::default() });
                        a.tol_sel = sts.len() - 1;
                    }
                    if ui.add_enabled(sts.len() > 1, egui::Button::new("Kette löschen")).clicked() {
                        remove_stack = true;
                    }
                });
                let st = &mut sts[a.tol_sel];
                let mut del = None;
                egui::ScrollArea::vertical().max_height(260.0).id_salt("tol_rows").show(ui, |ui| {
                    egui::Grid::new("tol_grid").striped(true).num_columns(8).spacing(vec2(8.0, 4.0)).show(ui, |ui| {
                        for h in ["Glied", "Richtung", "Nennmaß", "oberes Abmaß", "unteres Abmaß", "Verteilung", "Anteil", ""] {
                            ui.label(RichText::new(h).strong());
                        }
                        ui.end_row();
                        for (i, c) in st.items.iter_mut().enumerate() {
                            ui.add_sized(vec2(160.0, 20.0), egui::TextEdit::singleline(&mut c.name));
                            if ui
                                .button(if c.dir > 0 { "＋ vergrößert" } else { "－ verkleinert" })
                                .on_hover_text("Ob das Glied das Schließmaß vergrößert oder verkleinert")
                                .clicked()
                            {
                                c.dir = -c.dir.signum();
                            }
                            ui.add(egui::DragValue::new(&mut c.nominal).speed(0.05).max_decimals(4));
                            ui.add(egui::DragValue::new(&mut c.upper).speed(0.001).max_decimals(4));
                            ui.add(egui::DragValue::new(&mut c.lower).speed(0.001).max_decimals(4));
                            let mut k = stackup::Dist::ALL.iter().position(|d| *d == c.dist).unwrap_or(0);
                            let names: Vec<&str> = stackup::Dist::ALL.iter().map(|d| d.name()).collect();
                            egui::ComboBox::from_id_salt(("tol_dist", i)).width(130.0).show_index(ui, &mut k, names.len(), |j| names[j]);
                            c.dist = stackup::Dist::ALL[k];
                            if c.upper < c.lower {
                                std::mem::swap(&mut c.upper, &mut c.lower);
                            }
                            let share = res.share.get(i).copied().unwrap_or(0.0);
                            let (cell, _) = ui.allocate_exact_size(vec2(130.0, 14.0), Sense::hover());
                            let r = egui::Rect::from_min_size(cell.min, vec2(84.0, 14.0));
                            ui.painter().rect_filled(r, 2.0, Color32::from_rgb(232, 235, 238));
                            ui.painter().rect_filled(egui::Rect::from_min_size(r.min, vec2(r.width() * share as f32, r.height())), 2.0, Color32::from_rgb(70, 140, 220));
                            ui.painter().text(
                                r.right_center() + vec2(4.0, 0.0),
                                egui::Align2::LEFT_CENTER,
                                format!("{:.0} %", share * 100.0),
                                egui::FontId::proportional(11.5),
                                Color32::from_rgb(60, 64, 70),
                            );
                            if ui.small_button("✕").clicked() {
                                del = Some(i);
                            }
                            ui.end_row();
                        }
                    });
                });
                if let Some(i) = del {
                    st.items.remove(i);
                }
                ui.horizontal(|ui| {
                    if ui.button("Glied hinzufügen").clicked() {
                        st.items.push(stackup::Contributor {
                            name: format!("Maß {}", st.items.len() + 1),
                            nominal: 10.0,
                            upper: 0.1,
                            lower: -0.1,
                            dir: 1,
                            dist: stackup::Dist::Normal,
                        });
                    }
                    if ui.button("Aus Auswahl messen").on_hover_text("Zwei parallele Flächen (Abstand) oder eine Zylinderfläche (Durchmesser) im Modell auswählen").clicked() {
                        add_sel = true;
                    }
                    lbl_dim(ui, "Allgemeintoleranz ISO 2768-1");
                    combo(ui, "tol_class", 90.0, &mut a.tol_class, &["f", "m", "c", "v"], true, None);
                    if ui.button("auf Glieder ohne Toleranz anwenden").clicked() {
                        for c in st.items.iter_mut().filter(|c| c.upper == 0.0 && c.lower == 0.0) {
                            let t = stackup::iso2768(c.nominal, a.tol_class);
                            c.upper = t;
                            c.lower = -t;
                        }
                    }
                });
                ui.horizontal(|ui| {
                    lbl_bold(ui, "Anforderung an das Schließmaß:");
                    let mut has_l = st.lsl.is_some();
                    if ui.checkbox(&mut has_l, "min").changed() {
                        st.lsl = if has_l { Some(res.nominal - 0.2) } else { None };
                    }
                    if let Some(l) = &mut st.lsl {
                        ui.add(egui::DragValue::new(l).speed(0.005).max_decimals(4));
                    }
                    let mut has_u = st.usl.is_some();
                    if ui.checkbox(&mut has_u, "max").changed() {
                        st.usl = if has_u { Some(res.nominal + 0.2) } else { None };
                    }
                    if let Some(u) = &mut st.usl {
                        ui.add(egui::DragValue::new(u).speed(0.005).max_decimals(4));
                    }
                });
                hline(ui);
                if st.items.is_empty() {
                    lbl_dim(ui, "Glieder hinzufügen: das Schließmaß ist die Summe der vergrößernden minus der verkleinernden Maße.");
                    return;
                }
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(400.0);
                        let f = apps::num;
                        let mut rows = vec![
                            ("Nennmaß des Schließmaßes".to_string(), f(res.nominal)),
                            ("Worst Case".to_string(), format!("{} … {}  (± {})", f(res.wc.0), f(res.wc.1), f((res.wc.1 - res.wc.0) / 2.0))),
                            ("Statistisch (RSS, ±3σ)".to_string(), format!("{} … {}  (σ = {})", f(res.rss.0), f(res.rss.1), f(res.sigma))),
                            ("Monte Carlo (200 000)".to_string(), format!("{} … {}  (σ = {})", f(res.mc_range.0), f(res.mc_range.1), f(res.mc_sigma))),
                        ];
                        if let Some(cp) = res.cp {
                            rows.push(("Cp".into(), f(cp)));
                        }
                        if let Some(cpk) = res.cpk {
                            rows.push((
                                "Cpk".into(),
                                format!(
                                    "{}  {}",
                                    f(cpk),
                                    if cpk >= 1.33 {
                                        "✔ fähig"
                                    } else if cpk >= 1.0 {
                                        "⚠ knapp"
                                    } else {
                                        "✖ nicht fähig"
                                    }
                                ),
                            ));
                        }
                        if let (Some(p), Some(m)) = (res.ppm, res.mc_ppm) {
                            rows.push(("Ausschuss (Normalverteilung / MC)".into(), format!("{} / {} ppm", f(p), f(m))));
                        }
                        if let (Some(l), Some(u)) = (st.lsl, st.usl) {
                            let wc_ok = res.wc.0 >= l && res.wc.1 <= u;
                            rows.push(("Worst Case innerhalb der Grenzen".into(), if wc_ok { "ja – 100 % austauschbar".into() } else { "nein".into() }));
                        }
                        kv(ui, "tol_res", &rows);
                        if let Some((i, s)) = res.share.iter().enumerate().max_by(|x, y| x.1.partial_cmp(y.1).unwrap()) {
                            ui.label(
                                RichText::new(format!(
                                    "Größter Beitrag: {} ({:.0} % der Streuung) – hier lohnt sich eine engere Toleranz am meisten.",
                                    st.items[i].name,
                                    s * 100.0
                                ))
                                .size(12.5)
                                .color(dlgkit_dim()),
                            );
                        }
                    });
                    // histogram with worst-case and requirement limits
                    let (r, _) = ui.allocate_exact_size(vec2(440.0, 200.0), Sense::hover());
                    let p = ui.painter_at(r);
                    p.rect_filled(r, 2.0, Color32::WHITE);
                    p.rect_stroke(r, 2.0, Stroke::new(1.0_f32, Color32::from_rgb(200, 204, 210)), egui::StrokeKind::Inside);
                    let (h0, h1) = res.hist_range;
                    let mx = res.hist.iter().copied().max().unwrap_or(1).max(1) as f32;
                    let inner = r.shrink2(vec2(8.0, 18.0));
                    let bw = inner.width() / res.hist.len().max(1) as f32;
                    for (k, n) in res.hist.iter().enumerate() {
                        let hgt = inner.height() * (*n as f32 / mx);
                        let x = inner.left() + k as f32 * bw;
                        let mid = h0 + (h1 - h0) * (k as f64 + 0.5) / res.hist.len() as f64;
                        let bad = st.lsl.is_some_and(|l| mid < l) || st.usl.is_some_and(|u| mid > u);
                        p.rect_filled(
                            egui::Rect::from_min_max(pos2(x + 0.5, inner.bottom() - hgt), pos2(x + bw - 0.5, inner.bottom())),
                            0.0,
                            if bad { Color32::from_rgb(220, 70, 60) } else { Color32::from_rgb(90, 150, 220) },
                        );
                    }
                    let xat = |v: f64| inner.left() + ((v - h0) / (h1 - h0).max(1e-12)) as f32 * inner.width();
                    for (v, c, t) in [(res.wc.0, Color32::from_rgb(120, 120, 120), "WC"), (res.wc.1, Color32::from_rgb(120, 120, 120), "WC")] {
                        p.line_segment([pos2(xat(v), inner.top()), pos2(xat(v), inner.bottom())], Stroke::new(1.0_f32, c));
                        p.text(pos2(xat(v), r.top() + 8.0), egui::Align2::CENTER_CENTER, t, egui::FontId::proportional(10.5), c);
                    }
                    for (v, t) in [(st.lsl, "min"), (st.usl, "max")] {
                        if let Some(v) = v {
                            let c = Color32::from_rgb(200, 40, 30);
                            p.line_segment([pos2(xat(v), inner.top()), pos2(xat(v), inner.bottom())], Stroke::new(1.6_f32, c));
                            p.text(pos2(xat(v), r.bottom() - 8.0), egui::Align2::CENTER_CENTER, t, egui::FontId::proportional(10.5), c);
                        }
                    }
                    p.text(pos2(inner.left(), r.bottom() - 8.0), egui::Align2::LEFT_CENTER, apps::num(h0), egui::FontId::proportional(10.5), Color32::DARK_GRAY);
                    p.text(pos2(inner.right(), r.bottom() - 8.0), egui::Align2::RIGHT_CENTER, apps::num(h1), egui::FontId::proportional(10.5), Color32::DARK_GRAY);
                });
                ui.horizontal(|ui| {
                    if btn(ui, "Bericht", 100.0, 0) {
                        report = true;
                    }
                });
            });
        });
        if remove_stack && self.data.apps.stackups.len() > 1 {
            self.data.apps.stackups.remove(self.apps.tol_sel);
            self.apps.tol_sel = 0;
        }
        if self.data.apps.stackups != before {
            self.dirty = true;
        }
        if add_sel {
            self.tol_from_selection();
        }
        if report {
            let st = self.data.apps.stackups[self.apps.tol_sel].clone();
            let r = stackup::analyze(&st, 200_000, 7);
            let f = apps::num;
            let mut body = format!("Maßkette: {}\n\n  {:<26}{:>4}{:>12}{:>10}{:>10}  {:<16}{:>8}\n", st.name, "Glied", "", "Nennmaß", "oben", "unten", "Verteilung", "Anteil");
            for (i, c) in st.items.iter().enumerate() {
                body += &format!(
                    "  {:<26}{:>4}{:>12}{:>10}{:>10}  {:<16}{:>7.1} %\n",
                    c.name,
                    if c.dir > 0 { "+" } else { "−" },
                    f(c.nominal),
                    f(c.upper),
                    f(c.lower),
                    c.dist.name(),
                    r.share.get(i).unwrap_or(&0.0) * 100.0
                );
            }
            body += &format!(
                "\nNennmaß Schließmaß:   {}\nWorst Case:           {} … {}\nRSS (±3σ):            {} … {}\nMonte Carlo:          {} … {} (σ = {})\n",
                f(r.nominal),
                f(r.wc.0),
                f(r.wc.1),
                f(r.rss.0),
                f(r.rss.1),
                f(r.mc_range.0),
                f(r.mc_range.1),
                f(r.mc_sigma)
            );
            if let Some(cpk) = r.cpk {
                body += &format!("Cp / Cpk:             {} / {}\n", r.cp.map(f).unwrap_or("–".into()), f(cpk));
            }
            if let Some(p) = r.ppm {
                body += &format!("Ausschuss:            {} ppm\n", f(p));
            }
            self.apps_report("toleranzanalyse.inf", "TOLERANZANALYSE", body);
        }
        if !open {
            self.apps.tol_open = false;
        }
    }
}
