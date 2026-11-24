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
        let mut m = Model::default();
        let mut block = |mat, x0, y0, x1, y1, z0, z1| m.boxx(mat, [x0, y0, surface + z0], [x1, y1, surface + z1]);
        match self {
            Self::Esp32 => {
                block(Mat::Green, 4.97, -6.04, 22.97, 19.46, 0.0, 0.8);
                block(Mat::Metal, 5.2, 0.7, 22.7, 19.1, 0.8, 3.2);
                block(Mat::Metal, 10.0, 42.8, 17.9, 49.3, 0.0, 3.0);
                block(Mat::Plastic, 10.0, 29.0, 16.0, 35.0, 0.0, 1.2);
                for x in [5.2, 20.0] {
                    block(Mat::Metal, x, 40.0, x + 2.8, 44.5, 0.0, 2.4);
                }
                for x in [0.0, 25.4] {
                    block(Mat::Plastic, x, 0.0, x + 2.54, 48.26, -4.1, -1.6);
                }
            }
            Self::Pi4 => {
                block(Mat::Plastic, 7.1, 0.96, 57.9, 6.04, 0.0, 2.5);
                block(Mat::Metal, 65.5, 2.3, 88.0, 18.2, 0.0, 13.5); // Ethernet
                for y in [20.5, 38.8] {
                    block(Mat::Metal, 70.3, y, 88.0, y + 13.4, 0.0, 16.0);
                }
                block(Mat::Metal, 6.6, 50.0, 15.8, 57.3, 0.0, 3.2); // USB-C
                for x in [22.4, 35.9] {
                    block(Mat::Metal, x, 50.0, x + 7.2, 57.0, 0.0, 3.0);
                }
                block(Mat::Plastic, 54.1, 44.5, 60.1, 57.0, 0.0, 6.0); // A/V
                block(Mat::Metal, 21.6, 16.0, 36.6, 31.0, 0.0, 2.4);
                block(Mat::Plastic, 39.0, 16.0, 50.8, 31.0, 0.0, 1.5); // LPDDR4
            }
        }
        let (lo, hi) = if self == Self::Esp32 { (-8.0, 0.8) } else { (0.0, 8.5) };
        for pin in self.pins() {
            let q = pin.pos;
            m.boxx(Mat::Gold, [q.x - 0.32, q.y - 0.32, surface + lo], [q.x + 0.32, q.y + 0.32, surface + hi]);
        }
        m
    }
}
