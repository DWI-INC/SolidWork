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
    pub thumbs: Thumbs,
}

impl Default for LibraryUi {
    fn default() -> Self {
        LibraryUi { cat: Category::Boards, query: String::new(), items: library::catalog(), thumbs: Thumbs::default() }
    }
}

const THUMB: [usize; 2] = [320, 240];

/// Preview pictures of the library parts: rendered from the real part model on worker
/// threads while the window is open, then kept as textures (by item name).
#[derive(Default)]
pub struct Thumbs {
    done: Arc<std::sync::Mutex<Vec<(String, egui::ColorImage)>>>,
    textures: std::collections::HashMap<String, egui::TextureHandle>,
    requested: std::collections::HashSet<String>,
    queue: Option<std::sync::mpsc::Sender<Item>>,
}

impl Thumbs {
    /// The preview of `item`, asking for it to be rendered if it is not there yet.
    fn get(&mut self, ctx: &egui::Context, item: Item) -> Option<egui::TextureHandle> {
        for (name, img) in self.done.lock().unwrap().drain(..) {
            let tex = ctx.load_texture(format!("lib:{name}"), img, egui::TextureOptions::LINEAR);
            self.textures.insert(name, tex);
        }
        let name = item.name();
        if let Some(t) = self.textures.get(&name) {
            return Some(t.clone());
        }
        if self.requested.insert(name) {
            let tx = self.queue.get_or_insert_with(|| spawn_renderers(ctx.clone(), self.done.clone()));
            let _ = tx.send(item);
        }
        None
    }
}

fn spawn_renderers(ctx: egui::Context, done: Arc<std::sync::Mutex<Vec<(String, egui::ColorImage)>>>) -> std::sync::mpsc::Sender<Item> {
    let (tx, rx) = std::sync::mpsc::channel::<Item>();
    let rx = Arc::new(std::sync::Mutex::new(rx));
    let workers = std::thread::available_parallelism().map_or(2, |n| n.get().clamp(1, 4));
    for _ in 0..workers {
        let (rx, done, ctx) = (rx.clone(), done.clone(), ctx.clone());
        std::thread::spawn(move || loop {
            let Ok(item) = rx.lock().unwrap().recv() else { return };
            let Ok(lp) = item.build() else { continue };
            let r = crate::model::regen::run(&lp.data);
            let img = crate::softrender::render_part(&lp.data, &r, THUMB[0], THUMB[1], crate::kernel::math::V3::new(0.55, 1.0, 1.05));
            let rgb: Vec<u8> = img.rgb.iter().flatten().copied().collect();
            done.lock().unwrap().push((item.name(), egui::ColorImage::from_rgb(THUMB, &rgb)));
            ctx.request_repaint();
        });
    }
    tx
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
        egui::Window::new("Teilebibliothek").id(egui::Id::new("asm_library")).open(&mut open).default_size([520.0, 620.0]).show(ctx, |ui| {
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
                                let size = egui::vec2(112.0, 84.0);
                                let thumb = if *item == Item::Screw { None } else { lib.thumbs.get(ctx, *item) };
                                match thumb {
                                    Some(tex) => {
                                        let r = ui.add(egui::Image::new(&tex).fit_to_exact_size(size).corner_radius(4.0).sense(egui::Sense::click()));
                                        if r.double_clicked() {
                                            insert = Some(*item);
                                        }
                                        r.on_hover_ui(|ui| {
                                            ui.add(egui::Image::new(&tex).fit_to_exact_size(egui::vec2(THUMB[0] as f32, THUMB[1] as f32)));
                                        });
                                    }
                                    None => {
                                        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                                        ui.painter().rect_filled(rect, 4.0, ui.visuals().faint_bg_color);
                                        if *item != Item::Screw {
                                            egui::Spinner::new().paint_at(ui, egui::Rect::from_center_size(rect.center(), egui::vec2(18.0, 18.0)));
                                        }
                                    }
                                }
                                ui.vertical(|ui| {
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
