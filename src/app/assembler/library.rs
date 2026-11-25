//! Teilebibliothek: built-in boards, electronic parts, fasteners and enclosures, inserted
//! and placed like any other part.
use super::*;
use crate::model::library::{self, Category, Item};
use crate::pcb::devices::{ohm_label, BoardKind, Kind};

pub struct LibraryUi {
    pub cat: Category,
    pub query: String,
    /// the catalog with the values chosen in the window (resistor value, sizes …)
    pub items: Vec<Item>,
}

impl Default for LibraryUi {
    fn default() -> Self {
        LibraryUi { cat: Category::Boards, query: String::new(), items: library::catalog() }
    }
}

const OHMS: [u32; 14] = [10, 47, 100, 150, 220, 330, 470, 680, 1000, 2200, 4700, 10_000, 47_000, 100_000];
const STANDOFFS: [u16; 8] = [5, 6, 8, 10, 12, 15, 20, 25];

impl App {
    /// Builds a library part and starts placing it.
    pub fn assembler_insert(&mut self, item: Item) {
        if !self.in_asm() || self.busy() {
            return;
        }
        if item == Item::Screw {
            let a = self.asm.as_mut().unwrap();
            a.assembler.active = true;
            a.assembler.screws = true;
            return;
        }
        match item.build() {
            Ok(lp) => {
                // the module boards keep their pad table for the GPIO bench
                let board = match item {
                    Item::Device(Kind::Board(BoardKind::Esp32DevKit)) => Some(Module::Esp32.board()),
                    Item::Device(Kind::Board(BoardKind::Pi4)) => Some(Module::Pi4.board()),
                    _ => None,
                };
                self.asm.as_mut().unwrap().assembler.active = true;
                self.stage_device_part(lp.data, None, board.map(Arc::new), lp.device);
            }
            Err(e) => self.say(e, MsgKind::Error),
        }
    }

    pub fn open_library(&mut self) {
        if let Some(a) = &mut self.asm {
            if a.assembler.library.is_none() {
                a.assembler.library = Some(LibraryUi::default());
            }
        }
    }

    pub(super) fn library_window(&mut self, ctx: &egui::Context) {
        let Some(lib) = self.asm.as_mut().and_then(|a| a.assembler.library.as_mut()) else { return };
        let mut open = true;
        let mut insert = None;
        egui::Window::new("Teilebibliothek").id(egui::Id::new("asm_library")).open(&mut open).default_size([430.0, 520.0]).show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                for c in Category::ALL {
                    ui.selectable_value(&mut lib.cat, c, c.label());
                }
            });
            ui.horizontal(|ui| {
                ui.label("Suchen");
                ui.text_edit_singleline(&mut lib.query);
            });
            ui.separator();
            let q = lib.query.trim().to_lowercase();
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                for (i, item) in lib.items.iter_mut().enumerate() {
                    let shown = if q.is_empty() { item.category() == lib.cat } else { item.name().to_lowercase().contains(&q) || item.description().to_lowercase().contains(&q) };
                    if !shown {
                        continue;
                    }
                    ui.push_id(i, |ui| {
                        egui::Frame::group(ui.style()).show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.strong(item.name());
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.button("Einbauen").clicked() {
                                        insert = Some(*item);
                                    }
                                });
                            });
                            ui.small(item.description());
                            item_params(ui, item);
                        });
                    });
                }
            });
        });
        if !open {
            self.asm.as_mut().unwrap().assembler.library = None;
        }
        if let Some(item) = insert {
            self.assembler_insert(item);
        }
    }
}

fn item_params(ui: &mut egui::Ui, item: &mut Item) {
    match item {
        Item::Device(Kind::Resistor(r)) => {
            egui::ComboBox::from_id_salt("ohms").selected_text(ohm_label(*r)).show_ui(ui, |ui| {
                for v in OHMS {
                    ui.selectable_value(r, v, ohm_label(v));
                }
            });
        }
        Item::Device(Kind::Header(n)) => {
            ui.add(egui::DragValue::new(n).range(1..=40).prefix("Pins "));
        }
        Item::Box { w, d, h, wall } => {
            ui.horizontal(|ui| {
                for (v, label) in [(w, "B "), (d, "T "), (h, "H ")] {
                    ui.add(egui::DragValue::new(v).range(10.0..=500.0).prefix(label).suffix(" mm"));
                }
                ui.add(egui::DragValue::new(wall).range(0.8..=10.0).speed(0.1).prefix("Wand ").suffix(" mm"));
            });
        }
        Item::Lid { w, d, wall, led, button } => {
            ui.horizontal(|ui| {
                ui.add(egui::DragValue::new(w).range(10.0..=500.0).prefix("B ").suffix(" mm"));
                ui.add(egui::DragValue::new(d).range(10.0..=500.0).prefix("T ").suffix(" mm"));
                ui.add(egui::DragValue::new(wall).range(0.8..=10.0).speed(0.1).prefix("Dicke ").suffix(" mm"));
            });
            ui.horizontal(|ui| {
                ui.checkbox(led, "LED-Bohrung");
                ui.checkbox(button, "Tasterbohrung");
            });
        }
        Item::Nut(i) => {
            egui::ComboBox::from_id_salt("nut").selected_text(ISO[*i].name).show_ui(ui, |ui| {
                for (k, spec) in ISO.iter().enumerate().take(8) {
                    ui.selectable_value(i, k, spec.name);
                }
            });
        }
        Item::Standoff(l) => {
            egui::ComboBox::from_id_salt("standoff").selected_text(format!("{l} mm")).show_ui(ui, |ui| {
                for v in STANDOFFS {
                    ui.selectable_value(l, v, format!("{v} mm"));
                }
            });
        }
        _ => {}
    }
}
