//! Schaltung testen: wires between the pins of library parts, the boards' pin programs and
//! the DC simulation of the assembled product. Lit LEDs glow in the viewport, wires are
//! drawn between the pins, and buttons can be pressed by clicking them in the model.
use super::*;
use crate::kernel::topology::BODY_SHIFT;
use crate::pcb::circuit::{self, Mode, PinLevel, PinRef, Solution, Stimulus};
use crate::pcb::devices::{Device, Kind};
use std::collections::HashMap;

pub struct CircuitUi {
    pub stim: Stimulus,
    /// simulation running: time advances, LEDs glow
    pub running: bool,
    started: std::time::Instant,
    /// pick wire ends at the pins in the graphics window
    pub picking: bool,
    /// first end of the wire being drawn
    pub first: Option<PinRef>,
    add: (u64, String, u64, String),
    prog: (u64, String, usize, u32, String, bool),
    filter: String,
    pub sol: Option<Solution>,
    /// simulate without showing the window (demo views)
    pub hidden: bool,
}

impl Default for CircuitUi {
    fn default() -> Self {
        CircuitUi {
            stim: Stimulus::default(),
            running: true,
            started: std::time::Instant::now(),
            picking: false,
            first: None,
            add: (0, String::new(), 0, String::new()),
            prog: (0, String::new(), 2, 500, String::new(), false),
            filter: String::new(),
            sol: None,
            hidden: false,
        }
    }
}

const MODES: [&str; 6] = ["Eingang", "Eingang mit Pull-up", "Ausgang HIGH", "Ausgang LOW", "Blinken", "Folgt Eingang"];

fn mode_of(k: usize, ms: u32, input: &str, invert: bool) -> Mode {
    match k {
        0 => Mode::Input,
        1 => Mode::Pullup,
        2 => Mode::High,
        3 => Mode::Low,
        4 => Mode::Blink(ms.max(10)),
        _ => Mode::Follow { input: input.into(), invert },
    }
}

fn wire_color(level: Option<PinLevel>) -> Color32 {
    match level {
        Some(PinLevel::Volts(v)) if v >= 4.4 => Color32::from_rgb(214, 48, 40),
        Some(PinLevel::Volts(v)) if v >= 2.9 => Color32::from_rgb(236, 140, 30),
        Some(PinLevel::Volts(v)) if v.abs() < 0.05 => Color32::from_rgb(40, 40, 46),
        Some(PinLevel::Volts(_)) => Color32::from_rgb(220, 200, 40),
        _ => Color32::from_rgb(40, 110, 220),
    }
}

impl App {
    /// The library electronics in the assembly: (component, label, device).
    pub fn circuit_parts(&self) -> Vec<(u64, String, Device)> {
        let Some(a) = &self.asm else { return vec![] };
        a.data
            .components
            .iter()
            .filter(|c| !c.suppressed)
            .filter_map(|c| a.data.part(&c.part).and_then(|p| p.device.clone()).map(|d| (c.id, format!("{} · {}", c.part, c.id), d)))
            .collect()
    }

    pub fn open_circuit(&mut self) {
        if let Some(a) = &mut self.asm {
            if a.assembler.circuit.is_none() {
                a.assembler.circuit = Some(CircuitUi::default());
            }
        }
        self.circuit_update();
    }

    /// Solves the circuit and updates the glowing LEDs (once per frame while the window is open).
    pub fn circuit_update(&mut self) {
        let parts = self.circuit_parts();
        let Some(a) = &mut self.asm else { return };
        let Some(ui) = &mut a.assembler.circuit else {
            if !a.glow.is_empty() {
                a.glow.clear();
                a.res.display.generation = regen::next_generation();
            }
            return;
        };
        if ui.running {
            ui.stim.time = ui.started.elapsed().as_secs_f64();
        }
        let list: Vec<circuit::Part> = parts.iter().map(|(id, label, d)| circuit::Part { id: *id, label: label.clone(), device: d }).collect();
        let sol = circuit::simulate(&list, &a.data.wiring, &ui.stim);
        let mut glow = HashMap::new();
        if ui.running {
            for (id, _, dev) in &parts {
                let (Some(c), Some(span)) = (a.data.component(*id), a.res.spans.get(id)) else { continue };
                let Some(res) = a.res.parts.get(&c.part) else { continue };
                let Some(entry) = a.data.part(&c.part) else { continue };
                for lamp in &dev.lamps {
                    let Some(&b) = sol.lamps.get(&(*id, lamp.name.clone())) else { continue };
                    let off = entry.data.feature_colors.get(&lamp.feature).copied().unwrap_or([0.5; 3]);
                    let col: [f32; 3] = std::array::from_fn(|k| 2.0 + off[k] + (lamp.on[k] - off[k]) * b);
                    for (&tag, &f) in &res.tag_owner {
                        if f == lamp.feature {
                            glow.insert(span.body | (tag & ((1 << BODY_SHIFT) - 1)), col);
                        }
                    }
                }
            }
        }
        if glow != a.glow {
            a.glow = glow;
            a.res.display.generation = regen::next_generation();
        }
        ui.sol = Some(sol);
    }

    /// Where a pin of a component is in the (displayed) assembly.
    fn pin_world(&self, comp: u64, pin: &str) -> Option<V3> {
        let a = self.asm.as_ref()?;
        let c = a.data.component(comp)?;
        let dev = a.data.part(&c.part)?.device.as_ref()?;
        let q = dev.pin(pin)?.pos;
        Some(a.res.shown.get(&comp)?.point(V3::new(q[0], q[1], q[2])))
    }

    fn pins_on_screen(&self, to_screen: &dyn Fn(V3) -> Option<Pos2>) -> Vec<(PinRef, Pos2)> {
        let mut v = vec![];
        for (id, _, dev) in self.circuit_parts() {
            for p in &dev.pins {
                if let Some(s) = self.pin_world(id, &p.name).and_then(|w| to_screen(w)) {
                    v.push((PinRef::new(id, &p.name), s));
                }
            }
        }
        v
    }

    /// Wires, and the pins while they are being picked.
    pub(in crate::app) fn draw_circuit(&self, painter: &egui::Painter, to_screen: &dyn Fn(V3) -> Option<Pos2>) {
        let Some(ui) = self.asm.as_ref().filter(|_| self.in_asm() && !self.placing()).and_then(|a| a.assembler.circuit.as_ref()) else { return };
        let a = self.asm.as_ref().unwrap();
        for w in &a.data.wiring.wires {
            let (Some(p), Some(q)) = (self.pin_world(w.a.comp, &w.a.pin).and_then(|x| to_screen(x)), self.pin_world(w.b.comp, &w.b.pin).and_then(|x| to_screen(x))) else { continue };
            let level = ui.sol.as_ref().map(|s| s.level(w.a.comp, &w.a.pin));
            let col = wire_color(level);
            painter.line_segment([p, q], Stroke::new(4.5_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 140)));
            painter.line_segment([p, q], Stroke::new(2.5_f32, col));
            for e in [p, q] {
                painter.circle_filled(e, 3.2, col);
            }
        }
        if ui.picking {
            let hover = painter.ctx().pointer_hover_pos();
            let pins = self.pins_on_screen(to_screen);
            let near = hover.and_then(|h| pins.iter().filter(|(_, s)| s.distance(h) < 10.0).min_by(|x, y| x.1.distance(h).partial_cmp(&y.1.distance(h)).unwrap()));
            for (r, s) in &pins {
                let first = ui.first.as_ref() == Some(r);
                painter.circle_stroke(*s, if first { 5.0 } else { 3.0 }, Stroke::new(1.5_f32, if first { Color32::from_rgb(255, 180, 0) } else { Color32::from_rgb(40, 180, 230) }));
            }
            if let Some((r, s)) = near {
                painter.circle_filled(*s, 4.5, Color32::from_rgb(255, 210, 40));
                let label = format!("{} · {}", self.comp_label(r.comp), r.pin);
                painter.text(*s + egui::vec2(8.0, -8.0), egui::Align2::LEFT_BOTTOM, label, egui::FontId::proportional(13.0), Color32::from_rgb(20, 20, 20));
                if let (Some(f), Some(h)) = (&ui.first, hover) {
                    if let Some(fs) = self.pin_world(f.comp, &f.pin).and_then(|w| to_screen(w)) {
                        painter.line_segment([fs, h], Stroke::new(2.0_f32, Color32::from_rgb(255, 180, 0)));
                    }
                }
            } else if let (Some(f), Some(h)) = (&ui.first, hover) {
                if let Some(fs) = self.pin_world(f.comp, &f.pin).and_then(|w| to_screen(w)) {
                    painter.line_segment([fs, h], Stroke::new(1.5_f32, Color32::from_rgb(255, 180, 0)));
                }
            }
        }
    }

    /// Wire ends are being picked in the graphics window.
    pub fn circuit_picking(&self) -> bool {
        self.in_asm() && self.asm.as_ref().and_then(|a| a.assembler.circuit.as_ref()).is_some_and(|c| c.picking)
    }

    /// A click in the graphics window: wire ends while picking, otherwise pressing buttons.
    pub(in crate::app) fn circuit_click(&mut self, local: (f32, f32)) -> bool {
        if !self.in_asm() || self.placing() {
            return false;
        }
        let Some(ui) = self.asm.as_ref().and_then(|a| a.assembler.circuit.as_ref()) else { return false };
        let picking = ui.picking;
        if picking {
            let proj = self.camera.projector(self.viewport_size);
            let to = |p: V3| proj.project(p).map(|s| Pos2::new(s.0, s.1));
            let at = Pos2::new(local.0, local.1);
            let hit = self.pins_on_screen(&to).into_iter().filter(|(_, s)| s.distance(at) < 10.0).min_by(|x, y| x.1.distance(at).partial_cmp(&y.1.distance(at)).unwrap()).map(|x| x.0);
            let Some(hit) = hit else { return true };
            let first = self.asm.as_mut().unwrap().assembler.circuit.as_mut().unwrap().first.take();
            match first {
                None => self.asm.as_mut().unwrap().assembler.circuit.as_mut().unwrap().first = Some(hit),
                Some(f) if f == hit => {}
                Some(f) => self.circuit_connect(f, hit),
            }
            return true;
        }
        // press or release a button by clicking it
        let Some(comp) = self.pick_at(local).and_then(|p| self.owner(p)) else { return false };
        let kind = self.circuit_parts().into_iter().find(|x| x.0 == comp).map(|x| x.2.kind);
        if matches!(kind, Some(Kind::Tact6 | Kind::Tact12)) {
            let ui = self.asm.as_mut().unwrap().assembler.circuit.as_mut().unwrap();
            let key = (comp, String::new());
            if !ui.stim.pressed.remove(&key) {
                ui.stim.pressed.insert(key);
            }
            return true;
        }
        false
    }

    pub fn circuit_connect(&mut self, a: PinRef, b: PinRef) {
        let label = format!("{} {} mit {} {} verbunden.", self.comp_label(a.comp), a.pin, self.comp_label(b.comp), b.pin);
        let exists = self.asm.as_ref().is_some_and(|s| s.data.wiring.wires.iter().any(|w| (w.a == a && w.b == b) || (w.a == b && w.b == a)));
        if a == b || exists {
            return;
        }
        self.asm_commit(&label, move |d| {
            d.wiring.connect(a, b);
        });
    }

    fn circuit_set_mode(&mut self, comp: u64, pin: String, mode: Mode) {
        let label = format!("{} {}: {}.", self.comp_label(comp), pin, mode.label());
        self.asm_commit(&label, move |d| d.wiring.set_mode(comp, &pin, mode));
    }

    pub(super) fn circuit_window(&mut self, ctx: &egui::Context) {
        if self.asm.as_ref().is_none_or(|a| a.assembler.circuit.is_none()) {
            return;
        }
        self.circuit_update();
        let parts = self.circuit_parts();
        let wiring = self.asm.as_ref().unwrap().data.wiring.clone();
        let mut open = true;
        let mut connect = None;
        let mut remove_wire = None;
        let mut set_mode = None;
        let ui_state = self.asm.as_mut().unwrap().assembler.circuit.as_mut().unwrap();
        let label_of = |id: u64| parts.iter().find(|p| p.0 == id).map(|p| p.1.clone()).unwrap_or_else(|| format!("{id}"));
        if ui_state.running && wiring.programs.iter().any(|p| matches!(p.mode, Mode::Blink(_))) {
            ctx.request_repaint_after(std::time::Duration::from_millis(40));
        }
        if ui_state.hidden {
            return;
        }
        egui::Window::new("Schaltung testen").id(egui::Id::new("asm_circuit")).open(&mut open).default_size([480.0, 560.0]).default_pos(egui::pos2(ctx.screen_rect().right() - 500.0, 240.0)).show(ctx, |ui| {
            if parts.is_empty() {
                ui.label("Die Baugruppe enthält noch keine Elektronik. Mikrocontroller, LEDs, Taster und Batterien aus der Teilebibliothek einbauen.");
                return;
            }
            ui.horizontal(|ui| {
                ui.checkbox(&mut ui_state.running, "Simulation läuft").on_hover_text("LEDs leuchten im Modell, Blinkprogramme laufen");
                if ui.selectable_label(ui_state.picking, "Pins im Modell verbinden").on_hover_text("Erst einen Pin, dann den zweiten im Grafikfenster anklicken").clicked() {
                    ui_state.picking = !ui_state.picking;
                    ui_state.first = None;
                }
            });
            if ui_state.picking {
                ui.small(match &ui_state.first {
                    None => "Ersten Pin anklicken.".to_string(),
                    Some(f) => format!("{} · {} gewählt – zweiten Pin anklicken.", label_of(f.comp), f.pin),
                });
            } else if ui_state.running {
                ui.small("Taster lassen sich im Modell anklicken.");
            }
            ui.separator();
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                egui::CollapsingHeader::new("Versorgung und Bedienung").default_open(true).show(ui, |ui| {
                    for (id, label, dev) in &parts {
                        match dev.kind {
                            Kind::Board(b) => {
                                let mut on = ui_state.stim.usb.contains(id);
                                if ui.checkbox(&mut on, format!("USB · {label}")).changed() {
                                    if on {
                                        ui_state.stim.usb.insert(*id);
                                    } else {
                                        ui_state.stim.usb.remove(id);
                                    }
                                }
                                if let Some((name, _)) = b.spec().button {
                                    let key = (*id, name.to_string());
                                    let mut down = ui_state.stim.pressed.contains(&key);
                                    if ui.toggle_value(&mut down, format!("Taster {name} · {label}")).changed() {
                                        if down {
                                            ui_state.stim.pressed.insert(key);
                                        } else {
                                            ui_state.stim.pressed.remove(&key);
                                        }
                                    }
                                }
                            }
                            Kind::UsbC | Kind::MicroUsb => {
                                let mut on = ui_state.stim.usb.contains(id);
                                if ui.checkbox(&mut on, format!("Kabel · {label}")).changed() {
                                    if on {
                                        ui_state.stim.usb.insert(*id);
                                    } else {
                                        ui_state.stim.usb.remove(id);
                                    }
                                }
                            }
                            Kind::Battery9V => {
                                let mut inside = !ui_state.stim.removed.contains(id);
                                if ui.checkbox(&mut inside, format!("Eingesetzt · {label}")).changed() {
                                    if inside {
                                        ui_state.stim.removed.remove(id);
                                    } else {
                                        ui_state.stim.removed.insert(*id);
                                    }
                                }
                            }
                            Kind::Tact6 | Kind::Tact12 => {
                                let key = (*id, String::new());
                                let mut down = ui_state.stim.pressed.contains(&key);
                                if ui.toggle_value(&mut down, format!("Drücken · {label}")).changed() {
                                    if down {
                                        ui_state.stim.pressed.insert(key);
                                    } else {
                                        ui_state.stim.pressed.remove(&key);
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                });
                egui::CollapsingHeader::new(format!("Verdrahtung ({})", wiring.wires.len())).default_open(true).show(ui, |ui| {
                    for (i, w) in wiring.wires.iter().enumerate() {
                        ui.horizontal(|ui| {
                            let col = wire_color(ui_state.sol.as_ref().map(|s| s.level(w.a.comp, &w.a.pin)));
                            ui.colored_label(col, "●");
                            ui.label(format!("{} {}  ↔  {} {}", label_of(w.a.comp), w.a.pin, label_of(w.b.comp), w.b.pin));
                            if ui.small_button("✕").on_hover_text("Draht entfernen").clicked() {
                                remove_wire = Some(i);
                            }
                        });
                    }
                    let add = &mut ui_state.add;
                    let pin_combo = |ui: &mut egui::Ui, salt: &str, comp: &mut u64, pin: &mut String| {
                        egui::ComboBox::from_id_salt(format!("{salt}_c")).width(150.0).selected_text(label_of(*comp)).show_ui(ui, |ui| {
                            for (id, label, _) in &parts {
                                ui.selectable_value(comp, *id, label);
                            }
                        });
                        let names = parts.iter().find(|p| p.0 == *comp).map(|p| p.2.pin_names()).unwrap_or_default();
                        egui::ComboBox::from_id_salt(format!("{salt}_p")).width(70.0).selected_text(pin.as_str()).show_ui(ui, |ui| {
                            for n in names {
                                ui.selectable_value(pin, n.clone(), n);
                            }
                        });
                    };
                    ui.horizontal(|ui| {
                        pin_combo(ui, "wa", &mut add.0, &mut add.1);
                        ui.label("↔");
                    });
                    ui.horizontal(|ui| {
                        pin_combo(ui, "wb", &mut add.2, &mut add.3);
                        if ui.button("Verbinden").clicked() && !add.1.is_empty() && !add.3.is_empty() {
                            connect = Some((PinRef::new(add.0, &add.1), PinRef::new(add.2, &add.3)));
                        }
                    });
                });
                let boards: Vec<_> = parts.iter().filter(|p| matches!(p.2.kind, Kind::Board(_))).collect();
                if !boards.is_empty() {
                    egui::CollapsingHeader::new("Programm der Mikrocontroller").default_open(true).show(ui, |ui| {
                        for p in &wiring.programs {
                            ui.horizontal(|ui| {
                                ui.label(format!("{} · {}: {}", label_of(p.comp), p.pin, p.mode.label()));
                                if ui.small_button("✕").clicked() {
                                    set_mode = Some((p.comp, p.pin.clone(), Mode::Input));
                                }
                            });
                        }
                        let prog = &mut ui_state.prog;
                        if !boards.iter().any(|b| b.0 == prog.0) {
                            prog.0 = boards[0].0;
                        }
                        let (dev_kind, names) = boards.iter().find(|b| b.0 == prog.0).map(|b| {
                            let Kind::Board(k) = b.2.kind else { unreachable!() };
                            let mut n: Vec<String> = b.2.pin_names().into_iter().filter(|n| k.is_gpio(n)).collect();
                            n.extend(k.spec().internal.iter().map(|s| s.to_string()));
                            (k, n)
                        }).unwrap();
                        ui.horizontal(|ui| {
                            egui::ComboBox::from_id_salt("pg_c").width(150.0).selected_text(label_of(prog.0)).show_ui(ui, |ui| {
                                for b in &boards {
                                    ui.selectable_value(&mut prog.0, b.0, &b.1);
                                }
                            });
                            egui::ComboBox::from_id_salt("pg_p").width(70.0).selected_text(prog.1.as_str()).show_ui(ui, |ui| {
                                for n in &names {
                                    ui.selectable_value(&mut prog.1, n.clone(), n);
                                }
                            });
                            egui::ComboBox::from_id_salt("pg_m").width(130.0).selected_text(MODES[prog.2]).show_ui(ui, |ui| {
                                for (k, m) in MODES.iter().enumerate() {
                                    ui.selectable_value(&mut prog.2, k, *m);
                                }
                            });
                        });
                        ui.horizontal(|ui| {
                            if prog.2 == 4 {
                                ui.add(egui::DragValue::new(&mut prog.3).range(10..=10_000).suffix(" ms"));
                            }
                            if prog.2 == 5 {
                                egui::ComboBox::from_id_salt("pg_in").width(70.0).selected_text(prog.4.as_str()).show_ui(ui, |ui| {
                                    for n in &names {
                                        ui.selectable_value(&mut prog.4, n.clone(), n);
                                    }
                                });
                                ui.checkbox(&mut prog.5, "invertiert");
                            }
                            let output = prog.2 >= 2;
                            let ok = !prog.1.is_empty() && !(output && dev_kind.input_only(&prog.1)) && (prog.2 != 5 || !prog.4.is_empty());
                            if ui.add_enabled(ok, egui::Button::new("Übernehmen")).clicked() {
                                set_mode = Some((prog.0, prog.1.clone(), mode_of(prog.2, prog.3, &prog.4, prog.5)));
                            }
                        });
                    });
                }
                if let Some(sol) = &ui_state.sol {
                    egui::CollapsingHeader::new("Ergebnis").default_open(true).show(ui, |ui| {
                        for w in &sol.warnings {
                            ui.colored_label(Color32::from_rgb(190, 61, 40), format!("⚠ {w}"));
                        }
                        egui::Grid::new("circuit_status").striped(true).spacing([12.0, 3.0]).show(ui, |ui| {
                            for (id, text) in &sol.status {
                                ui.label(label_of(*id));
                                ui.label(text);
                                ui.end_row();
                            }
                        });
                        ui.add_space(4.0);
                        ui.horizontal(|ui| {
                            ui.label("Messpunkte");
                            ui.text_edit_singleline(&mut ui_state.filter);
                        });
                        let q = ui_state.filter.trim().to_lowercase();
                        if !q.is_empty() {
                            let mut rows: Vec<_> = sol.pins.iter().filter(|(r, _)| format!("{} {}", label_of(r.comp), r.pin).to_lowercase().contains(&q)).collect();
                            rows.sort_by(|a, b| (a.0.comp, &a.0.pin).cmp(&(b.0.comp, &b.0.pin)));
                            egui::Grid::new("circuit_pins").striped(true).show(ui, |ui| {
                                for (r, l) in rows.into_iter().take(60) {
                                    ui.label(format!("{} · {}", label_of(r.comp), r.pin));
                                    ui.label(l.label());
                                    ui.end_row();
                                }
                            });
                        }
                    });
                }
            });
        });
        if !open {
            self.asm.as_mut().unwrap().assembler.circuit = None;
            self.circuit_update();
            return;
        }
        if let Some((a, b)) = connect {
            self.circuit_connect(a, b);
        }
        if let Some(i) = remove_wire {
            self.asm_commit("Draht entfernt.", move |d| {
                if i < d.wiring.wires.len() {
                    d.wiring.wires.remove(i);
                }
            });
        }
        if let Some((c, p, m)) = set_mode {
            self.circuit_set_mode(c, p, m);
        }
    }
}
