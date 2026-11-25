//! Werkzeuge ▸ Assembler uses the ordinary component placement dashboard and .pfasm files.
use super::icons::Icon;
use super::ribbon::{drop_menu, group, large, menu_entry, small, split, stack};
use super::*;
use crate::model::assembly::{AsmCType, AsmConstraint, Component, Placement};
use crate::model::{
    fastener::{self, Head},
    threads::ISO,
};
use crate::pcb::{
    bench::{Bench, Drive, LedState, Level},
    body3d,
    modules::Module,
    Board,
};
use eframe::egui;
use eframe::egui::{Color32, Pos2, Stroke};
use std::collections::VecDeque;

mod circuit;
mod library;
mod placement;
mod strength;
pub use circuit::CircuitUi;
pub use library::LibraryUi;
pub(super) use placement::Guide;
pub use strength::StrengthUi;

pub struct Assembler {
    pub active: bool,
    screws: bool,
    screw_used: bool,
    pub(super) pending: VecDeque<PathBuf>,
    head: Head,
    thread: usize,
    length: u16,
    pub bench: Option<BenchWindow>,
    pub library: Option<LibraryUi>,
    pub circuit: Option<CircuitUi>,
    pub strength: Option<StrengthUi>,
}

impl Default for Assembler {
    fn default() -> Self {
        Self { active: false, screws: false, screw_used: false, pending: VecDeque::new(), head: Head::Phillips, thread: 2, length: 10, bench: None, library: None, circuit: None, strength: None }
    }
}

pub struct BenchWindow {
    title: String,
    bench: Bench,
    filter: String,
}

impl BenchWindow {
    pub fn new(name: &str, board: &Board) -> Self {
        Self { title: format!("GPIO-Prüfstand · {name}"), bench: Bench::new(board), filter: String::new() }
    }
    fn show(&mut self, ctx: &egui::Context) -> bool {
        let mut open = true;
        egui::Window::new(&self.title).id(egui::Id::new("assembler_gpio")).open(&mut open).default_size([550.0, 430.0]).show(ctx, |ui| {
            ui.label("USB oder Labornetzteil einschalten und GPIO-Ausgänge schalten. Die Messpunkte folgen dem gerouteten Kupfer.");
            ui.small("Statisches GPIO-Modell · ideale Versorgung · rote LED mit 2 V Flussspannung");
            if self.bench.chips.is_empty() {
                ui.label("Prüfpads über das Labornetzteil speisen oder im PCB-Editor ein Rechnermodul platzieren.");
            }
            for chip in &mut self.bench.chips {
                ui.checkbox(&mut chip.usb, format!("USB · {}", chip.label));
            }
            ui.collapsing("Labornetzteil an Prüfpads", |ui| {
                let supply = &mut self.bench.supply;
                let contacts = &self.bench.contacts;
                let label = |i: usize| contacts.get(i).map(|c| format!("{} · {} {}", c.reference, c.number, c.name)).unwrap_or_default();
                ui.horizontal(|ui| {
                    ui.checkbox(&mut supply.on, "Ein");
                    ui.selectable_value(&mut supply.five_volts, false, "3,3 V");
                    ui.selectable_value(&mut supply.five_volts, true, "5 V");
                });
                for (title, pin) in [("Plus", &mut supply.positive), ("Masse", &mut supply.ground)] {
                    egui::ComboBox::from_label(title).selected_text(label(*pin)).show_ui(ui, |ui| {
                        for i in 0..contacts.len() {
                            ui.selectable_value(pin, i, label(i));
                        }
                    });
                }
            });
            ui.horizontal(|ui| {
                ui.label("Pin oder Netz suchen");
                ui.text_edit_singleline(&mut self.filter);
            });
            let readings = self.bench.read();
            for (chip, on) in self.bench.chips.iter().zip(readings.running) {
                ui.label(format!("{} · {}", chip.label, if on { "GPIO aktiv" } else { "Versorgung / Freigabe fehlt" }));
            }
            for (reference, state) in readings.leds {
                let desc = match state {
                    LedState::Current(i) if i > 0.0 => format!("leuchtet · {i:.1} mA"),
                    LedState::Current(_) => "aus · 0 mA".into(),
                    LedState::MissingResistor => "Vorwiderstand fehlt".into(),
                    LedState::Unresolved => "Strompfad offen oder außerhalb des LED-Modells".into(),
                };
                ui.label(format!("{reference} · {desc}"));
            }
            let query = self.filter.trim().to_lowercase();
            egui::ScrollArea::both().auto_shrink([false, false]).show(ui, |ui| {
                egui::Grid::new("gpio_contacts").striped(true).spacing([12.0, 3.0]).show(ui, |ui| {
                    for h in ["Kontakt", "Pin", "Netz", "Ausgang", "Messwert"] {
                        ui.strong(h);
                    }
                    ui.end_row();
                    for (pin, level) in self.bench.contacts.iter_mut().zip(readings.pins) {
                        let pad = format!("{} · {}", pin.reference, pin.number);
                        if !query.is_empty() && ![&pad, &pin.name, &pin.net].iter().any(|s| s.to_lowercase().contains(&query)) {
                            continue;
                        }
                        ui.label(pad);
                        ui.label(&pin.name);
                        ui.label(&pin.net);
                        if pin.output {
                            ui.horizontal(|ui| {
                                for (drive, label) in [(Drive::Open, "Z"), (Drive::Low, "0"), (Drive::High, "1")] {
                                    ui.selectable_value(&mut pin.drive, drive, label);
                                }
                            });
                        } else {
                            ui.label("—");
                        }
                        if level == Level::Conflict || (level == Level::Five && pin.name.starts_with("GPIO")) {
                            ui.colored_label(egui::Color32::from_rgb(190, 61, 40), if level == Level::Five { "5 V am GPIO".into() } else { level.label() });
                        } else {
                            ui.label(level.label());
                        }
                        ui.end_row();
                    }
                });
            });
        });
        open
    }
}

impl App {
    pub fn assembler_group(&mut self, ui: &mut egui::Ui) {
        group(ui, "Montage", |ui| {
            if large(ui, Icon::Assemble, "Assembler", !self.busy() && self.drw.is_none() && self.idea.is_none(), false)
                .on_hover_text("Baugruppe mit Teilen, Schrauben und bestückten Leiterplatten zusammenbauen")
                .clicked()
            {
                self.open_assembler();
            }
        });
    }

    pub fn open_assembler(&mut self) {
        if self.busy() || self.drw.is_some() || self.idea.is_some() {
            return;
        }
        if self.editing_from_asm().is_some() {
            self.asm_return();
        }
        if !self.in_asm() {
            let include = self.path.is_some() || self.dirty || self.data.features.iter().any(|f| !f.kind.is_datum());
            let base = include.then(|| (self.data.clone(), self.path.clone(), self.dirty));
            let name = if include { format!("{}_ASM", self.data.name) } else { "ASM0001".into() };
            self.new_assembly(&name);
            if let Some((data, path, dirty)) = base {
                self.asm_commit("Aktuelles Teil als Basis eingebaut.", move |a| {
                    let part = a.add_part(data, path);
                    a.part_mut(&part).unwrap().modified = dirty;
                    let mut c = Component::new(a.next_id, &part, Placement::default());
                    c.constraints.push(AsmConstraint::new(AsmCType::Default));
                    a.components.push(c);
                    a.next_id += 1;
                });
            }
        }
        self.asm.as_mut().unwrap().assembler.active = true;
        self.tab = Tab::Tools;
    }

    pub fn assembler_place_pcb(&mut self) {
        if !self.in_asm() || self.busy() {
            return;
        }
        let project = &self.pcb.project;
        match body3d::populated_part(&project.board, &project.name) {
            Ok(part) => {
                let board = Arc::new(project.board.clone());
                if self.pcb.open {
                    self.pcb_close();
                }
                self.asm.as_mut().unwrap().assembler.active = true;
                self.stage_part(part, None, Some(board));
            }
            Err(e) => self.say(e, MsgKind::Error),
        }
    }

    pub fn assembler_place_module(&mut self, module: Module) {
        if !self.in_asm() || self.busy() {
            return;
        }
        match body3d::module_part(module) {
            Ok((part, board)) => {
                self.stage_part(part, None, Some(Arc::new(board)));
            }
            Err(e) => self.say(e, MsgKind::Error),
        }
    }

    pub fn assembler_place_screw(&mut self, thread: usize, length: u16, head: Head) {
        let Some(spec) = ISO.get(thread) else { return };
        match fastener::screw(spec, length, head) {
            Ok(part) => self.stage_part(part, None, None),
            Err(e) => self.say(e, MsgKind::Error),
        }
    }

    fn test_component(&mut self, id: u64) {
        let Some(a) = &mut self.asm else { return };
        let Some(part) = a.data.component(id).filter(|c| !c.suppressed).and_then(|c| a.data.part(&c.part)) else { return };
        if let Some(board) = &part.board {
            a.assembler.bench = Some(BenchWindow::new(&format!("{} · {id}", part.data.name), board));
        }
    }

    pub(super) fn assembler_workspace(&self) -> bool {
        self.in_asm() && self.asm.as_ref().is_some_and(|a| a.assembler.active) && (self.tab == Tab::Tools && !self.busy() || self.placement_guide().is_some())
    }

    fn choose_assembler_files(&mut self) {
        if let Some(paths) = rfd::FileDialog::new()
            .set_title("Teile für die Baugruppe auswählen")
            .set_directory(&self.browser_dir)
            .add_filter("Teile und 3D-Modelle", &["pfprt", "step", "stp", "p21", "stl", "obj"])
            .pick_files()
        {
            self.queue_assembler_files(paths);
        }
    }

    pub fn queue_assembler_files(&mut self, paths: Vec<PathBuf>) {
        if !self.assembler_workspace() {
            return;
        }
        for path in paths {
            let name = path.to_string_lossy().to_lowercase();
            if name.ends_with(".pfprt") || super::creo::is_neutral_file(&name) && !super::drawimport::is_drawing2d(&name) {
                self.asm.as_mut().unwrap().assembler.pending.push_back(path);
            } else {
                self.say(format!("{} ist keine unterstützte Teiledatei.", path.display()), MsgKind::Warning);
            }
        }
        self.next_assembler_file();
    }

    pub(super) fn next_assembler_file(&mut self) {
        while self.assembler_workspace() && !self.busy() {
            let Some(path) = self.asm.as_mut().unwrap().assembler.pending.pop_front() else { break };
            self.assemble_path(path);
        }
    }

    pub(super) fn assembler_panel(&mut self, ui: &mut egui::Ui) {
        let a = self.asm.as_ref().unwrap();
        let components: Vec<_> = a.data.components.iter().map(|c| (c.id, c.part.clone(), c.suppressed, c.hidden)).collect();
        let base = a.data.components.iter().find(|c| !c.suppressed).map(|c| c.id);
        let pending = a.assembler.pending.len();
        self.tree_hover = None;
        egui::Frame::NONE.inner_margin(12.0).show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.add_space(4.0);
            ui.heading("Baugruppe");
            ui.add_space(5.0);
            if components.is_empty() {
                ui.label("Wähle zuerst das Gehäuse oder die Grundplatte. Das erste Teil wird automatisch zur Basis.");
                ui.add_space(8.0);
                if ui.button("Teile auswählen…").clicked() {
                    self.choose_assembler_files();
                }
                ui.add_space(5.0);
                ui.weak("Mehrere Teile lassen sich auch gemeinsam aus dem Finder hierher ziehen.");
                return;
            }
            if let Some(ed) = self.place_editor() {
                ui.strong(&ed.comp.part);
                ui.weak("Wird gerade platziert");
                if pending > 0 {
                    ui.weak(format!("Danach noch {pending} in der Warteschlange"));
                }
            } else {
                ui.label("Weitere Teile hinzufügen oder ein eingebautes Teil auswählen.");
            }
            ui.add_space(7.0);
            ui.separator();
            ui.add_space(4.0);
            ui.strong(format!("Eingebaut ({})", components.len()));
            let selected = self.tree_selection.last().copied();
            if let Some((id, name, false, hidden)) = components.iter().find(|(id, ..)| Some(*id) == selected) {
                if !self.busy() {
                    ui.horizontal_wrapped(|ui| {
                        if ui.button("Weiteres Exemplar").clicked() {
                            self.assemble_part(name);
                        }
                        if ui.add_enabled(!hidden, egui::Button::new("Neu platzieren")).clicked() {
                            self.reposition_in_assembler(*id);
                        }
                    });
                }
            }
            ui.add_space(4.0);
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                for (id, name, suppressed, hidden) in components {
                    let suffix = if suppressed {
                        " · unterdrückt"
                    } else if hidden {
                        " · ausgeblendet"
                    } else if Some(id) == base {
                        " · Basis"
                    } else {
                        ""
                    };
                    let label = format!("{name}{suffix}");
                    let response = ui.add_enabled(!self.busy(), egui::Button::new(label).selected(Some(id) == selected).min_size(egui::vec2(ui.available_width(), 28.0)));
                    if response.clicked() {
                        self.tree_click(id, false);
                    }
                    if response.hovered() {
                        self.tree_hover = Some(id);
                    }
                    if !self.busy() {
                        response.context_menu(|ui| self.comp_menu(ui, id));
                    }
                }
            });
        });
    }

    pub(super) fn guided_ribbon(&mut self, ui: &mut egui::Ui) {
        let g = self.placement_guide().unwrap();
        let screw = matches!(g.method, placement::Method::Hole { .. });
        let free = g.free();
        let ready = self.can_finish_guided();
        let ed = self.place_editor().unwrap();
        let can_change_screw = ed.is_new && self.asm.as_ref().unwrap().assembler.pending.is_empty();
        let can_snap = self.asm.as_ref().unwrap().data.components.iter().take_while(|c| c.id != ed.comp.id).any(|c| !c.hidden && !c.suppressed);
        let choosing = g.choosing_source();
        let is_face = matches!(g.method, placement::Method::Face(_));
        let instruction = g.instruction();
        group(ui, "Platzierung", |ui| {
            if large(ui, Icon::Ok, "Einbauen", ready, false).on_hover_text("Platzierung übernehmen (Enter)").clicked() {
                self.place_finish();
            }
            if large(ui, Icon::Cancel, "Abbrechen", true, false).on_hover_text("Platzierung und ausstehende Importe abbrechen (Esc)").clicked() {
                self.place_cancel();
            }
        });
        group(ui, "Im Modell", |ui| {
            ui.vertical(|ui| {
                ui.set_width(255.0);
                ui.add_space(9.0);
                ui.strong(if free {
                    "Frei platzieren"
                } else if ready {
                    "Position prüfen"
                } else if screw {
                    "Bohrung wählen"
                } else {
                    if choosing {
                        "1 · Fläche am beweglichen Teil"
                    } else {
                        "2 · Zielfläche am eingebauten Teil"
                    }
                });
                ui.label(instruction);
            });
        });
        if self.placement_guide().is_some() {
            group(ui, "Drehung", |ui| self.rotation_controls(ui));
        }
        group(ui, "Anpassen", |ui| {
            stack(ui, |ui| {
                if free {
                    if small(ui, Icon::Assemble, "Wieder einrasten", can_snap, false).clicked() {
                        self.restart_snap();
                    }
                } else if screw {
                    if can_change_screw && small(ui, Icon::CosmThread, "Schraube ändern", true, false).clicked() {
                        self.place_cancel();
                        self.asm.as_mut().unwrap().assembler.screws = true;
                    }
                } else if is_face && choosing {
                    if small(ui, Icon::Plane, "Unterseite verwenden", true, false).clicked() {
                        if let Some(face) = self.underside() {
                            self.choose_source(face);
                        } else {
                            self.say("Keine ebene Unterseite gefunden. Eine Fläche am Teil anklicken.", MsgKind::Prompt);
                        }
                    }
                } else if small(ui, Icon::Plane, "Kontaktfläche wechseln", true, false).clicked() {
                    self.change_contact();
                }
                if small(ui, Icon::DragComp, "Frei verschieben", !free, free).clicked() {
                    self.assembler_place_freely();
                }
                if small(ui, Icon::Options, "Bedingungen bearbeiten", true, false).clicked() {
                    self.advanced_placement();
                }
            });
        });
    }

    fn rotation_controls(&mut self, ui: &mut egui::Ui) {
        let ed = self.asm.as_mut().unwrap().place.as_mut().unwrap();
        let g = ed.guide.as_mut().unwrap();
        let normal = match g.method {
            placement::Method::Face(Some(face)) => -ed.comp.placement.vector(face.normal),
            placement::Method::Hole { .. } => ed.comp.placement.vector(V3::Y),
            _ => V3::ZERO,
        };
        let axes = [V3::X, V3::Y, V3::Z, V3::ZERO];
        let allowed = |i: usize| !g.attached || i == 3 || normal.cross(axes[i]).length() < 1e-6;
        if !allowed(g.axis) {
            g.axis = 3;
        }
        let mut turn = 0.0;
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                for (i, label) in ["X", "Y", "Z", "Kontakt"].into_iter().enumerate().take(if g.attached { 4 } else { 3 }) {
                    if ui
                        .add_enabled(allowed(i), egui::Button::new(label).selected(g.axis == i))
                        .on_disabled_hover_text("Für diese Achse zuerst „Frei verschieben“ wählen.")
                        .clicked()
                    {
                        g.axis = i;
                    }
                }
            });
            ui.horizontal(|ui| {
                if ui.button("−").on_hover_text("Negativ um die gewählte Achse drehen").clicked() {
                    turn = -g.degrees;
                }
                ui.add(egui::DragValue::new(&mut g.degrees).range(0.01..=360.0).speed(1.0).suffix("°")).on_hover_text("Drehwinkel pro Klick");
                if ui.button("+").on_hover_text("Positiv um die gewählte Achse drehen").clicked() {
                    turn = g.degrees;
                }
            });
            if g.attached {
                if ui.small_button("90° drehen").on_hover_text("Vierteldrehung um die Kontaktnormale").clicked() {
                    g.axis = 3;
                    turn = 90.0;
                }
            } else {
                ui.small("Achsen der Baugruppe");
            }
        });
        let axis = axes[g.axis];
        if turn != 0.0 {
            self.rotate_guided(axis, turn);
        }
    }

    pub fn assembler_ribbon(&mut self, ui: &mut egui::Ui) {
        let free = !self.busy();
        group(ui, "Assembler", |ui| {
            if large(ui, Icon::Open, "Teile\nhinzufügen", free, false).clicked() {
                self.choose_assembler_files();
            }
            if large(ui, Icon::CosmThread, "Schraube", free, false).clicked() {
                let a = self.asm.as_mut().unwrap();
                if !a.assembler.screw_used && a.data.components.iter().any(|c| c.part == Module::Pi4.name() && !c.suppressed) {
                    a.assembler.thread = 1;
                }
                a.assembler.screws = true;
            }
            if large(ui, Icon::Pcb3d, "Bibliothek", free, false).on_hover_text("Mikrocontroller, Bauteile, Befestigung und Gehäuse aus der eingebauten Teilebibliothek").clicked() {
                self.open_library();
            }
        });
        group(ui, "Leiterplatte", |ui| {
            if large(ui, Icon::PcbBoard, "PCB-Editor", free, false).on_hover_text("Platine entwerfen oder ein KiCad-Projekt öffnen").clicked() {
                self.pcb_open();
            }
            if large(ui, Icon::Pcb3d, "Platine\neinbauen", free && !self.pcb.brd().outline.is_empty(), false)
                .on_hover_text(format!("Bestückte Platine aus dem aktuellen Projekt {} einbauen", self.pcb.project.name))
                .clicked()
            {
                self.assembler_place_pcb();
            }
        });
        group(ui, "Prüfen", |ui| {
            let a = self.asm.as_ref().unwrap();
            let boards: Vec<_> =
                a.data.components.iter().filter(|c| !c.suppressed && a.data.part(&c.part).is_some_and(|p| p.board.is_some())).map(|c| (c.id, c.part.clone())).collect();
            let (go, menu) = split(ui, Icon::PcbErc, "GPIO-Prüfstand", free && !boards.is_empty(), false);
            if go.clicked() {
                if let Some((id, _)) = boards.iter().find(|(id, _)| self.tree_selection.contains(id)).or(boards.first()) {
                    self.test_component(*id);
                }
            }
            drop_menu(ui, "asm_boards", &menu, |ui| {
                for (id, name) in &boards {
                    if menu_entry(ui, Icon::PcbBoard, &format!("{name} · {id}"), true, false).clicked() {
                        self.test_component(*id);
                    }
                }
            });
            let has_device = self.asm.as_ref().unwrap().data.parts.iter().any(|p| p.device.is_some());
            if large(ui, Icon::PcbErc, "Schaltung\ntesten", free && has_device, false).on_hover_text("Pins verdrahten, Versorgung einschalten, Taster drücken: LEDs leuchten im Modell").clicked() {
                self.open_circuit();
            }
            let target = self.strength_target();
            if large(ui, Icon::Simulate, "Festigkeit\nprüfen", free && target.is_some(), false)
                .on_hover_text("Ausgewähltes Teil mit Lagerung und Lasten aus der Baugruppe in der Live-Simulation prüfen")
                .clicked()
            {
                self.open_strength(target.unwrap());
            }
            if large(ui, Icon::Interference, "Durchdringung\nprüfen", free && self.asm.as_ref().unwrap().data.components.len() > 1, false).clicked() {
                self.run_interference();
            }
        });
        group(ui, "Baugruppe", |ui| {
            stack(ui, |ui| {
                if small(ui, Icon::Bom, "Stückliste", free, false).clicked() {
                    self.asm.as_mut().unwrap().show_bom = true;
                }
                if small(ui, Icon::Explode, "Explosionsansicht", free, false).clicked() {
                    self.toggle_exploded();
                }
                if small(ui, Icon::Close, "Assembler schließen", free, false).clicked() {
                    let tool = &mut self.asm.as_mut().unwrap().assembler;
                    tool.active = false;
                    tool.screws = false;
                    tool.bench = None;
                    tool.library = None;
                    tool.circuit = None;
                    tool.strength = None;
                }
            });
        });
    }

    pub fn assembler_windows(&mut self, ctx: &egui::Context) {
        if self.pcb.open {
            if self.pcb.bench.as_mut().is_some_and(|b| !b.show(ctx)) {
                self.pcb.bench = None;
            }
            return;
        }
        if !self.in_asm() || self.placing() || self.drawing_active() {
            return;
        }
        self.library_window(ctx);
        self.circuit_window(ctx);
        self.strength_window(ctx);
        let a = &mut self.asm.as_mut().unwrap().assembler;
        if a.bench.as_mut().is_some_and(|b| !b.show(ctx)) {
            a.bench = None;
        }
        if !a.active || !a.screws || self.tab != Tab::Tools {
            return;
        }
        let mut open = true;
        let mut insert = false;
        egui::Window::new("Schraube einbauen").collapsible(false).resizable(false).open(&mut open).show(ctx, |ui| {
            egui::Grid::new("asm_screw").spacing([14.0, 8.0]).show(ui, |ui| {
                ui.label("Kopf / Antrieb");
                egui::ComboBox::from_id_salt("asm_head").selected_text(a.head.label()).show_ui(ui, |ui| {
                    for h in Head::ALL {
                        ui.selectable_value(&mut a.head, h, h.label());
                    }
                });
                ui.end_row();
                ui.label("Gewinde");
                egui::ComboBox::from_id_salt("asm_thread").selected_text(ISO[a.thread].name).show_ui(ui, |ui| {
                    for (i, spec) in ISO.iter().enumerate() {
                        ui.selectable_value(&mut a.thread, i, format!("{} × {} mm", spec.name, spec.pitch));
                    }
                });
                ui.end_row();
                ui.label("Länge unter Kopf");
                ui.add(egui::DragValue::new(&mut a.length).range(1..=200).suffix(" mm"));
                ui.end_row();
            });
            ui.separator();
            insert = ui.button("Platzieren").clicked();
        });
        a.screws = open && !insert;
        if insert {
            a.screw_used = true;
            let (t, l, h) = (a.thread, a.length, a.head);
            self.assembler_place_screw(t, l, h);
        }
    }
}
