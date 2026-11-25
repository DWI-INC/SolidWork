//! Development boards in top view. ESP32 pad numbers follow J2/J3 on Espressif's V4
//! drawing. The Pi uses the physical J8 numbers; its HAT carrier sits above the Pi.
use super::board::{Board, FpInst};
use super::footprints::{self, Footprint};
use super::geom::{p, P};
use super::model3d::{Mat, Model};
use super::symbols::{box_symbol, PinRow, PinType, Side, Symbol};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Module {
    Esp32,
    Pi4,
}

pub const MODULES: [Module; 2] = [Module::Esp32, Module::Pi4];

const ESP_J2: [&str; 19] =
    ["3V3", "EN", "GPIO36", "GPIO39", "GPIO34", "GPIO35", "GPIO32", "GPIO33", "GPIO25", "GPIO26", "GPIO27", "GPIO14", "GPIO12", "GND", "GPIO13", "D2", "D3", "CMD", "5V"];
const ESP_J3: [&str; 19] =
    ["GND", "GPIO23", "GPIO22", "GPIO1", "GPIO3", "GPIO21", "GND", "GPIO19", "GPIO18", "GPIO5", "GPIO17", "GPIO16", "GPIO4", "GPIO0", "GPIO2", "GPIO15", "D1", "D0", "CLK"];
const PI_J8: [&str; 40] = [
    "3V3", "5V", "GPIO2", "5V", "GPIO3", "GND", "GPIO4", "GPIO14", "GND", "GPIO15", "GPIO17", "GPIO18", "GPIO27", "GND", "GPIO22", "GPIO23", "3V3", "GPIO24", "GPIO10", "GND",
    "GPIO9", "GPIO25", "GPIO11", "GPIO8", "GND", "GPIO7", "ID_SD", "ID_SC", "GPIO5", "GND", "GPIO6", "GPIO12", "GPIO13", "GND", "GPIO19", "GPIO16", "GPIO26", "GPIO20", "GND",
    "GPIO21",
];

pub struct HeaderPin {
    pub number: String,
    pub name: &'static str,
    pub pos: P,
    pub kind: PinType,
    side: Side,
}

impl Module {
    pub fn id(self) -> &'static str {
        match self {
            Self::Esp32 => "Freo_Module:ESP32-DevKitC-V4-32E",
            Self::Pi4 => "Freo_Module:Raspberry-Pi-4B-4GB",
        }
    }
    pub fn of(fp: &Footprint) -> Option<Self> {
        MODULES.into_iter().find(|m| m.id() == fp.id)
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Esp32 => "ESP32-DevKitC V4",
            Self::Pi4 => "Raspberry Pi 4 Model B (4 GB)",
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::Esp32 => "ESP32-WROOM-32E, zwei 19-polige Stiftleisten, Micro-USB",
            Self::Pi4 => "4 GB LPDDR4, 40-poliger GPIO-Anschluss. HAT über dem Raspberry Pi, 11 mm Abstand.",
        }
    }
    pub fn size(self) -> P {
        match self {
            Self::Esp32 => p(27.94, 48.26),
            Self::Pi4 => p(85.0, 56.0),
        }
    }
    pub fn mounts(self) -> &'static [(f64, f64)] {
        match self {
            Self::Pi4 => &[(3.5, 3.5), (61.5, 3.5), (3.5, 52.5), (61.5, 52.5)],
            Self::Esp32 => &[],
        }
    }
    pub fn pins(self) -> Vec<HeaderPin> {
        use PinType::*;
        let headers: Vec<_> = match self {
            Self::Esp32 => [ESP_J2, ESP_J3]
                .iter()
                .enumerate()
                .flat_map(|(col, pins)| {
                    pins.iter().enumerate().map(move |(i, &name)| (format!("J{}.{}", col + 2, i + 1), name, p(1.24 + 25.4 * col as f64, 1.25 + 2.54 * i as f64), col))
                })
                .collect(),
            Self::Pi4 => PI_J8
                .iter()
                .enumerate()
                .map(|(i, &name)| ((i + 1).to_string(), name, p(32.5 - 9.5 * 2.54 + (i / 2) as f64 * 2.54, if i % 2 == 0 { 4.77 } else { 2.23 }), i % 2))
                .collect(),
        };
        headers
            .into_iter()
            .map(|(number, name, pos, col)| {
                let kind = match name {
                    "GND" | "5V" => PowerIn,
                    "3V3" if self == Self::Pi4 => PowerOut,
                    "3V3" => Passive, // regulated output or external supply
                    "EN" | "GPIO36" | "GPIO39" | "GPIO34" | "GPIO35" => Input,
                    _ if name.starts_with("GPIO") => Bidi,
                    _ => Passive, // flash bus or HAT ID pins
                };
                HeaderPin { number, name, pos, kind, side: if col == 0 { Side::Left } else { Side::Right } }
            })
            .collect()
    }
    pub fn symbol(self) -> Symbol {
        let rows: Vec<_> = self.pins().into_iter().map(|q| PinRow { num: q.number, name: q.name.into(), side: q.side, kind: q.kind }).collect();
        let mut s = box_symbol(self.id(), "U", self.name(), self.id(), self.description(), &rows);
        s.category = "Rechnermodule".into();
        s
    }

    /// Standalone board with the actual pin drills and mounting holes. The same pin table
    /// supplies the schematic, carrier land pattern and test bench.
    pub fn board(self) -> Board {
        let mut b = Board::default();
        b.outline = vec![self.outline()];
        let fp = footprints::library().iter().find(|f| f.id == self.id()).unwrap().clone();
        let nets = vec![String::new(); fp.pads.len()];
        b.footprints.push(FpInst { reference: "U1".into(), value: self.name().into(), fp, pos: P::ZERO, rot: 0.0, bottom: false, nets, locked: true });
        b
    }
    pub fn outline(self) -> Vec<P> {
        let size = self.size();
        if self == Self::Esp32 {
            return vec![P::ZERO, p(size.x, 0.0), size, p(0.0, size.y)];
        }
        let r = 3.0;
        [p(size.x - r, r), size - P::splat(r), p(r, size.y - r), P::splat(r)]
            .into_iter()
            .enumerate()
            .flat_map(|(i, c)| {
                (0..=6).map(move |j| {
                    let a = (i as f64 - 1.0 + j as f64 / 6.0) * std::f64::consts::FRAC_PI_2;
                    c + r * p(a.cos(), a.sin())
                })
            })
            .collect()
    }

    /// Nominal connector and component envelopes above the module's own laminate.
    pub fn hardware(self, surface: f64) -> Model {
        use super::model3d::{receptacle, Receptacle};
        let z = surface;
        let mut m = Model::default();
        // plated ring and solder joint around a through-hole pin on the top side
        let joint = |m: &mut Model, q: P| {
            m.annulus(Mat::Rgb(205, 205, 198), q, 0.45, 0.85, z + 0.02, 16, false);
            m.cone(Mat::Rgb(190, 192, 190), [q.x, q.y, z + 0.02], [q.x, q.y, z + 0.55], 0.62, 0.36, 12);
        };
        match self {
            Self::Esp32 => {
                // WROOM-32E: module laminate with the antenna overhanging the top edge
                let module_pcb = Mat::Rgb(40, 44, 50);
                m.boxx(module_pcb, [4.97, -6.04, z], [22.97, 19.46, z + 0.8]);
                for i in 0..10 {
                    // castellated pads down both sides
                    let y = 1.9 + i as f64 * 1.27 * 1.35;
                    for (x0, x1) in [(4.95, 5.6), (22.34, 22.99)] {
                        m.boxx(Mat::Gold, [x0, y - 0.45, z], [x1, y + 0.45, z + 0.82]);
                    }
                }
                // meandered PCB antenna
                let ant = Mat::Gold;
                let (top, bot) = (-5.3, -1.6);
                m.boxx(ant, [6.0, bot - 0.5, z + 0.8], [6.5, -0.6, z + 0.83]);
                let mut x = 6.0;
                for k in 0..8 {
                    m.boxx(ant, [x, top, z + 0.8], [x + 0.5, bot, z + 0.83]);
                    let y = if k % 2 == 0 { top } else { bot - 0.5 };
                    m.boxx(ant, [x, y, z + 0.8], [x + 2.0, y + 0.5, z + 0.83]);
                    x += 1.5;
                }
                m.boxx(ant, [x, top, z + 0.8], [x + 0.5, bot, z + 0.83]);
                // shielding can with a pressed edge
                m.rbox(Mat::Rgb(205, 207, 210), [5.2, 0.7, z + 0.8], [22.7, 19.1, z + 2.9], 0.5);
                m.body(Mat::Metal, [5.5, 1.0, z + 2.9], [22.4, 18.8, z + 3.2], 0.25);
                // Micro-USB at the bottom edge
                receptacle(&mut m, Receptacle::MicroB, p(13.97, 49.0), p(0.0, 1.0), z);
                // USB-UART bridge, regulator, EN / BOOT buttons, passives
                m.body(Mat::Plastic, [11.0, 31.0, z], [16.0, 36.0, z + 0.9], 0.08);
                m.cyl(Mat::Rgb(90, 90, 92), [11.6, 31.6, z + 0.9], [11.6, 31.6, z + 0.91], 0.25, 10);
                m.body(Mat::Plastic, [17.5, 23.0, z], [24.0, 26.5, z + 1.6], 0.15);
                m.boxx(Mat::Metal, [19.2, 21.6, z], [22.3, 23.0, z + 0.2]);
                for x in [18.0, 20.75, 23.5] {
                    m.boxx(Mat::Metal, [x - 0.35, 26.5, z], [x + 0.35, 27.6, z + 0.2]);
                }
                for x in [5.2, 20.0] {
                    m.boxx(Mat::Rgb(60, 62, 66), [x, 40.0, z], [x + 2.8, 44.5, z + 1.4]);
                    m.boxx(Mat::Metal, [x + 0.2, 40.2, z + 1.4], [x + 2.6, 44.3, z + 1.55]);
                    m.cyl(Mat::Plastic, [x + 1.4, 42.25, z + 1.55], [x + 1.4, 42.25, z + 2.3], 0.75, 16);
                }
                for (i, (x, y)) in [(9.0, 25.0), (9.0, 27.0), (19.0, 30.0), (19.0, 32.0), (9.0, 38.0), (18.0, 38.0), (12.0, 39.5)].into_iter().enumerate() {
                    let mat = if i % 2 == 0 { Mat::Ceramic } else { Mat::Plastic };
                    m.boxx(mat, [x - 0.8, y - 0.4, z], [x + 0.8, y + 0.4, z + 0.5]);
                    for sx in [-0.8, 0.5] {
                        m.boxx(Mat::Metal, [x + sx, y - 0.41, z], [x + sx + 0.3, y + 0.41, z + 0.51]);
                    }
                }
            }
            Self::Pi4 => {
                // GPIO header J8 and the PoE header
                m.boxx(Mat::Plastic, [7.1, 0.96, z], [57.9, 6.04, z + 2.5]);
                for i in 0..4 {
                    let q = p(76.0 + (i % 2) as f64 * 2.54, 3.0 + (i / 2) as f64 * 2.54);
                    m.boxx(Mat::Plastic, [q.x - 1.24, q.y - 1.24, z], [q.x + 1.24, q.y + 1.24, z + 2.5]);
                    m.boxx(Mat::Gold, [q.x - 0.32, q.y - 0.32, z], [q.x + 0.32, q.y + 0.32, z + 8.5]);
                }
                // right edge: USB 2.0 stack, USB 3.0 stack (blue tongues), Gigabit Ethernet
                for (y, tongue) in [(9.0, Mat::Plastic), (27.0, Mat::Rgb(30, 95, 210))] {
                    for level in 0..2 {
                        receptacle(&mut m, Receptacle::TypeA(tongue), p(87.0, y), p(1.0, 0.0), z + level as f64 * 7.8);
                    }
                }
                receptacle(&mut m, Receptacle::Rj45, p(87.1, 45.75), p(1.0, 0.0), z);
                // bottom edge: USB-C power, two micro-HDMI, A/V jack
                receptacle(&mut m, Receptacle::TypeC, p(11.2, 57.3), p(0.0, 1.0), z);
                for x in [26.0, 39.5] {
                    receptacle(&mut m, Receptacle::MicroHdmi, p(x, 57.0), p(0.0, 1.0), z);
                }
                m.rbox(Mat::Plastic, [50.6, 45.0, z], [56.4, 57.2, z + 5.9], 0.3);
                m.cyl(Mat::Plastic, [53.5, 57.2, z + 3.0], [53.5, 59.5, z + 3.0], 3.0, 32);
                m.cyl(Mat::Rgb(18, 18, 20), [53.5, 59.5, z + 3.0], [53.5, 59.52, z + 3.0], 1.8, 24);
                // SoC with heat spreader, LPDDR4, USB controller, PMIC, Wi-Fi can
                m.boxx(Mat::Rgb(20, 70, 40), [21.4, 15.8, z], [36.8, 31.2, z + 1.0]);
                m.rbox(Mat::Metal, [22.0, 16.4, z + 1.0], [36.2, 30.6, z + 2.4], 1.2);
                m.body(Mat::Plastic, [39.0, 16.0, z], [50.8, 31.0, z + 1.2], 0.15);
                m.body(Mat::Plastic, [59.5, 24.0, z], [65.5, 30.0, z + 0.9], 0.1);
                m.body(Mat::Plastic, [49.5, 34.0, z], [54.5, 39.0, z + 0.9], 0.1);
                m.rbox(Mat::Rgb(200, 202, 205), [6.5, 10.0, z], [18.5, 21.0, z + 1.5], 0.4);
                m.cyl(Mat::Rgb(170, 172, 176), [12.5, 15.5, z + 1.5], [12.5, 15.5, z + 1.52], 2.4, 24);
                // camera (CSI) and display (DSI) FPC connectors with dark latches
                m.boxx(INSERT, [44.0, 33.0, z], [47.4, 54.0, z + 5.5]);
                m.boxx(Mat::Rgb(40, 40, 42), [44.4, 33.3, z + 5.5], [46.0, 53.7, z + 5.8]);
                m.boxx(INSERT, [1.2, 11.0, z], [4.6, 33.0, z + 5.5]);
                m.boxx(Mat::Rgb(40, 40, 42), [2.9, 11.3, z + 5.5], [4.4, 32.7, z + 5.8]);
                // passives sprinkled around the SoC
                for (i, (x, y)) in [(20.0, 14.0), (38.0, 14.0), (20.0, 33.0), (38.0, 33.0), (57.0, 18.0), (57.0, 33.0), (66.5, 21.0), (12.0, 26.0), (16.0, 26.0), (30.0, 34.0)].into_iter().enumerate() {
                    let mat = if i % 3 == 0 { Mat::Plastic } else { Mat::Ceramic };
                    m.boxx(mat, [x - 0.5, y - 0.25, z], [x + 0.5, y + 0.25, z + 0.35]);
                    for sx in [-0.5, 0.32] {
                        m.boxx(Mat::Metal, [x + sx, y - 0.26, z], [x + sx + 0.18, y + 0.26, z + 0.36]);
                    }
                }
            }
        }
        let (lo, hi) = if self == Self::Esp32 { (-8.0, 0.8) } else { (0.0, 8.5) };
        for pin in self.pins() {
            let q = pin.pos;
            m.boxx(Mat::Gold, [q.x - 0.32, q.y - 0.32, surface + lo], [q.x + 0.32, q.y + 0.32, surface + hi]);
            if self == Self::Esp32 {
                // individual spacers on the pins below the board
                m.boxx(Mat::Plastic, [q.x - 1.22, q.y - 1.22, surface - 4.1], [q.x + 1.22, q.y + 1.22, surface - 1.6]);
                joint(&mut m, q);
            }
        }
        m
    }
}

const INSERT: Mat = Mat::Rgb(236, 236, 230);
