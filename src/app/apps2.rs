//! Dialogs of the Anwendungen tab, part 2: Schweißen, Spritzguss/Guss, Expert Moldbase,
//! Spritzgussanalyse, Generatives Design, GD&T Advisor and Render Studio.

use super::apps::{dlgkit_dim, kv, progress_row, warn_rows, FillShown, Job};
use super::dlgkit::{btn, combo, cwin, hline, lbl, lbl_bold, lbl_dim, pad};
use super::*;
use crate::apps::{self, gdt as adv, mold, moldflow, raytrace, topopt, weld};
use crate::model::features::{ARef, Annot3, AnnotKind};
use crate::model::sim::StudyKind;
use eframe::egui::{self, pos2, vec2, Color32, RichText, Sense, Stroke};
use std::sync::Arc;

const DIRS: [(&str, V3); 6] = [("+Y (OBEN)", V3::Y), ("−Y", V3::NEG_Y), ("+X", V3::X), ("−X", V3::NEG_X), ("+Z", V3::Z), ("−Z", V3::NEG_Z)];

/// The last mould check, for the dialog, the parting plane and the mould base.
pub struct MoldShown {
    pub generation: u64,
    pub dir: V3,
    pub dir_name: String,
    pub material: mold::MoldMaterial,
    pub check: mold::MoldCheck,
    pub volume: f64,
    pub wall: Option<(f64, f64)>,
    pub bbox: BBox,
}

impl MoldShown {
    /// Outline of the parting plane and an arrow along the pull direction.
    pub fn plane_lines(&self) -> Vec<(V3, V3)> {
        let (e1, e2) = mold::frame(self.dir);
        let c = self.bbox.center();
        let c = c + self.dir * (self.check.parting - c.dot(self.dir));
        let (a, b) = (self.check.extent.x * 0.65 + 5.0, self.check.extent.y * 0.65 + 5.0);
        let q = [c + e1 * a + e2 * b, c - e1 * a + e2 * b, c - e1 * a - e2 * b, c + e1 * a - e2 * b];
        let mut v: Vec<(V3, V3)> = (0..4).map(|i| (q[i], q[(i + 1) % 4])).collect();
        let tip = c + self.dir * (self.check.extent.z * 0.5 + self.check.depth_cavity + 10.0);
        v.push((c, tip));
        let s = self.check.extent.z.max(10.0) * 0.12;
        v.push((tip, tip - self.dir * s + e1 * s * 0.4));
        v.push((tip, tip - self.dir * s - e1 * s * 0.4));
        v
    }
}

impl App {
    // ------------------------------------------------------------ Schweißen

    fn weld_new_from_selection(&mut self) {
        let topo = self.result.topo.clone();
        let edges: Vec<crate::kernel::topology::EdgeRef> = self.selection.iter().filter_map(|p| if let Pick::Edge(e) = p { Some(topo.edge_ref(*e)) } else { None }).collect();
        if edges.is_empty() {
            self.say("Schweißen: zuerst die Kanten der Naht im Modell auswählen (Strg für mehrere).", MsgKind::Prompt);
            return;
        }
        let a = &self.apps;
        let kind = weld::WeldKind::ALL[a.weld_kind.min(3)];
        let id = self.data.apps.welds.iter().map(|w| w.id).max().unwrap_or(0) + 1;
        let seam = weld::WeldSeam { id, name: format!("Naht {}", id), kind, a: a.weld_a, both_sides: a.weld_both, intermittent: None, edges };
        let n = self.data.apps.welds.len();
        self.commit(&format!("{} ({} a{}) erzeugt.", seam.name, kind.name(), apps::num(seam.a)), |d| d.apps.welds.push(seam));
        self.apps.weld_sel = Some(n);
        self.selection.clear();
    }

    pub fn weld_window(&mut self, ctx: &egui::Context) {
        let topo = self.result.topo.clone();
        let edges_sel = self.selection.iter().filter(|p| matches!(p, Pick::Edge(_))).count();
        let (mut create, mut delete, mut report) = (false, None, false);
        let mut select_edges: Option<usize> = None;
        let before = self.data.apps.welds.clone();
        let a = &mut self.apps;
        let welds = &mut self.data.apps.welds;
        let mat = weld::MATERIALS[a.weld_mat.min(weld::MATERIALS.len() - 1)];
        let prc = weld::PROCESSES[a.weld_proc.min(weld::PROCESSES.len() - 1)];
        let calcs: Vec<weld::SeamCalc> = welds.iter().map(|w| weld::calc(w, &topo, &mat, &prc, a.weld_plate)).collect();
        let open = cwin(ctx, "apps_weld", "Schweißen", true, pos2(400.0, 130.0), 760.0, |ui| {
            pad(ui, 10.0, |ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                lbl_bold(ui, "Neue Naht");
                ui.horizontal(|ui| {
                    let kinds: Vec<&str> = weld::WeldKind::ALL.iter().map(|k| k.name()).collect();
                    combo(ui, "weld_kind", 130.0, &mut a.weld_kind, &kinds, true, None);
                    lbl(ui, if a.weld_kind == 0 { "a-Maß" } else { "Blechdicke" });
                    ui.add(egui::DragValue::new(&mut a.weld_a).speed(0.1).range(0.5..=60.0).suffix(" mm"));
                    ui.checkbox(&mut a.weld_both, "beidseitig");
                    if btn(ui, &format!("Aus {} gewählten Kante(n)", edges_sel), 210.0, if edges_sel > 0 { 1 } else { 2 }) {
                        create = true;
                    }
                });
                hline(ui);
                if welds.is_empty() {
                    lbl_dim(ui, "Noch keine Naht: Kanten im Modell auswählen und „Aus gewählten Kanten“ klicken.");
                } else {
                    egui::Grid::new("weld_tab").striped(true).num_columns(9).spacing(vec2(10.0, 4.0)).show(ui, |ui| {
                        for h in ["Naht", "Art", "a / t", "beids.", "Unterbrochen", "Länge", "Zusatz", "Zeit", ""] {
                            ui.label(RichText::new(h).strong());
                        }
                        ui.end_row();
                        for (i, w) in welds.iter_mut().enumerate() {
                            let c = &calcs[i];
                            if ui.selectable_label(a.weld_sel == Some(i), &w.name).clicked() {
                                a.weld_sel = Some(i);
                                select_edges = Some(i);
                            }
                            let mut k = weld::WeldKind::ALL.iter().position(|x| *x == w.kind).unwrap_or(0);
                            let kinds: Vec<&str> = weld::WeldKind::ALL.iter().map(|k| k.name()).collect();
                            egui::ComboBox::from_id_salt(("wk", w.id)).width(100.0).show_index(ui, &mut k, kinds.len(), |j| kinds[j]);
                            w.kind = weld::WeldKind::ALL[k];
                            ui.add(egui::DragValue::new(&mut w.a).speed(0.1).range(0.5..=60.0).suffix(" mm"));
                            ui.checkbox(&mut w.both_sides, "");
                            ui.horizontal(|ui| {
                                let mut on = w.intermittent.is_some();
                                if ui.checkbox(&mut on, "").changed() {
                                    w.intermittent = if on { Some((50.0, 100.0)) } else { None };
                                }
                                if let Some((s, p)) = &mut w.intermittent {
                                    ui.add(egui::DragValue::new(s).speed(1.0).range(5.0..=1000.0).suffix(" /"));
                                    ui.add(egui::DragValue::new(p).speed(1.0).range(10.0..=2000.0).suffix(" mm"));
                                    if *p <= *s {
                                        *p = *s + 1.0;
                                    }
                                }
                            });
                            ui.label(format!("{} mm", apps::num(c.weld_length)));
                            ui.label(format!("{} kg", apps::num(c.filler)));
                            ui.label(format!("{} min", apps::num(c.hours * 60.0)));
                            if ui.small_button("✕").clicked() {
                                delete = Some(i);
                            }
                            ui.end_row();
                        }
                    });
                }
                hline(ui);
                ui.horizontal(|ui| {
                    lbl(ui, "Verfahren");
                    let names: Vec<&str> = weld::PROCESSES.iter().map(|p| p.name).collect();
                    combo(ui, "weld_proc", 180.0, &mut a.weld_proc, &names, true, None);
                    lbl(ui, "Werkstoff");
                    let names: Vec<&str> = weld::MATERIALS.iter().map(|p| p.name).collect();
                    combo(ui, "weld_mat", 170.0, &mut a.weld_mat, &names, true, None);
                });
                ui.horizontal(|ui| {
                    lbl(ui, "Dünnstes Blech");
                    ui.add(egui::DragValue::new(&mut a.weld_plate).speed(0.1).range(0.5..=200.0).suffix(" mm"));
                    lbl(ui, "Last auf alle Nähte");
                    ui.add(egui::DragValue::new(&mut a.weld_force).speed(50.0).range(0.0..=1e8).suffix(" N"));
                    lbl(ui, "Stundensatz");
                    ui.add(egui::DragValue::new(&mut a.weld_rate).speed(1.0).range(0.0..=1000.0).suffix(" €/h"));
                });
                if !welds.is_empty() {
                    let len: f64 = calcs.iter().map(|c| c.weld_length).sum();
                    let filler: f64 = calcs.iter().map(|c| c.filler).sum();
                    let hours: f64 = calcs.iter().map(|c| c.hours).sum();
                    let throat: f64 = calcs.iter().map(|c| c.throat_area).sum();
                    let (tau, fvw, eta) = weld::strength(throat, a.weld_force, &mat);
                    let cost = hours * a.weld_rate + filler * prc.filler_cost;
                    kv(
                        ui,
                        "weld_sum",
                        &[
                            ("Nahtlänge gesamt".into(), format!("{} mm", apps::num(len))),
                            ("Zusatzwerkstoff".into(), format!("{} kg ({} kg Nahtgut)", apps::num(filler), apps::num(calcs.iter().map(|c| c.mass).sum::<f64>()))),
                            ("Schweißzeit".into(), format!("{} min (Lichtbogenzeitanteil {:.0} %)", apps::num(hours * 60.0), prc.duty * 100.0)),
                            ("Kosten".into(), format!("{} € (Lohn {} €, Draht {} €)", apps::num(cost), apps::num(hours * a.weld_rate), apps::num(filler * prc.filler_cost))),
                            ("Nahtspannung τ_w = F / Σ a·l".into(), format!("{} MPa", apps::num(tau))),
                            (
                                "Grenzspannung f_vw,d (EN 1993-1-8)".into(),
                                format!("{} MPa (f_u {} MPa, β_w {}, γ_M2 1,25)", apps::num(fvw), apps::num(mat.fu), apps::num(mat.beta)),
                            ),
                        ],
                    );
                    let col = if eta <= 1.0 { Color32::from_rgb(20, 130, 50) } else { Color32::from_rgb(200, 40, 30) };
                    ui.label(
                        RichText::new(format!(
                            "Ausnutzung η = {:.0} %  {}",
                            eta * 100.0,
                            if eta <= 1.0 { "✔ Nachweis erbracht" } else { "✖ Nachweis nicht erbracht – a-Maß oder Nahtlänge erhöhen" }
                        ))
                        .color(col)
                        .strong(),
                    );
                    let notes: Vec<String> = calcs.iter().flat_map(|c| c.notes.clone()).collect();
                    warn_rows(ui, &notes);
                }
                ui.horizontal(|ui| {
                    if btn(ui, "Bericht", 100.0, 0) {
                        report = true;
                    }
                });
            });
        });
        if let Some(i) = select_edges {
            let w = &self.data.apps.welds[i];
            self.selection = w.chains(&topo).into_iter().map(Pick::Edge).collect();
        }
        if self.data.apps.welds != before {
            self.dirty = true;
        }
        if create {
            self.weld_new_from_selection();
        }
        if let Some(i) = delete {
            let n = self.data.apps.welds[i].name.clone();
            self.commit(&format!("{} gelöscht.", n), |d| {
                d.apps.welds.remove(i);
            });
            self.apps.weld_sel = None;
        }
        if report {
            let mut body = format!(
                "Verfahren: {}\nWerkstoff: {}\n\n  {:<12}{:<12}{:>8}{:>12}{:>12}{:>12}\n",
                prc.name, mat.name, "Naht", "Art", "a [mm]", "Länge [mm]", "Draht [kg]", "Zeit [min]"
            );
            for (w, c) in self.data.apps.welds.iter().zip(&calcs) {
                body += &format!(
                    "  {:<12}{:<12}{:>8}{:>12}{:>12}{:>12}\n",
                    w.name,
                    w.kind.name(),
                    apps::num(w.a),
                    apps::num(c.weld_length),
                    apps::num(c.filler),
                    apps::num(c.hours * 60.0)
                );
            }
            let throat: f64 = calcs.iter().map(|c| c.throat_area).sum();
            let (tau, fvw, eta) = weld::strength(throat, self.apps.weld_force, &mat);
            body += &format!("\nLast {} N: τ_w = {} MPa, f_vw,d = {} MPa, Ausnutzung {:.0} %\n", apps::num(self.apps.weld_force), apps::num(tau), apps::num(fvw), eta * 100.0);
            for c in &calcs {
                for n in &c.notes {
                    body += &format!("Hinweis: {}\n", n);
                }
            }
            self.apps_report("schweissen.inf", "SCHWEISSNÄHTE", body);
        }
        if !open {
            self.apps.weld_open = false;
            self.apps.weld_sel = None;
        }
    }

    // ------------------------------------------------------------ Spritzguss/Guss

    fn mold_dir(&self) -> (String, V3) {
        DIRS.get(self.apps.mold_dir).map(|d| (d.0.to_string(), d.1)).unwrap_or(("+Y (OBEN)".into(), V3::Y))
    }

    pub fn mold_analyze(&mut self) {
        if self.result.topo.polys.is_empty() {
            return;
        }
        let (name, d) = self.mold_dir();
        let m = mold::MATERIALS[self.apps.mold_mat.min(mold::MATERIALS.len() - 1)];
        let topo = self.result.topo.clone();
        let check = mold::check(&topo, d, self.apps.mold_draft);
        // the same classes painted on the displayed polygons
        let s_min = self.apps.mold_draft.to_radians().sin();
        let diag = topo.bbox.diagonal().max(1e-6);
        let polys = self.result.polys.clone();
        let bins: Vec<Option<usize>> = polys
            .iter()
            .map(|p| {
                let n = p.plane.n;
                let c = n.dot(d);
                let cen = p.v.iter().map(|v| v.p).sum::<V3>() / p.v.len().max(1) as f64;
                let cls = if c.abs() < 1e-3 {
                    mold::DraftClass::None
                } else if topo.ray_first_hit(cen + n * (diag * 1e-5), d * c.signum(), diag * 1e-6).is_some() {
                    mold::DraftClass::Undercut
                } else if c.abs() < s_min {
                    mold::DraftClass::Low
                } else if c > 0.0 {
                    mold::DraftClass::Cavity
                } else {
                    mold::DraftClass::Core
                };
                mold::DraftClass::ALL.iter().position(|x| *x == cls)
            })
            .collect();
        let legend = mold::DraftClass::ALL.iter().map(|c| (c.label().to_string(), c.color())).collect();
        self.paint(&format!("Entformung {} – Mindestschräge {}°", name, apps::num(self.apps.mold_draft)), legend, &polys, &bins);
        let mp = export::mass_properties(&self.result, self.data.density);
        let wall = moldflow::wall_range(&topo, 40_000);
        self.apps.mold_res = Some(MoldShown { generation: self.result.generation, dir: d, dir_name: name, material: m, check, volume: mp.volume, wall, bbox: topo.bbox });
    }

    pub fn mold_window(&mut self, ctx: &egui::Context) {
        if self.apps.mold_res.as_ref().is_some_and(|m| m.generation != self.result.generation) {
            self.mold_analyze();
        }
        let (mut redo, mut sel_under, mut sel_low, mut add_draft, mut to_base, mut report) = (false, false, false, false, false, false);
        let a = &mut self.apps;
        let open = cwin(ctx, "apps_mold", "Spritzguss/Guss – Entformbarkeit", true, pos2(420.0, 130.0), 620.0, |ui| {
            pad(ui, 10.0, |ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                ui.horizontal(|ui| {
                    lbl(ui, "Entformrichtung");
                    let names: Vec<&str> = DIRS.iter().map(|d| d.0).collect();
                    redo |= combo(ui, "mold_dir", 120.0, &mut a.mold_dir, &names, true, None);
                    lbl(ui, "Werkstoff");
                    let names: Vec<&str> = mold::MATERIALS.iter().map(|m| m.name).collect();
                    if combo(ui, "mold_mat", 230.0, &mut a.mold_mat, &names, true, None) {
                        a.mold_draft = mold::MATERIALS[a.mold_mat].draft;
                        redo = true;
                    }
                });
                ui.horizontal(|ui| {
                    lbl(ui, "Mindestschräge");
                    redo |= ui.add(egui::DragValue::new(&mut a.mold_draft).speed(0.05).range(0.1..=15.0).suffix(" °")).changed();
                    lbl(ui, "Kavitäten");
                    let mut k = [1usize, 2, 4, 8].iter().position(|c| *c == a.mold_cav).unwrap_or(0);
                    if combo(ui, "mold_cav", 70.0, &mut k, &["1", "2", "4", "8"], true, None) {
                        a.mold_cav = [1, 2, 4, 8][k];
                    }
                });
                let Some(r) = &a.mold_res else {
                    lbl_dim(ui, "Kein Volumenmodell.");
                    return;
                };
                let c = &r.check;
                let m = &r.material;
                hline(ui);
                let total: f64 = c.area.iter().sum::<f64>().max(1e-9);
                for (i, cls) in mold::DraftClass::ALL.iter().enumerate() {
                    ui.horizontal(|ui| {
                        let (sw, _) = ui.allocate_exact_size(vec2(16.0, 14.0), Sense::hover());
                        let col = cls.color();
                        ui.painter().rect_filled(sw, 1.0, Color32::from_rgb((col[0] * 255.0) as u8, (col[1] * 255.0) as u8, (col[2] * 255.0) as u8));
                        ui.label(format!("{:<40}", cls.label()));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(format!("{} mm²  ({:.1} %)", apps::num(c.area[i]), c.area[i] / total * 100.0));
                        });
                    });
                }
                let s = m.shrink / 100.0;
                let clamp_kn = c.projected * m.cavity_mpa / 1000.0 * 1.2;
                let mass_g = r.volume * 1e-9 * m.rho * 1000.0;
                let mut rows = vec![
                    ("Projizierte Fläche".to_string(), format!("{} cm²", apps::num(c.projected / 100.0))),
                    ("Schließkraft (+20 % Reserve)".to_string(), format!("{} kN je Kavität, {} kN gesamt", apps::num(clamp_kn), apps::num(clamp_kn * a.mold_cav as f64))),
                    ("Teilungsebene".to_string(), format!("bei {} mm in Entformrichtung (blau im Modell)", apps::num(c.parting))),
                    ("Formtiefe Düsen- / Auswerferseite".to_string(), format!("{} / {} mm", apps::num(c.depth_cavity), apps::num(c.depth_core))),
                    ("Teilegewicht".to_string(), format!("{} g ({} cm³)", apps::num(mass_g), apps::num(r.volume / 1000.0))),
                    ("Schwindung".to_string(), format!("{} % → Werkzeugmaße × {}", apps::num(m.shrink), apps::num(1.0 + s))),
                    (
                        "Kavität (Teil + Schwindung)".to_string(),
                        format!("{} × {} × {} mm", apps::num(c.extent.x * (1.0 + s)), apps::num(c.extent.y * (1.0 + s)), apps::num(c.extent.z * (1.0 + s))),
                    ),
                ];
                if let Some((lo, hi)) = r.wall {
                    rows.push(("Wanddicke min / max".into(), format!("{} / {} mm (empfohlen {}–{} mm)", apps::num(lo), apps::num(hi), apps::num(m.wall.0), apps::num(m.wall.1))));
                    if m.process != mold::Process::Sand {
                        rows.push(("Kühlzeit (dickste Stelle)".into(), format!("{} s", apps::num(mold::cooling_time(m, hi)))));
                    }
                }
                kv(ui, "mold_rows", &rows);
                let mut w = vec![];
                if c.area[4] > 0.0 {
                    w.push(format!("{} Fläche(n) mit Hinterschnitt: Schieber, Kernzug oder eine andere Entformrichtung nötig.", c.undercut_faces.len().max(1)));
                }
                if c.area[2] + c.area[3] > total * 0.001 {
                    w.push(format!("{} Fläche(n) mit zu wenig Schräge: Zugschräge ≥ {}° vorsehen.", c.bad_faces.len(), apps::num(a.mold_draft)));
                }
                if let Some((lo, hi)) = r.wall {
                    if lo < m.wall.0 * 0.8 {
                        w.push(format!("Dünnste Wand {} mm: unter {} mm füllt {} schlecht.", apps::num(lo), apps::num(m.wall.0), m.name));
                    }
                    if hi > m.wall.1 * 1.5 {
                        w.push(format!("Masseanhäufung {} mm: Einfallstellen und lange Kühlzeit – Wand ausräumen.", apps::num(hi)));
                    }
                }
                if w.is_empty() {
                    ui.label(RichText::new("✔ Das Teil ist in dieser Richtung ohne Schieber entformbar.").color(Color32::from_rgb(20, 130, 50)).strong());
                }
                warn_rows(ui, &w);
                ui.horizontal_wrapped(|ui| {
                    if btn(ui, "Neu analysieren", 130.0, 0) {
                        redo = true;
                    }
                    if btn(ui, "Hinterschnitte auswählen", 190.0, if c.undercut_faces.is_empty() { 2 } else { 0 }) && !c.undercut_faces.is_empty() {
                        sel_under = true;
                    }
                    if btn(ui, "Flächen ohne Schräge auswählen", 230.0, if c.bad_faces.is_empty() { 2 } else { 0 }) && !c.bad_faces.is_empty() {
                        sel_low = true;
                    }
                });
                ui.horizontal_wrapped(|ui| {
                    if btn(ui, "Zugschräge auf Auswahl", 190.0, 0) {
                        add_draft = true;
                    }
                    if btn(ui, "Formaufbau (Expert Moldbase)", 230.0, 1) {
                        to_base = true;
                    }
                    if btn(ui, "Bericht", 90.0, 0) {
                        report = true;
                    }
                });
            });
        });
        if redo {
            self.mold_analyze();
        }
        if let Some(r) = &self.apps.mold_res {
            if sel_under {
                self.selection = r.check.undercut_faces.iter().map(|f| Pick::Face(*f)).collect();
            }
            if sel_low {
                self.selection = r.check.bad_faces.iter().map(|f| Pick::Face(*f)).collect();
                self.say(format!("{} Fläche(n) ausgewählt: mit „Zugschräge auf Auswahl“ eine Schräge hinzufügen.", r.check.bad_faces.len()), MsgKind::Info);
            }
        }
        if add_draft {
            if !self.selection.iter().any(|p| matches!(p, Pick::Face(_))) {
                self.say("Zuerst Flächen auswählen (z. B. „Flächen ohne Schräge auswählen“).", MsgKind::Prompt);
            } else {
                self.tab = Tab::Model;
                self.start_feature(crate::model::features::FeatureKind::new_draft());
            }
        }
        if to_base {
            self.open_moldbase();
        }
        if report {
            self.mold_report();
        }
        if !open {
            self.apps.mold_open = false;
            self.creo.tools.overlay = None;
        }
    }

    fn mold_report(&mut self) {
        let Some(r) = &self.apps.mold_res else { return };
        let c = &r.check;
        let m = &r.material;
        let mut body =
            format!("Entformrichtung: {}\nWerkstoff: {} (Schwindung {} %)\nMindestschräge: {}°\n\n", r.dir_name, m.name, apps::num(m.shrink), apps::num(self.apps.mold_draft));
        for (i, cls) in mold::DraftClass::ALL.iter().enumerate() {
            body += &format!("  {:<42}{:>14} mm²\n", cls.label(), apps::num(c.area[i]));
        }
        body += &format!(
            "\nProjizierte Fläche:      {} cm²\nSchließkraft:            {} kN je Kavität\nTeilungsebene:           {} mm\nFormtiefe Düse/Auswerfer: {} / {} mm\nVolumen:                 {} cm³\n",
            apps::num(c.projected / 100.0),
            apps::num(c.projected * m.cavity_mpa / 1000.0 * 1.2),
            apps::num(c.parting),
            apps::num(c.depth_cavity),
            apps::num(c.depth_core),
            apps::num(r.volume / 1000.0)
        );
        if let Some((lo, hi)) = r.wall {
            body += &format!("Wanddicke:               {} … {} mm\nKühlzeit:                {} s\n", apps::num(lo), apps::num(hi), apps::num(mold::cooling_time(m, hi)));
        }
        self.apps_report("spritzguss.inf", "ENTFORMBARKEIT", body);
    }

    // ------------------------------------------------------------ Expert Moldbase

    pub fn open_moldbase(&mut self) {
        if self.apps.mold_res.as_ref().is_none_or(|m| m.generation != self.result.generation) {
            let overlay = self.creo.tools.overlay.take();
            self.mold_analyze();
            // the base alone does not need the draft colours
            if !self.apps.mold_open {
                self.creo.tools.overlay = overlay;
            }
        }
        self.apps.base_open = true;
    }

    fn moldbase(&self) -> Option<mold::MoldBase> {
        let r = self.apps.mold_res.as_ref()?;
        let c = &r.check;
        let kn = c.projected * r.material.cavity_mpa / 1000.0 * 1.2;
        Some(mold::mold_base(c.extent, c.depth_cavity, c.depth_core, self.apps.mold_cav, r.material.shrink, kn))
    }

    pub fn moldbase_window(&mut self, ctx: &egui::Context) {
        let base = self.moldbase();
        let (mut csv, mut report) = (false, false);
        let a = &mut self.apps;
        let material = a.mold_res.as_ref().map(|r| r.material.name).unwrap_or("–");
        let open = cwin(ctx, "apps_base", "Expert Moldbase – Formaufbau", true, pos2(460.0, 110.0), 860.0, |ui| {
            pad(ui, 10.0, |ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                let Some(b) = &base else {
                    lbl_dim(ui, "Kein Volumenmodell.");
                    return;
                };
                ui.horizontal(|ui| {
                    lbl(ui, "Kavitäten");
                    let mut k = [1usize, 2, 4, 8].iter().position(|c| *c == a.mold_cav).unwrap_or(0);
                    if combo(ui, "base_cav", 70.0, &mut k, &["1", "2", "4", "8"], true, None) {
                        a.mold_cav = [1, 2, 4, 8][k];
                    }
                    lbl_dim(ui, &format!("Anordnung {} × {}, Werkstoff {}, Entformrichtung aus Spritzguss/Guss", b.layout.0, b.layout.1, material));
                });
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(450.0);
                        egui::Grid::new("base_plates").striped(true).num_columns(4).spacing(vec2(12.0, 4.0)).show(ui, |ui| {
                            for h in ["Platte", "Größe [mm]", "Dicke", "Werkstoff"] {
                                ui.label(RichText::new(h).strong());
                            }
                            ui.end_row();
                            for p in &b.plates {
                                ui.label(p.name);
                                ui.label(format!("{} × {}", apps::num(p.size.0), apps::num(p.size.1)));
                                ui.label(format!("{} mm", apps::num(p.thickness)));
                                ui.label(p.material);
                                ui.end_row();
                            }
                        });
                        ui.add_space(4.0);
                        let mut rows = vec![
                            (
                                "Formeinsatz (je Hälfte)".to_string(),
                                format!("{} × {} mm, Höhe {} / {} mm", apps::num(b.insert.0), apps::num(b.insert.1), apps::num(b.insert.2), apps::num(b.insert.3)),
                            ),
                            ("Führungssäulen".to_string(), format!("4 × ⌀{} mm mit Führungsbuchsen", apps::num(b.guide_d))),
                            ("Verschraubung".to_string(), format!("{} (ISO 4762, 12.9)", b.screw)),
                            ("Auswerferhub".to_string(), format!("{} mm", apps::num(b.ejector_stroke))),
                            ("Werkzeughöhe".to_string(), format!("{} mm", apps::num(b.height))),
                            ("Öffnungsweg".to_string(), format!("{} mm", apps::num(b.opening))),
                            ("Stahlgewicht".to_string(), format!("{} kg", apps::num(b.steel_kg))),
                            ("Schließkraft".to_string(), format!("{} kN", apps::num(b.clamp_kn))),
                        ];
                        rows.push((
                            "Maschine".into(),
                            match &b.machine {
                                Some(m) => format!(
                                    "{} kN, Holmabstand {} mm, Werkzeughöhe {}–{} mm",
                                    apps::num(m.kn),
                                    apps::num(m.tie_bar),
                                    apps::num(m.min_height),
                                    apps::num(m.max_height)
                                ),
                                None => "keine aus der Standardreihe".into(),
                            },
                        ));
                        kv(ui, "base_rows", &rows);
                        warn_rows(ui, &b.notes);
                    });
                    // schematic section: plates from the fixed half (top) to the moving half (bottom)
                    let (r, _) = ui.allocate_exact_size(vec2(360.0, 440.0), Sense::hover());
                    let p = ui.painter_at(r);
                    p.rect_filled(r, 2.0, Color32::WHITE);
                    let w_long = b.plates[1].size.0;
                    let sc = ((r.width() - 30.0) as f64 / w_long).min((r.height() - 30.0) as f64 / (b.height + 20.0)) as f32;
                    let cx = r.center().x;
                    let mut y = r.top() + 15.0;
                    let steel = Color32::from_rgb(176, 186, 198);
                    let edge = Stroke::new(1.0_f32, Color32::from_rgb(70, 78, 90));
                    for (i, pl) in b.plates.iter().enumerate() {
                        if i == 5 || i == 6 {
                            continue; // ejector plates are drawn inside the rails
                        }
                        let h = pl.thickness as f32 * sc;
                        let wpx = pl.size.0 as f32 * sc;
                        if pl.name.starts_with("Leisten") {
                            let rw = pl.size.1 as f32 * sc;
                            let full = b.plates[1].size.0 as f32 * sc;
                            for x0 in [cx - full / 2.0, cx + full / 2.0 - rw] {
                                let rr = egui::Rect::from_min_size(pos2(x0, y), vec2(rw, h));
                                p.rect_filled(rr, 0.0, steel);
                                p.rect_stroke(rr, 0.0, edge, egui::StrokeKind::Inside);
                            }
                            // ejector plates between the rails, retracted
                            let ew = b.plates[5].size.0 as f32 * sc;
                            let (h5, h6) = (b.plates[5].thickness as f32 * sc, b.plates[6].thickness as f32 * sc);
                            let ey = y + h - h5 - h6;
                            for (yy, hh) in [(ey, h5), (ey + h5, h6)] {
                                let rr = egui::Rect::from_min_size(pos2(cx - ew / 2.0, yy), vec2(ew, hh));
                                p.rect_filled(rr, 0.0, Color32::from_rgb(150, 190, 150));
                                p.rect_stroke(rr, 0.0, edge, egui::StrokeKind::Inside);
                            }
                            // ejector pins up to the part
                            for k in [-1.0f32, 1.0] {
                                let x = cx + k * (b.insert.0 as f32 * sc * 0.25);
                                p.line_segment(
                                    [pos2(x, ey), pos2(x, ey - 40.0 * sc - b.plates[3].thickness as f32 * sc - b.plates[2].thickness as f32 * sc * 0.6)],
                                    Stroke::new(1.5_f32, Color32::from_rgb(40, 120, 40)),
                                );
                            }
                        } else {
                            let rr = egui::Rect::from_min_size(pos2(cx - wpx / 2.0, y), vec2(wpx, h));
                            p.rect_filled(rr, 0.0, steel);
                            p.rect_stroke(rr, 0.0, edge, egui::StrokeKind::Inside);
                        }
                        p.text(
                            pos2(r.left() + 4.0, y + h / 2.0),
                            egui::Align2::LEFT_CENTER,
                            format!("{}", apps::num(pl.thickness)),
                            egui::FontId::proportional(10.0),
                            Color32::from_rgb(60, 66, 76),
                        );
                        if i == 1 {
                            // the parting line and the cavities straddling it
                            let yp = y + h;
                            let (nx, _) = b.layout;
                            let ext = a.mold_res.as_ref().map(|m| m.check.extent).unwrap_or(V3::ONE);
                            let (dc, dk) = a.mold_res.as_ref().map(|m| (m.check.depth_cavity, m.check.depth_core)).unwrap_or((1.0, 1.0));
                            let pw = ext.x.max(ext.y) as f32 * sc;
                            let ins_w = b.insert.0 as f32 * sc;
                            let ir = egui::Rect::from_min_max(pos2(cx - ins_w / 2.0, yp - b.insert.2 as f32 * sc), pos2(cx + ins_w / 2.0, yp + b.insert.3 as f32 * sc));
                            p.rect_filled(ir, 0.0, Color32::from_rgb(200, 210, 222));
                            p.rect_stroke(ir, 0.0, Stroke::new(1.0_f32, Color32::from_rgb(110, 120, 135)), egui::StrokeKind::Inside);
                            for k in 0..nx {
                                let x0 = cx - (nx as f32 * pw + (nx as f32 - 1.0) * 10.0 * sc) / 2.0 + k as f32 * (pw + 10.0 * sc);
                                let pr = egui::Rect::from_min_max(pos2(x0, yp - dc as f32 * sc), pos2(x0 + pw, yp + dk as f32 * sc));
                                p.rect_filled(pr, 0.0, Color32::from_rgb(240, 160, 60));
                            }
                            p.line_segment([pos2(r.left() + 20.0, yp), pos2(r.right() - 4.0, yp)], Stroke::new(1.2_f32, Color32::from_rgb(30, 100, 200)));
                            p.text(pos2(r.right() - 6.0, yp - 7.0), egui::Align2::RIGHT_CENTER, "Trennebene", egui::FontId::proportional(10.0), Color32::from_rgb(30, 100, 200));
                        }
                        y += h;
                    }
                    // guide pillars through A and B
                    let a_top = r.top() + 15.0 + b.plates[0].thickness as f32 * sc;
                    let len = (b.plates[1].thickness + b.plates[2].thickness) as f32 * sc;
                    for k in [-1.0f32, 1.0] {
                        let x = cx + k * (b.plates[1].size.0 as f32 * sc / 2.0 - 14.0 * sc - b.guide_d as f32 * sc);
                        let gr = egui::Rect::from_min_size(pos2(x - b.guide_d as f32 * sc / 2.0, a_top), vec2(b.guide_d as f32 * sc, len));
                        p.rect_filled(gr, 0.0, Color32::from_rgb(120, 128, 140));
                    }
                    p.text(pos2(cx, r.top() + 7.0), egui::Align2::CENTER_CENTER, "Düsenseite", egui::FontId::proportional(10.5), Color32::DARK_GRAY);
                    p.text(pos2(cx, r.bottom() - 7.0), egui::Align2::CENTER_CENTER, "Auswerferseite", egui::FontId::proportional(10.5), Color32::DARK_GRAY);
                });
                ui.horizontal(|ui| {
                    if btn(ui, "Stückliste als CSV", 160.0, 0) {
                        csv = true;
                    }
                    if btn(ui, "Bericht", 90.0, 0) {
                        report = true;
                    }
                });
            });
        });
        if let Some(b) = base {
            if csv {
                let mut s = "Pos;Benennung;Menge;Länge mm;Breite mm;Dicke mm;Werkstoff\n".to_string();
                for (i, p) in b.plates.iter().enumerate() {
                    let n = if p.name.starts_with("Leisten") { 2 } else { 1 };
                    s += &format!("{};{};{};{};{};{};{}\n", i + 1, p.name, n, p.size.0, p.size.1, p.thickness, p.material);
                }
                s += &format!("{};Führungssäule ⌀{};4;;;;1.7131\n{};Führungsbuchse ⌀{};4;;;;1.7131\n", b.plates.len() + 1, b.guide_d, b.plates.len() + 2, b.guide_d);
                self.apps_save_file("formaufbau.csv", s.into_bytes());
            }
            if report {
                let mut body = format!("Kavitäten: {} ({} × {})\n\n", b.cavities, b.layout.0, b.layout.1);
                for p in &b.plates {
                    body += &format!("  {:<34}{:>16}{:>10} mm  {}\n", p.name, format!("{} × {}", p.size.0, p.size.1), p.thickness, p.material);
                }
                body += &format!(
                    "\nFührungssäulen ⌀{} · Schrauben {} · Auswerferhub {} mm\nWerkzeughöhe {} mm · Öffnungsweg {} mm · Stahl {} kg\nSchließkraft {} kN · Maschine: {}\n",
                    b.guide_d,
                    b.screw,
                    b.ejector_stroke,
                    apps::num(b.height),
                    apps::num(b.opening),
                    apps::num(b.steel_kg),
                    apps::num(b.clamp_kn),
                    b.machine.map(|m| format!("{} kN", m.kn)).unwrap_or("keine passende".into())
                );
                self.apps_report("moldbase.inf", "FORMAUFBAU", body);
            }
        }
        if !open {
            self.apps.base_open = false;
        }
    }

    // ------------------------------------------------------------ Spritzgussanalyse

    fn fill_start(&mut self) {
        let topo = self.result.topo.clone();
        if topo.polys.is_empty() {
            return;
        }
        let gate = if self.apps.fill_gate_sel {
            let mut pts = vec![];
            for p in &self.selection {
                match p {
                    Pick::Face(f) => pts.push(adv::face_anchor(&topo, *f).0),
                    Pick::Edge(e) => {
                        if let Some(c) = topo.chains.get(*e) {
                            pts.push(c.mid());
                        }
                    }
                    _ => {}
                }
            }
            if pts.is_empty() {
                self.say("Anschnitt: eine Fläche oder Kante im Modell auswählen – oder „Automatisch“ wählen.", MsgKind::Prompt);
                return;
            }
            Some(pts.iter().sum::<V3>() / pts.len() as f64)
        } else {
            None
        };
        let inp = moldflow::FillInput {
            topo,
            material: mold::MATERIALS[self.apps.fill_mat.min(mold::MATERIALS.len() - 1)],
            gate,
            fill_time: self.apps.fill_time,
            cells: [15_000, 50_000, 150_000][self.apps.fill_res.min(2)],
            cavities: 1,
        };
        if let Some(j) = &self.apps.fill_job {
            j.cancel();
        }
        self.apps.fill_play = false;
        self.apps.fill_job = Some(Job::spawn(move |sh| moldflow::run(&inp, sh)));
    }

    pub fn fill_tick(&mut self) {
        if self.apps.fill_play {
            if let Some(f) = &self.apps.fill_shown {
                self.apps.fill_t += (f.time as f32) / 90.0;
                if self.apps.fill_t >= f.time as f32 {
                    self.apps.fill_t = f.time as f32;
                    self.apps.fill_play = false;
                }
                let o = Arc::new(moldflow::progress_output(&f.out, &f.fill, self.apps.fill_t));
                self.apps_display(o);
            } else {
                self.apps.fill_play = false;
            }
        }
        let Some(j) = &self.apps.fill_job else { return };
        let Some(r) = j.take() else { return };
        self.apps.fill_job = None;
        match r {
            Ok(res) => {
                let out = Arc::new(res.out);
                self.apps.fill_shown = Some(FillShown { out: out.clone(), fill: res.fill_at_vertex, time: res.fill_time });
                self.apps.fill_t = res.fill_time as f32;
                self.apps_show(out.clone(), "Spritzgussanalyse");
                self.say(format!("Spritzgussanalyse fertig: Einspritzdruck ≈ {} MPa.", apps::num(res.pressure_mpa)), MsgKind::Success);
                for w in out.warnings.clone() {
                    self.say(w, MsgKind::Warning);
                }
            }
            Err(e) if e == "abgebrochen" => {}
            Err(e) => self.say(format!("Spritzgussanalyse: {}", e), MsgKind::Error),
        }
    }

    pub fn fill_window(&mut self, ctx: &egui::Context) {
        let (mut start, mut stop, mut show_t, mut show_full) = (false, false, false, false);
        let has_sel = self.selection.iter().any(|p| matches!(p, Pick::Face(_) | Pick::Edge(_)));
        let vol_cm3 = if self.result.polys.is_empty() { 0.0 } else { export::mass_properties(&self.result, self.data.density).volume / 1000.0 };
        let a = &mut self.apps;
        let job = a.fill_job.as_ref().map(|j| (j.progress(), j.stage(), j.started.elapsed().as_secs_f64()));
        let open = cwin(ctx, "apps_fill", "Spritzgussanalyse", true, pos2(440.0, 140.0), 600.0, |ui| {
            pad(ui, 10.0, |ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                ui.horizontal(|ui| {
                    lbl(ui, "Werkstoff");
                    let names: Vec<&str> = mold::MATERIALS.iter().map(|m| m.name).collect();
                    combo(ui, "fill_mat", 240.0, &mut a.fill_mat, &names, true, None);
                    lbl(ui, "Netz");
                    combo(ui, "fill_res", 110.0, &mut a.fill_res, &["Schnell", "Mittel", "Fein"], true, None);
                });
                ui.horizontal(|ui| {
                    lbl(ui, "Einspritzzeit");
                    ui.add(egui::DragValue::new(&mut a.fill_time).speed(0.02).range(0.05..=60.0).suffix(" s"));
                    if ui.small_button("Vorschlag").on_hover_text("Aus dem Schussvolumen (Faustformel)").clicked() {
                        a.fill_time = ((0.4 + 0.035 * vol_cm3.powf(0.9)) * 100.0).round() / 100.0;
                    }
                    lbl_dim(ui, &format!("Schussvolumen {} cm³", apps::num(vol_cm3)));
                });
                ui.horizontal(|ui| {
                    lbl(ui, "Anschnitt");
                    if ui.radio(!a.fill_gate_sel, "Automatisch (dickste Stelle, zentral)").clicked() {
                        a.fill_gate_sel = false;
                    }
                    if ui.radio(a.fill_gate_sel, "Ausgewählte Fläche/Kante").clicked() {
                        a.fill_gate_sel = true;
                    }
                });
                if a.fill_gate_sel && !has_sel {
                    lbl_dim(ui, "Im Modell eine Fläche oder Kante für den Anschnitt auswählen.");
                }
                if let Some((p, s, t)) = &job {
                    stop = progress_row(ui, *p, s, *t);
                } else if btn(ui, "Füllsimulation starten", 190.0, 1) {
                    start = true;
                }
                if let Some(f) = &a.fill_shown {
                    hline(ui);
                    lbl_bold(ui, "Füllbild");
                    ui.horizontal(|ui| {
                        if ui.button(if a.fill_play { "⏸ Pause" } else { "▶ Abspielen" }).clicked() {
                            if !a.fill_play && a.fill_t >= f.time as f32 * 0.999 {
                                a.fill_t = 0.0;
                            }
                            a.fill_play = !a.fill_play;
                        }
                        if ui.add(egui::Slider::new(&mut a.fill_t, 0.0..=f.time as f32).suffix(" s").max_decimals(2)).changed() {
                            a.fill_play = false;
                            show_t = true;
                        }
                        if ui.button("Alle Ergebnisse").clicked() {
                            show_full = true;
                        }
                    });
                    kv(ui, "fill_sum", &f.out.summary);
                    warn_rows(ui, &f.out.warnings);
                    ui.label(RichText::new("Ergebnisgrößen (Füllzeit, Druck, Wanddicke, Abkühlzeit, Füllqualität) im Register Live-Simulation unter „Ergebnisse“ umschalten. Rote Kreuze: Bindenähte, rote Ringe: Lufteinschlüsse, Linien: Anschnitt.").size(12.0).color(dlgkit_dim()));
                }
            });
        });
        if stop {
            if let Some(j) = &self.apps.fill_job {
                j.cancel();
            }
            self.apps.fill_job = None;
        }
        if start {
            self.fill_start();
        }
        if show_t {
            if let Some(f) = &self.apps.fill_shown {
                let o = Arc::new(moldflow::progress_output(&f.out, &f.fill, self.apps.fill_t));
                self.apps_display(o);
            }
        }
        if show_full {
            if let Some(f) = &self.apps.fill_shown {
                let o = f.out.clone();
                self.apps_display(o);
            }
        }
        if !open {
            self.apps.fill_open = false;
            self.apps.fill_play = false;
        }
    }

    // ------------------------------------------------------------ Generatives Design

    fn gen_start(&mut self) {
        let Some(study) = self.sim_study().cloned().filter(|s| s.kind == StudyKind::Structural) else {
            self.say("Generatives Design: zuerst eine Struktur-Studie mit Lasten und Randbedingungen anlegen.", MsgKind::Warning);
            return;
        };
        let inp = topopt::OptInput {
            topo: self.result.topo.clone(),
            study,
            volume: self.apps.gen_volume,
            iterations: self.apps.gen_iter,
            elements: [6_000, 16_000, 40_000][self.apps.gen_res.min(2)],
            filter: 1.6,
            symmetry: match self.apps.gen_sym {
                1 => Some(0),
                2 => Some(1),
                3 => Some(2),
                _ => None,
            },
            keep_skin: self.apps.gen_skin,
        };
        if let Some(j) = &self.apps.gen_job {
            j.cancel();
        }
        self.apps.gen_stl = None;
        self.apps.gen_history.clear();
        self.apps.gen_job = Some(Job::spawn(move |sh| topopt::run(&inp, sh)));
        // watch the shape form in the graphics window
        self.tab = Tab::LiveSim;
    }

    pub fn gen_tick(&mut self) {
        let Some(j) = &self.apps.gen_job else { return };
        let (partial, done) = (j.partial(), j.take());
        // intermediate shapes only while the user watches the Live-Simulation tab
        if let Some(p) = partial.filter(|_| self.tab == Tab::LiveSim) {
            self.apps_display(Arc::new(p));
        }
        let Some(r) = done else { return };
        self.apps.gen_job = None;
        match r {
            Ok(res) => {
                self.apps.gen_stl = Some(res.stl);
                self.apps.gen_history = res.history;
                let out = Arc::new(res.out);
                self.apps_show(out.clone(), "Generatives Design");
                let s = out.summary.iter().find(|s| s.0 == "Ergebnis").map(|s| s.1.clone()).unwrap_or_default();
                self.say(format!("Generatives Design fertig: {}.", s), MsgKind::Success);
                for w in out.warnings.clone() {
                    self.say(w, MsgKind::Warning);
                }
            }
            Err(e) if e == "abgebrochen" => {}
            Err(e) => self.say(format!("Generatives Design: {}", e), MsgKind::Error),
        }
    }

    pub fn gen_window(&mut self, ctx: &egui::Context) {
        let study = self.sim_study().filter(|s| s.kind == StudyKind::Structural).map(|s| {
            let fixes = s.items.iter().filter(|i| !i.kind.is_load()).count();
            let loads = s.items.iter().filter(|i| i.kind.is_load()).count();
            (s.name.clone(), s.material.clone(), fixes, loads)
        });
        let (mut start, mut stop, mut define, mut stl) = (false, false, false, false);
        let a = &mut self.apps;
        let job = a.gen_job.as_ref().map(|j| (j.progress(), j.stage(), j.started.elapsed().as_secs_f64()));
        let open = cwin(ctx, "apps_gen", "Generatives Design", true, pos2(430.0, 140.0), 560.0, |ui| {
            pad(ui, 10.0, |ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                ui.label(RichText::new("Das Teil ist der Bauraum. Lasten und Randbedingungen kommen aus der aktiven Struktur-Studie; wo sie angreifen, bleibt Material stehen. Berechnet wird die steifste Form mit dem gewählten Materialanteil.").color(dlgkit_dim()));
                match &study {
                    Some((n, m, f, l)) => {
                        lbl(ui, &format!("Studie: {} · {} · {} Randbedingung(en), {} Last(en)", n, m, f, l));
                        if *f == 0 || *l == 0 {
                            ui.label(RichText::new("⚠ Es fehlen Randbedingungen oder Lasten.").color(Color32::from_rgb(190, 100, 0)));
                        }
                    }
                    None => {
                        lbl_dim(ui, "Keine Struktur-Studie aktiv.");
                    }
                }
                if btn(ui, "Lasten und Randbedingungen festlegen", 280.0, 0) {
                    define = true;
                }
                hline(ui);
                ui.horizontal(|ui| {
                    lbl(ui, "Materialanteil");
                    let mut pct = a.gen_volume * 100.0;
                    if ui.add(egui::Slider::new(&mut pct, 8.0..=85.0).suffix(" %").max_decimals(0)).changed() {
                        a.gen_volume = pct / 100.0;
                    }
                });
                ui.horizontal(|ui| {
                    lbl(ui, "Iterationen");
                    ui.add(egui::DragValue::new(&mut a.gen_iter).range(10..=150));
                    lbl(ui, "Auflösung");
                    combo(ui, "gen_res", 110.0, &mut a.gen_res, &["Grob", "Mittel", "Fein"], true, None);
                });
                ui.horizontal(|ui| {
                    lbl(ui, "Symmetrie");
                    combo(ui, "gen_sym", 150.0, &mut a.gen_sym, &["keine", "Ebene ⟂ X", "Ebene ⟂ Y", "Ebene ⟂ Z"], true, None);
                    ui.checkbox(&mut a.gen_skin, "Außenhaut erhalten");
                });
                if let Some((p, s, t)) = &job {
                    stop = progress_row(ui, *p, s, *t);
                } else {
                    ui.horizontal(|ui| {
                        if btn(ui, "Optimierung starten", 170.0, if study.is_some() { 1 } else { 2 }) && study.is_some() {
                            start = true;
                        }
                        if a.gen_stl.is_some() && btn(ui, "Ergebnis als STL speichern", 210.0, 0) {
                            stl = true;
                        }
                    });
                }
                if a.gen_history.len() > 1 {
                    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 90.0), Sense::hover());
                    let p = ui.painter_at(r);
                    p.rect_filled(r, 2.0, Color32::WHITE);
                    let c0 = a.gen_history.iter().map(|h| h.0).fold(0.0, f64::max).max(1e-30);
                    let pts: Vec<egui::Pos2> = a
                        .gen_history
                        .iter()
                        .enumerate()
                        .map(|(i, h)| {
                            pos2(r.left() + 6.0 + (r.width() - 12.0) * i as f32 / (a.gen_history.len() - 1) as f32, r.bottom() - 6.0 - (r.height() - 20.0) * (h.0 / c0) as f32)
                        })
                        .collect();
                    p.add(egui::Shape::line(pts, Stroke::new(1.6_f32, Color32::from_rgb(40, 110, 200))));
                    p.text(
                        r.left_top() + vec2(6.0, 8.0),
                        egui::Align2::LEFT_CENTER,
                        "Nachgiebigkeit je Iteration (kleiner = steifer)",
                        egui::FontId::proportional(11.0),
                        Color32::DARK_GRAY,
                    );
                }
            });
        });
        if define {
            self.tab = Tab::LiveSim;
            if self.sim_study().is_none_or(|s| s.kind != StudyKind::Structural) {
                if let Some(i) = self.data.sims.iter().position(|s| s.kind == StudyKind::Structural) {
                    self.sim_activate(i);
                } else {
                    self.sim_new_study(StudyKind::Structural);
                }
            }
            self.sim.viewing_saved = false;
            self.sim.show_results = false;
            self.say("Flächen wählen und im Register Live-Simulation „Fest“ und eine Last hinzufügen.", MsgKind::Prompt);
        }
        if stop {
            if let Some(j) = &self.apps.gen_job {
                j.cancel();
            }
            self.apps.gen_job = None;
        }
        if start {
            self.gen_start();
        }
        if stl {
            if let Some(b) = self.apps.gen_stl.clone() {
                self.apps_save_file("generativ.stl", b);
            }
        }
        if !open {
            self.apps.gen_open = false;
        }
    }

    // ------------------------------------------------------------ GD&T Advisor

    pub fn gdt_analyze(&mut self) {
        let topo = self.result.topo.clone();
        self.apps.gdt_items = adv::advise(&topo, self.apps.gdt_class, &self.data.annots);
        self.apps.gdt_checks = adv::check(&self.data.annots);
        self.apps.gdt_done = false;
    }

    fn gdt_apply(&mut self) {
        let topo = self.result.topo.clone();
        let diag = topo.bbox.diagonal().max(1.0);
        let plane = self.annot_plane_name();
        let mut next = self.data.annots.iter().map(|a| a.id).max().unwrap_or(0).max(self.data.annot_groups.iter().map(|g| g.id).max().unwrap_or(0)) + 1;
        let mut new = vec![];
        for (k, it) in self.apps.gdt_items.iter().filter(|i| i.on).enumerate() {
            let mut refs = vec![];
            for &f in std::iter::once(&it.face).chain(it.also.iter()) {
                if f < topo.faces.len() {
                    let (p, n) = adv::face_anchor(&topo, f);
                    refs.push(ARef::Face { p, n });
                }
            }
            let Some(ARef::Face { p, n }) = refs.first().cloned() else { continue };
            let kind = match &it.what {
                adv::Proposal::Datum { letter } => AnnotKind::Datum { letter: letter.clone() },
                adv::Proposal::Gtol { ch, diameter, value, mat, datums } => AnnotKind::Gtol { ch: *ch, diameter: *diameter, value: *value, mat: *mat, datums: datums.clone() },
            };
            // datum symbols close to the face, frames a little further out and staggered
            let off = if matches!(kind, AnnotKind::Datum { .. }) { 0.08 } else { 0.16 + 0.03 * (k % 3) as f64 };
            new.push(Annot3 { id: next, kind, refs, pos: p + n * (diag * off), plane: plane.clone(), group: None });
            next += 1;
        }
        let n = new.len();
        if n == 0 {
            self.say("GD&T Advisor: kein Vorschlag ausgewählt.", MsgKind::Info);
            return;
        }
        self.commit(&format!("GD&T Advisor: {} Anmerkung(en) erzeugt.", n), |d| d.annots.extend(new));
        self.apps.gdt_done = true;
        self.apps.gdt_checks = adv::check(&self.data.annots);
        self.tab = Tab::Annot3d;
    }

    pub fn gdt_window(&mut self, ctx: &egui::Context) {
        let (mut redo, mut apply, mut pick) = (false, false, None);
        let a = &mut self.apps;
        let open = cwin(ctx, "apps_gdt", "GD&T Advisor", true, pos2(400.0, 120.0), 700.0, |ui| {
            pad(ui, 10.0, |ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                ui.horizontal(|ui| {
                    lbl(ui, "Toleranzklasse ISO 2768-2");
                    redo |= combo(ui, "gdt_class", 130.0, &mut a.gdt_class, &adv::CLASSES, true, None);
                    if ui.button("Neu analysieren").clicked() {
                        redo = true;
                    }
                });
                lbl_bold(ui, "Vorschläge");
                if a.gdt_items.is_empty() {
                    lbl_dim(ui, "Keine geeigneten Flächen gefunden.");
                }
                egui::ScrollArea::vertical().max_height(340.0).id_salt("gdt_items").show(ui, |ui| {
                    for (i, it) in a.gdt_items.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            ui.checkbox(&mut it.on, "");
                            let icon = match &it.what {
                                adv::Proposal::Datum { letter } => format!("[{}]", letter),
                                adv::Proposal::Gtol { ch, .. } => format!("{}", gchar_symbol(*ch)),
                            };
                            ui.label(RichText::new(icon).monospace().strong().color(Color32::from_rgb(20, 70, 150)));
                            if ui.link(&it.label).clicked() {
                                pick = Some(i);
                            }
                        });
                        ui.label(RichText::new(format!("      {}", it.reason)).size(12.0).color(dlgkit_dim()));
                    }
                });
                ui.horizontal(|ui| {
                    if btn(ui, "Ausgewählte Vorschläge übernehmen", 260.0, if a.gdt_items.iter().any(|i| i.on) { 1 } else { 2 }) {
                        apply = true;
                    }
                    if ui.button("Alle").clicked() {
                        a.gdt_items.iter_mut().for_each(|i| i.on = true);
                    }
                    if ui.button("Keine").clicked() {
                        a.gdt_items.iter_mut().for_each(|i| i.on = false);
                    }
                });
                if a.gdt_done {
                    ui.label(RichText::new("✔ Übernommen – die Anmerkungen stehen im Register Anmerkungen erstellen.").color(Color32::from_rgb(20, 130, 50)));
                }
                hline(ui);
                lbl_bold(ui, "Prüfung der vorhandenen Angaben");
                if a.gdt_checks.is_empty() {
                    ui.label(RichText::new("✔ Keine Widersprüche gefunden.").color(Color32::from_rgb(20, 130, 50)));
                } else {
                    warn_rows(ui, &a.gdt_checks);
                }
            });
        });
        if redo {
            self.gdt_analyze();
        }
        if let Some(i) = pick {
            if let Some(it) = self.apps.gdt_items.get(i) {
                self.selection = std::iter::once(it.face).chain(it.also.iter().copied()).map(Pick::Face).collect();
            }
        }
        if apply {
            self.gdt_apply();
        }
        if !open {
            self.apps.gdt_open = false;
        }
    }

    // ------------------------------------------------------------ Render Studio

    fn render_start(&mut self) {
        if let Some(r) = &self.apps.render {
            r.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        let a = &self.apps;
        let (w, h) = [(640, 400), (960, 600), (1600, 1000), (2560, 1600)][a.render_size.min(3)];
        let c = self.data.appearance.color;
        let s = raytrace::Settings {
            width: w,
            height: h,
            scene: raytrace::Scene::ALL[a.render_scene.min(2)],
            material: raytrace::MATERIALS[a.render_mat.min(raytrace::MATERIALS.len() - 1)],
            color: [c[0] as f64, c[1] as f64, c[2] as f64],
            ground: a.render_ground,
            ground_mirror: a.render_mirror,
            exposure: a.render_exposure,
            max_samples: [16, 64, 256][a.render_quality.min(2)],
            bounces: 5,
            light_az: a.render_az,
            light_el: a.render_el,
        };
        // the view of the graphics window, fitted to the picture's aspect ratio
        let cam = self.camera;
        self.apps.render = Some(raytrace::start(self.result.topo.clone(), cam, s));
        self.apps.render_shown = 0;
    }

    pub fn render_window(&mut self, ctx: &egui::Context) {
        if self.apps.render_auto && !self.fit_pending {
            self.apps.render_auto = false;
            self.render_start();
        }
        // upload the picture when new samples arrived
        if let Some(r) = &self.apps.render {
            let n = r.samples.load(std::sync::atomic::Ordering::Relaxed);
            if n != self.apps.render_shown {
                let img = egui::ColorImage::from_rgba_unmultiplied([r.settings.width, r.settings.height], &r.image());
                match &mut self.apps.render_tex {
                    Some(t) => t.set(img, egui::TextureOptions::LINEAR),
                    None => self.apps.render_tex = Some(ctx.load_texture("render_studio", img, egui::TextureOptions::LINEAR)),
                }
                self.apps.render_shown = n;
            }
            if n < r.settings.max_samples && !r.cancel.load(std::sync::atomic::Ordering::Relaxed) {
                ctx.request_repaint_after(std::time::Duration::from_millis(150));
            }
        }
        let (mut start, mut stop, mut save) = (false, false, false);
        let a = &mut self.apps;
        let status = a
            .render
            .as_ref()
            .map(|r| (r.samples.load(std::sync::atomic::Ordering::Relaxed), r.settings.max_samples, r.started.elapsed().as_secs_f64(), r.settings.width, r.settings.height));
        let open = cwin(ctx, "apps_render", "Render Studio", true, pos2(300.0, 90.0), 980.0, |ui| {
            pad(ui, 10.0, |ui| {
                ui.spacing_mut().item_spacing = vec2(6.0, 6.0);
                ui.horizontal_top(|ui| {
                    ui.vertical(|ui| {
                        ui.set_width(250.0);
                        lbl_bold(ui, "Szene");
                        let names: Vec<&str> = raytrace::Scene::ALL.iter().map(|s| s.name()).collect();
                        combo(ui, "render_scene", 240.0, &mut a.render_scene, &names, true, None);
                        lbl_bold(ui, "Material");
                        let names: Vec<&str> = raytrace::MATERIALS.iter().map(|m| m.name).collect();
                        combo(ui, "render_mat", 240.0, &mut a.render_mat, &names, true, None);
                        lbl_dim(ui, "Kunststoffe und Lack in der Farbe des Teils (Darstellung ▸ Erscheinungsbild).");
                        ui.checkbox(&mut a.render_ground, "Boden mit Schatten");
                        ui.add_enabled(a.render_ground, egui::Checkbox::new(&mut a.render_mirror, "Boden spiegelnd"));
                        lbl_bold(ui, "Hauptlicht");
                        ui.add(egui::Slider::new(&mut a.render_az, -180.0..=180.0).text("Richtung").suffix("°"));
                        ui.add(egui::Slider::new(&mut a.render_el, 5.0..=89.0).text("Höhe").suffix("°"));
                        ui.add(egui::Slider::new(&mut a.render_exposure, 0.2..=4.0).text("Belichtung").logarithmic(true));
                        lbl_bold(ui, "Bild");
                        combo(ui, "render_size", 240.0, &mut a.render_size, &["640 × 400", "960 × 600", "1600 × 1000", "2560 × 1600"], true, None);
                        combo(ui, "render_q", 240.0, &mut a.render_quality, &["Entwurf (16 Durchgänge)", "Gut (64 Durchgänge)", "Final (256 Durchgänge)"], true, None);
                        ui.add_space(4.0);
                        if btn(ui, "Rendern", 240.0, 1) {
                            start = true;
                        }
                        ui.horizontal(|ui| {
                            if status.is_some_and(|s| s.0 < s.1) && ui.button("Anhalten").clicked() {
                                stop = true;
                            }
                            if status.is_some_and(|s| s.0 > 0) && ui.button("Bild speichern (PNG)").clicked() {
                                save = true;
                            }
                        });
                        if let Some((n, m, t, _, _)) = status {
                            ui.add(egui::ProgressBar::new(n as f32 / m as f32).text(format!("{} / {} Durchgänge · {:.0} s", n, m, t)));
                        }
                        ui.label(RichText::new("Gerendert wird die aktuelle Ansicht des Grafikfensters.").size(12.0).color(dlgkit_dim()));
                    });
                    let avail = 690.0f32;
                    match (&a.render_tex, status) {
                        (Some(t), Some((_, _, _, w, h))) => {
                            let s = (avail / w as f32).min(470.0 / h as f32);
                            ui.image((t.id(), vec2(w as f32 * s, h as f32 * s)));
                        }
                        _ => {
                            let (r, _) = ui.allocate_exact_size(vec2(avail, 430.0), Sense::hover());
                            ui.painter().rect_filled(r, 2.0, Color32::from_rgb(232, 234, 237));
                            ui.painter().text(
                                r.center(),
                                egui::Align2::CENTER_CENTER,
                                "Ansicht im Grafikfenster einrichten, dann „Rendern“.",
                                egui::FontId::proportional(14.0),
                                Color32::DARK_GRAY,
                            );
                        }
                    }
                });
            });
        });
        if stop {
            if let Some(r) = &self.apps.render {
                r.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }
        if start {
            self.render_start();
        }
        if save {
            if let Some(r) = self.apps.render.clone() {
                let png = crate::softrender::encode_png_rgba(r.settings.width, r.settings.height, &r.image());
                self.apps_save_file("rendering.png", png);
            }
        }
        if !open {
            self.apps.render_open = false;
            if let Some(r) = self.apps.render.take() {
                r.cancel.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            self.apps.render_tex = None;
        }
    }
}

/// ISO 1101 symbol of a characteristic.
pub fn gchar_symbol(c: crate::model::drawing::GChar) -> &'static str {
    use crate::model::drawing::GChar::*;
    match c {
        Straightness => "⏤",
        Flatness => "⏥",
        Circularity => "○",
        Cylindricity => "⌭",
        LineProfile => "⌒",
        SurfaceProfile => "⌓",
        Angularity => "∠",
        Perpendicularity => "⟂",
        Parallelism => "∥",
        Position => "⌖",
        Concentricity => "◎",
        Symmetry => "⌯",
        CircularRunout => "↗",
        TotalRunout => "⌰",
    }
}

impl App {
    /// Screenshot scenes (FREO_DEMO=app:<tool>): the dialog open on the demo part.
    pub fn demo_app_tool(&mut self, which: &str) {
        match which {
            "weld" => {
                let topo = self.result.topo.clone();
                let long: Vec<usize> = (0..topo.chains.len()).filter(|&c| !topo.chains[c].convex && topo.chains[c].length() > 30.0).take(2).collect();
                let edges = if long.is_empty() { vec![0] } else { long };
                self.selection = edges.into_iter().map(Pick::Edge).collect();
                self.weld_new_from_selection();
                self.apps.weld_open = true;
            }
            "mold" => {
                self.apps.mold_open = true;
                self.mold_analyze();
            }
            "moldbase" => {
                self.apps.mold_cav = 2;
                self.open_moldbase();
            }
            "gdt" => {
                self.apps.gdt_open = true;
                self.gdt_analyze();
            }
            "math" => {
                self.apps.math_open = true;
                let (n, t) = crate::apps::worksheet::TEMPLATES[0];
                self.math_insert_template(n, t);
            }
            "tol" => {
                use crate::apps::stackup::*;
                let mk = |n: &str, nom: f64, t: f64, dir: i8| Contributor { name: n.into(), nominal: nom, upper: t, lower: -t, dir, dist: Dist::Normal };
                self.data.apps.stackups = vec![Stackup {
                    name: "Spalt Deckel".into(),
                    items: vec![mk("Gehäuse innen", 80.0, 0.1, 1), mk("Platte", 60.0, 0.05, -1), mk("Distanzring", 19.0, 0.08, -1)],
                    lsl: Some(0.7),
                    usl: Some(1.3),
                }];
                self.apps.tol_open = true;
            }
            "fill" => {
                self.apps.fill_open = true;
                self.apps.fill_res = 0;
                self.fill_start();
            }
            "render" => {
                self.apps.render_open = true;
                self.apps.render_quality = 0;
                self.apps.render_size = 0;
                self.apps.render_auto = true;
            }
            "launch" => self.apps.launch = Some(super::apps::Launch::Flow),
            _ => {}
        }
    }
}
