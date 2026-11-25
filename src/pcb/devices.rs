//! Built-in electronics for the Assembler library: development boards and common parts at
//! real scale, coloured per surface, with markings drawn in the stroke font as raised
//! geometry, and with named pins and lamps (LEDs) for the assembly circuit simulation.
//!
//! Model frame while building: x right, y towards the viewer of the top view (board y,
//! "down" in the PCB editor), z up from the part's underside. In the part this becomes
//! world X = x, Y = z, Z = y (the board lies on the OBEN/TOP plane like an inserted PCB).

use super::board::{Board, Via};
use super::geom::{p, P};
use super::model3d::{receptacle, Mat, Model, Receptacle};
use super::modules::Module;
use super::stroke;
use crate::model::features::*;
use crate::sketch::sketch::Sketch;
use serde::{Deserialize, Serialize};

mod sbc;
pub use sbc::Sbc;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum BoardKind {
    ArduinoUno,
    ArduinoNano,
    Esp32DevKit,
    PiPico,
    BluePill,
    Nucleo64,
    Pi4,
    /// every other Raspberry Pi and the Orange Pis
    Sbc(Sbc),
}

impl BoardKind {
    pub const ALL: [BoardKind; 7] = [Self::ArduinoUno, Self::ArduinoNano, Self::Esp32DevKit, Self::PiPico, Self::BluePill, Self::Nucleo64, Self::Pi4];
    pub fn name(self) -> &'static str {
        match self {
            Self::ArduinoUno => "Arduino Uno R3",
            Self::ArduinoNano => "Arduino Nano",
            Self::Esp32DevKit => "ESP32-DevKitC V4",
            Self::PiPico => "Raspberry Pi Pico",
            Self::BluePill => "STM32 Blue Pill (F103C8)",
            Self::Nucleo64 => "STM32 Nucleo-64 (F401RE)",
            Self::Pi4 => "Raspberry Pi 4 Model B",
            Self::Sbc(s) => s.name(),
        }
    }
    pub fn description(self) -> &'static str {
        match self {
            Self::ArduinoUno => "ATmega328P, 5 V, 68,6 × 53,3 mm, USB-B und Hohlstecker, LED „L“ an D13",
            Self::ArduinoNano => "ATmega328P, 5 V, 18 × 45 mm, Steckbrett-Stiftleisten, LED „L“ an D13",
            Self::Esp32DevKit => "ESP32-WROOM-32E, 3,3 V, zwei 19-polige Stiftleisten, Micro-USB",
            Self::PiPico => "RP2040, 3,3 V, 21 × 51 mm, 40 Pins, LED an GP25",
            Self::BluePill => "STM32F103C8T6, 3,3 V, 53 × 23 mm, LED an PC13 (aktiv LOW)",
            Self::Nucleo64 => "STM32F401RE, 3,3 V, Arduino- und Morpho-Leisten, LD2 an D13, Taster B1 an PC13",
            Self::Pi4 => "BCM2711, 3,3 V GPIO, 85 × 56 mm, 40-polige Leiste, USB-C-Versorgung",
            Self::Sbc(s) => s.description(),
        }
    }
    /// Every board of the library, Raspberry Pis and Orange Pis included.
    pub fn all() -> Vec<BoardKind> {
        Self::ALL.iter().copied().chain(Sbc::ALL.iter().map(|&s| Self::Sbc(s))).collect()
    }
    pub fn raspberry(self) -> bool {
        matches!(self, Self::PiPico | Self::Pi4) || matches!(self, Self::Sbc(s) if !s.orange())
    }
    pub fn orange(self) -> bool {
        matches!(self, Self::Sbc(s) if s.orange())
    }
    /// Electrical behaviour of the board in the assembly circuit.
    pub fn spec(self) -> McuSpec {
        const REG5: &[(&str, f64)] = &[("5V", 5.0), ("3V3", 3.3)];
        const REG3: &[(&str, f64)] = &[("3V3", 3.3)];
        let five_volt = McuSpec {
            logic: 5.0,
            usb: REG5,
            inputs: &[("VIN", 7.0, REG5), ("5V", 4.5, REG3)],
            internal: &[],
            led: Some(("L", "D13", true)),
            power_lamp: Some("ON"),
            button: None,
            aliases: &[],
            max_ma: 20.0,
        };
        match self {
            Self::ArduinoUno => McuSpec { aliases: &[("SCL", "A5"), ("SDA", "A4")], ..five_volt },
            Self::ArduinoNano => McuSpec { power_lamp: Some("PWR"), ..five_volt },
            Self::Nucleo64 => McuSpec {
                logic: 3.3,
                internal: &["PC13"],
                led: Some(("LD2", "D13", true)),
                power_lamp: Some("LD3"),
                button: Some(("B1", "PC13")),
                ..five_volt
            },
            Self::Esp32DevKit => McuSpec {
                logic: 3.3,
                usb: REG5,
                inputs: &[("5V", 4.5, REG3), ("3V3", 3.0, &[])],
                internal: &[],
                led: None,
                power_lamp: Some("PWR"),
                button: None,
                aliases: &[],
                max_ma: 40.0,
            },
            Self::PiPico => McuSpec {
                logic: 3.3,
                usb: &[("VBUS", 5.0), ("VSYS", 4.7), ("3V3", 3.3)],
                inputs: &[("VSYS", 1.8, REG3), ("VBUS", 4.3, &[("VSYS", 4.7), ("3V3", 3.3)]), ("3V3", 3.0, &[])],
                internal: &["GP25"],
                led: Some(("LED", "GP25", true)),
                power_lamp: None,
                button: None,
                aliases: &[],
                max_ma: 12.0,
            },
            Self::BluePill => McuSpec {
                logic: 3.3,
                usb: REG5,
                inputs: &[("5V", 4.5, REG3), ("3V3", 3.0, &[])],
                internal: &[],
                led: Some(("PC13", "PC13", false)),
                power_lamp: Some("PWR"),
                button: None,
                aliases: &[],
                max_ma: 20.0,
            },
            Self::Sbc(s) if s.pico() => {
                let base = Self::PiPico.spec();
                if s.wireless_pico() {
                    McuSpec { internal: &["WL_GPIO0"], led: Some(("LED", "WL_GPIO0", true)), ..base }
                } else {
                    base
                }
            }
            Self::Sbc(s) if s.compute_module() => McuSpec {
                logic: 3.3,
                usb: &[],
                inputs: &[("5V", 4.75, REG3)],
                internal: &[],
                led: None,
                power_lamp: None,
                button: None,
                aliases: &[],
                max_ma: 16.0,
            },
            Self::Sbc(s) => McuSpec { power_lamp: s.power_lamp(), ..Self::Pi4.spec() },
            Self::Pi4 => McuSpec {
                logic: 3.3,
                usb: &[("5V", 5.1), ("3V3", 3.3)],
                inputs: &[("5V", 4.75, REG3)],
                internal: &[],
                led: None,
                power_lamp: Some("PWR"),
                button: None,
                aliases: &[],
                max_ma: 16.0,
            },
        }
    }
    /// Whether a pin of this board is a general purpose I/O.
    pub fn is_gpio(self, name: &str) -> bool {
        let rest = |pre: &str| name.strip_prefix(pre).is_some_and(|r| !r.is_empty() && r.chars().all(|c| c.is_ascii_digit()));
        match self {
            Self::ArduinoUno | Self::ArduinoNano | Self::Nucleo64 => rest("D") || rest("A"),
            Self::Esp32DevKit | Self::Pi4 => rest("GPIO"),
            Self::PiPico => rest("GP"),
            Self::BluePill => rest("PA") || rest("PB") || rest("PC"),
            Self::Sbc(s) if s.pico() => rest("GP") || name == "WL_GPIO0",
            Self::Sbc(_) => rest("GPIO") || rest("PIN"),
        }
    }
    /// Input-only pins (no output driver).
    pub fn input_only(self, name: &str) -> bool {
        match self {
            Self::ArduinoNano => matches!(name, "A6" | "A7"),
            Self::Esp32DevKit => matches!(name, "GPIO34" | "GPIO35" | "GPIO36" | "GPIO39"),
            _ => false,
        }
    }
}

/// Supply and I/O of a board. Rails are pin names; same-named pins are one node.
#[derive(Clone, Copy, Debug)]
pub struct McuSpec {
    /// GPIO HIGH level
    pub logic: f64,
    /// rails driven while USB is plugged in
    pub usb: &'static [(&'static str, f64)],
    /// (input rail, minimum voltage, rails then driven by the on-board regulators), by preference
    pub inputs: &'static [(&'static str, f64, &'static [(&'static str, f64)])],
    /// GPIOs without a header pin (on-board LED or button)
    pub internal: &'static [&'static str],
    /// on-board user LED: (lamp, pin, lit when HIGH)
    pub led: Option<(&'static str, &'static str, bool)>,
    /// lamp lit while the board runs
    pub power_lamp: Option<&'static str>,
    /// on-board user button: (name, pin pulled LOW while pressed)
    pub button: Option<(&'static str, &'static str)>,
    /// header pins that are the same signal
    pub aliases: &'static [(&'static str, &'static str)],
    /// recommended current per GPIO, mA
    pub max_ma: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum LedColor {
    Red,
    Green,
    Blue,
    Yellow,
    White,
}

impl LedColor {
    pub const ALL: [LedColor; 5] = [Self::Red, Self::Green, Self::Blue, Self::Yellow, Self::White];
    pub fn name(self) -> &'static str {
        match self {
            Self::Red => "rot",
            Self::Green => "grün",
            Self::Blue => "blau",
            Self::Yellow => "gelb",
            Self::White => "weiß",
        }
    }
    /// Forward voltage at about 10 mA.
    pub fn vf(self) -> f64 {
        match self {
            Self::Red => 2.0,
            Self::Yellow => 2.1,
            Self::Green => 2.2,
            Self::Blue | Self::White => 3.0,
        }
    }
    /// Lens colour off and lit (lit colours are drawn unshaded).
    pub fn rgb(self) -> ([f32; 3], [f32; 3]) {
        match self {
            Self::Red => ([0.55, 0.08, 0.07], [1.0, 0.22, 0.16]),
            Self::Green => ([0.10, 0.45, 0.14], [0.35, 1.0, 0.35]),
            Self::Blue => ([0.10, 0.18, 0.55], [0.35, 0.55, 1.0]),
            Self::Yellow => ([0.62, 0.52, 0.08], [1.0, 0.92, 0.30]),
            Self::White => ([0.80, 0.82, 0.84], [1.0, 1.0, 1.0]),
        }
    }
}

/// What a library electronics part is (decides its circuit behaviour).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Kind {
    Board(BoardKind),
    Led(LedColor),
    /// axial resistor, ohms
    Resistor(u32),
    /// 6 × 6 mm tactile switch
    Tact6,
    /// 12 × 12 mm tactile switch with cap
    Tact12,
    /// 1 × n male pin header
    Header(u8),
    UsbC,
    MicroUsb,
    Dht22,
    HcSr04,
    Battery9V,
}

impl Kind {
    pub fn name(self) -> String {
        match self {
            Kind::Board(b) => b.name().into(),
            Kind::Led(c) => format!("LED 5 mm {}", c.name()),
            Kind::Resistor(r) => format!("Widerstand {} (0,25 W)", ohm_label(r)),
            Kind::Tact6 => "Taster 6 × 6 mm".into(),
            Kind::Tact12 => "Taster 12 × 12 mm mit Kappe".into(),
            Kind::Header(n) => format!("Stiftleiste 1 × {n}"),
            Kind::UsbC => "USB-C-Buchse (Breakout)".into(),
            Kind::MicroUsb => "Micro-USB-Buchse (Breakout)".into(),
            Kind::Dht22 => "DHT22 Temperatur-/Feuchtesensor".into(),
            Kind::HcSr04 => "HC-SR04 Ultraschallsensor".into(),
            Kind::Battery9V => "9-V-Blockbatterie".into(),
        }
    }
    pub fn description(self) -> String {
        match self {
            Kind::Board(b) => b.description().into(),
            Kind::Led(c) => format!("Ø 5 mm, Flussspannung {:.1} V, Anode am langen Bein", c.vf()),
            Kind::Resistor(_) => "Kohleschicht, Farbcode nach Wert, Rastermaß 10,16 mm".into(),
            Kind::Tact6 => "Pins 1–2 und 3–4 verbunden, gedrückt 1–3".into(),
            Kind::Tact12 => "Pins 1–2 und 3–4 verbunden, gedrückt 1–3, rote Kappe".into(),
            Kind::Header(_) => "Raster 2,54 mm, vergoldet".into(),
            Kind::UsbC => "5 V an VBUS, solange ein Kabel steckt".into(),
            Kind::MicroUsb => "5 V an VBUS, solange ein Kabel steckt".into(),
            Kind::Dht22 => "3,3–6 V, Daten an einem GPIO, 15,1 × 25 × 7,7 mm".into(),
            Kind::HcSr04 => "5 V, Trig/Echo an zwei GPIOs, 45 × 20 mm".into(),
            Kind::Battery9V => "6LR61, 9 V, Innenwiderstand 1,5 Ω".into(),
        }
    }
}

pub fn ohm_label(r: u32) -> String {
    if r >= 1_000_000 {
        format!("{} MΩ", trim(r as f64 / 1e6))
    } else if r >= 1000 {
        format!("{} kΩ", trim(r as f64 / 1e3))
    } else {
        format!("{r} Ω")
    }
}

fn trim(v: f64) -> String {
    let s = format!("{v:.1}");
    s.trim_end_matches(".0").replace('.', ",")
}

/// A pin in part coordinates: where wires attach.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct DevPin {
    pub name: String,
    pub pos: [f64; 3],
}

/// An LED body of the part: its import feature lights up in the simulation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Lamp {
    pub name: String,
    pub feature: u64,
    pub on: [f32; 3],
}

/// Electrical description stored with a library part in the assembly.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Device {
    pub kind: Kind,
    pub pins: Vec<DevPin>,
    pub lamps: Vec<Lamp>,
}

impl Device {
    pub fn pin(&self, name: &str) -> Option<&DevPin> {
        self.pins.iter().find(|p| p.name == name)
    }
    /// Pin names without repeats (GND appears once).
    pub fn pin_names(&self) -> Vec<String> {
        let mut v: Vec<String> = vec![];
        for p in &self.pins {
            if !v.contains(&p.name) {
                v.push(p.name.clone());
            }
        }
        v
    }
}

/// A finished library part: geometry plus its electrical description.
pub struct DevicePart {
    pub data: ModelData,
    pub device: Device,
}

// ------------------------------------------------------------------ building blocks

fn world_face(v: &[[f32; 6]], t: &[[u32; 3]]) -> ImportFace {
    ImportFace { v: v.iter().map(|q| [q[0], q[2], q[1], q[3], q[5], q[4]]).collect(), t: t.iter().map(|x| [x[0], x[2], x[1]]).collect() }
}

fn w3(q: [f64; 3]) -> [f64; 3] {
    [q[0], q[2], q[1]]
}

fn rect_pts(lo: P, hi: P) -> Vec<P> {
    vec![lo, p(hi.x, lo.y), hi, p(lo.x, hi.y)]
}

struct Build {
    m: ModelData,
    pins: Vec<DevPin>,
    lamps: Vec<Lamp>,
}

impl Build {
    fn new(name: &str, color: [f32; 3], density: f64) -> Build {
        let mut m = ModelData::new_part_template(name);
        m.appearance.color = color;
        m.density = density;
        Build { m, pins: vec![], lamps: vec![] }
    }
    /// Laminate with drilled holes (pins and mounting holes) like an inserted PCB.
    fn board(name: &str, outline: Vec<P>, holes: &[(P, f64)], thickness: f64, color: [f32; 3]) -> Build {
        let b = Board {
            outline: vec![outline],
            vias: holes.iter().map(|&(pos, d)| Via { pos, dia: d + 0.6, drill: d, net: String::new() }).collect(),
            thickness,
            ..Board::default()
        };
        let mut m = super::body3d::board_part(&b, name).expect("outline");
        m.appearance.color = color;
        m.feature_colors.insert(10, color);
        Build { m, pins: vec![], lamps: vec![] }
    }
    fn push(&mut self, name: &str, kind: FeatureKind) -> u64 {
        let id = self.m.next_id;
        self.m.features.push(Feature { id, name: name.into(), kind, suppressed: false, hidden: false });
        self.m.next_id += 1;
        id
    }
    /// Solid extruded up from the TOP plane (the part's underside at y = 0).
    fn extrude(&mut self, name: &str, sketch: Sketch, depth: f64, color: [f32; 3]) -> u64 {
        let FeatureKind::Extrude(mut e) = FeatureKind::new_extrude() else { unreachable!() };
        e.plane = Some(PlaneRef::Datum(TOP_ID));
        e.sketch = sketch;
        e.depth = Dim::v(depth);
        let id = self.push(name, FeatureKind::Extrude(e));
        self.m.feature_colors.insert(id, color);
        id
    }
    fn extrude_rect(&mut self, name: &str, lo: P, hi: P, depth: f64, color: [f32; 3]) -> u64 {
        self.extrude(name, Sketch::rect(hi.x - lo.x, hi.y - lo.y, (lo.x + hi.x) / 2.0, -(lo.y + hi.y) / 2.0), depth, color)
    }
    fn import(&mut self, name: &str, color: [f32; 3], faces: Vec<ImportFace>) -> u64 {
        let def = ImportDef { source: self.m.name.clone(), format: "SolidWork-Bibliothek".into(), faces };
        super::body3d::add_import_body(&mut self.m, name, color, def)
    }
    /// One body per material of `model`.
    fn model(&mut self, model: &Model) {
        for (mat, v, t) in &model.parts {
            if !t.is_empty() {
                self.import(&mat.name(), mat.rgb(), vec![world_face(v, t)]);
            }
        }
    }
    fn lamp(&mut self, name: &str, off: [f32; 3], on: [f32; 3], model: &Model) {
        let faces: Vec<ImportFace> = model.parts.iter().filter(|x| !x.2.is_empty()).map(|(_, v, t)| world_face(v, t)).collect();
        let feature = self.import(&format!("LED {name}"), off, faces);
        self.lamps.push(Lamp { name: name.into(), feature, on });
    }
    fn pin(&mut self, name: &str, q: [f64; 3]) {
        self.pins.push(DevPin { name: name.into(), pos: w3(q) });
    }
    fn done(self, kind: Kind) -> DevicePart {
        DevicePart { data: self.m, device: Device { kind, pins: self.pins, lamps: self.lamps } }
    }
}

/// Raised stroke-font text on a horizontal surface at height z.
fn text(m: &mut Model, mat: Mat, s: &str, c: P, h: f64, angle: f64, z: f64) {
    let pen = stroke::pen(h);
    for line in stroke::centered(s, c, h, angle, false) {
        if line.len() == 1 {
            let q = line[0];
            m.boxx(mat, [q.x - pen / 2.0, q.y - pen / 2.0, z], [q.x + pen / 2.0, q.y + pen / 2.0, z + 0.04]);
        }
        for w in line.windows(2) {
            let (a, b) = (w[0], w[1]);
            // extend each stroke by half the pen so the corners close
            let d = (b - a).normalize_or_zero() * (pen / 2.0);
            let n = p(-d.y, d.x);
            m.prism(mat, &[a - d - n, b + d - n, b + d + n, a - d + n], z, z + 0.04);
        }
    }
}

const SILK: Mat = Mat::White;
const MARK: Mat = Mat::Rgb(190, 190, 186);
const BLACK: Mat = Mat::Plastic;

fn rotated_square(c: P, half: f64, deg: f64) -> Vec<P> {
    let r = |q: P| super::geom::rot(q, deg) + c;
    vec![r(p(-half, -half)), r(p(half, -half)), r(p(half, half)), r(p(-half, half))]
}

/// Quad flat package: moulded body, gull-wing leads on four sides and the marking.
fn qfp(m: &mut Model, c: P, size: f64, per_side: usize, pitch: f64, deg: f64, z: f64, mark: &str) {
    let half = size / 2.0;
    m.prism(BLACK, &rotated_square(c, half, deg), z + 0.1, z + 1.4);
    let r = |q: P| super::geom::rot(q, deg) + c;
    for side in 0..4 {
        let srot = side as f64 * 90.0;
        for i in 0..per_side {
            let t = (i as f64 - (per_side as f64 - 1.0) / 2.0) * pitch;
            let a = r(super::geom::rot(p(t, half), srot));
            let b = r(super::geom::rot(p(t, half + 0.9), srot));
            m.bar(Mat::Metal, [a.x, a.y, z + 0.5], [b.x, b.y, z + 0.08], 0.22, 0.12);
        }
    }
    // pin-1 dot and marking
    let dot = r(p(-half + 0.9, -half + 0.9));
    m.cyl(MARK, [dot.x, dot.y, z + 1.4], [dot.x, dot.y, z + 1.43], 0.3, 10);
    text(m, MARK, mark, c, (size / mark.len().max(4) as f64).min(1.0), deg, z + 1.4);
}

/// QFN / DFN: moulded body without leads.
fn qfn(m: &mut Model, c: P, w: f64, h: f64, z: f64, mark: &str) {
    m.body(BLACK, [c.x - w / 2.0, c.y - h / 2.0, z], [c.x + w / 2.0, c.y + h / 2.0, z + 0.9], 0.05);
    if !mark.is_empty() {
        text(m, MARK, mark, c, (w / mark.len().max(3) as f64).min(0.8), 0.0, z + 0.9);
    }
}

/// Small SMD chip part (0603 / 0805) with terminations.
fn chip(m: &mut Model, c: P, l: f64, w: f64, vertical: bool, z: f64, body: Mat) {
    let (hx, hy) = if vertical { (w / 2.0, l / 2.0) } else { (l / 2.0, w / 2.0) };
    m.boxx(body, [c.x - hx, c.y - hy, z], [c.x + hx, c.y + hy, z + w * 0.6]);
    let t = l * 0.18;
    for s in [-1.0, 1.0] {
        let (lo, hi) = if vertical {
            ([c.x - hx - 0.01, c.y + s * hy - if s > 0.0 { t } else { 0.0 } - 0.01, z], [c.x + hx + 0.01, c.y + s * hy + if s < 0.0 { t } else { 0.0 } + 0.01, z + w * 0.6 + 0.01])
        } else {
            ([c.x + s * hx - if s > 0.0 { t } else { 0.0 } - 0.01, c.y - hy - 0.01, z], [c.x + s * hx + if s < 0.0 { t } else { 0.0 } + 0.01, c.y + hy + 0.01, z + w * 0.6 + 0.01])
        };
        m.boxx(Mat::Metal, lo, hi);
    }
}

/// SMD tactile switch (reset / boot buttons on boards).
fn smd_button(m: &mut Model, c: P, w: f64, h: f64, z: f64, cap: Mat) {
    m.boxx(Mat::Metal, [c.x - w / 2.0, c.y - h / 2.0, z], [c.x + w / 2.0, c.y + h / 2.0, z + 1.2]);
    m.boxx(cap, [c.x - w / 4.0, c.y - h / 4.0, z + 1.2], [c.x + w / 4.0, c.y + h / 4.0, z + 1.8]);
}

/// Female header strip (Arduino shield sockets): one row along `dir` from `start`.
fn female_header(m: &mut Model, start: P, dir: P, n: usize, z: f64) {
    let end = start + dir * (n as f64 - 1.0) * 2.54;
    let (lo, hi) = super::geom::bbox([start, end]).unwrap();
    m.boxx(BLACK, [lo.x - 1.27, lo.y - 1.27, z], [hi.x + 1.27, hi.y + 1.27, z + 8.5]);
    for i in 0..n {
        let q = start + dir * (i as f64 * 2.54);
        m.boxx(Mat::Rgb(60, 60, 58), [q.x - 0.5, q.y - 0.5, z + 8.5], [q.x + 0.5, q.y + 0.5, z + 8.52]);
        m.boxx(Mat::Gold, [q.x - 0.25, q.y - 0.25, z + 8.52], [q.x + 0.25, q.y + 0.25, z + 8.53]);
    }
}

/// Male pins through a board: plastic spacer on one side, pin from `bottom` to `top`.
fn male_pins(m: &mut Model, at: &[P], spacer: (f64, f64), bottom: f64, top: f64) {
    for q in at {
        m.boxx(BLACK, [q.x - 1.24, q.y - 1.24, spacer.0], [q.x + 1.24, q.y + 1.24, spacer.1]);
        m.boxx(Mat::Gold, [q.x - 0.32, q.y - 0.32, bottom], [q.x + 0.32, q.y + 0.32, top]);
    }
}

fn smd_led(c: P, z: f64) -> Model {
    let mut m = Model::default();
    m.boxx(Mat::White, [c.x - 0.8, c.y - 0.5, z], [c.x + 0.8, c.y + 0.5, z + 0.6]);
    m
}

fn crystal(m: &mut Model, c: P, l: f64, w: f64, z: f64, vertical: bool) {
    let (hx, hy) = if vertical { (w / 2.0, l / 2.0) } else { (l / 2.0, w / 2.0) };
    m.body(Mat::Metal, [c.x - hx, c.y - hy, z], [c.x + hx, c.y + hy, z + 1.0], 0.3);
}

fn elcap(m: &mut Model, c: P, r: f64, h: f64, z: f64) {
    m.boxx(BLACK, [c.x - r - 0.2, c.y - r - 0.2, z], [c.x + r + 0.2, c.y + r + 0.2, z + 0.5]);
    m.cyl(Mat::Metal, [c.x, c.y, z + 0.5], [c.x, c.y, z + h - 0.3], r, 32);
    m.cone(Mat::Metal, [c.x, c.y, z + h - 0.3], [c.x, c.y, z + h], r, r - 0.3, 32);
    // pressed vent cross and the black minus marking
    let vent = Mat::Rgb(130, 132, 138);
    m.boxx(vent, [c.x - r * 0.62, c.y - 0.07, z + h], [c.x + r * 0.62, c.y + 0.07, z + h + 0.01]);
    m.boxx(vent, [c.x - 0.07, c.y - r * 0.62, z + h], [c.x + 0.07, c.y + r * 0.62, z + h + 0.01]);
    m.boxx(BLACK, [c.x - r * 0.62, c.y - r * 0.5, z + h], [c.x - r * 0.3, c.y + r * 0.5, z + h + 0.012]);
}

const TIN: Mat = Mat::Rgb(205, 205, 198);

/// Plated ring and solder joint around a through-hole pin on the top side at z.
fn joint(m: &mut Model, q: P, z: f64) {
    m.annulus(TIN, q, 0.45, 0.85, z + 0.02, 16, false);
    m.cone(Mat::Rgb(190, 192, 190), [q.x, q.y, z + 0.02], [q.x, q.y, z + 0.55], 0.62, 0.36, 12);
}

fn circle_ring(c: P, r: f64, n: usize) -> Vec<P> {
    (0..n).map(|i| c + r * p((i as f64 / n as f64 * std::f64::consts::TAU).cos(), (i as f64 / n as f64 * std::f64::consts::TAU).sin())).collect()
}

// ------------------------------------------------------------------ boards

const UNO_W: f64 = 68.58;
const UNO_H: f64 = 53.34;

/// Arduino shield headers (Uno R3 / Nucleo): name and position in Arduino's drawing frame
/// (origin at the lower left, y up).
fn shield_headers(nucleo: bool) -> Vec<(&'static str, P)> {
    let mut v = vec![];
    let ten: [&str; 10] = if nucleo {
        ["D15", "D14", "AREF", "GND", "D13", "D12", "D11", "D10", "D9", "D8"]
    } else {
        ["SCL", "SDA", "AREF", "GND", "D13", "D12", "D11", "D10", "D9", "D8"]
    };
    for (i, n) in ten.iter().enumerate() {
        v.push((*n, p(18.796 + i as f64 * 2.54, 50.8)));
    }
    for (i, n) in ["D7", "D6", "D5", "D4", "D3", "D2", "D1", "D0"].iter().enumerate() {
        v.push((*n, p(45.72 + i as f64 * 2.54, 50.8)));
    }
    for (i, n) in ["NC", "IOREF", "RESET", "3V3", "5V", "GND", "GND", "VIN"].iter().enumerate() {
        v.push((*n, p(27.94 + i as f64 * 2.54, 2.54)));
    }
    for (i, n) in ["A0", "A1", "A2", "A3", "A4", "A5"].iter().enumerate() {
        v.push((*n, p(50.8 + i as f64 * 2.54, 2.54)));
    }
    v
}

fn uno() -> DevicePart {
    let f = |x: f64, y: f64| p(x, UNO_H - y);
    let outline: Vec<P> = [(0.0, 0.0), (66.04, 0.0), (68.58, 2.54), (68.58, 35.56), (66.04, 38.1), (66.04, 51.82), (64.52, 53.34), (0.0, 53.34)]
        .iter()
        .map(|&(x, y)| f(x, y))
        .collect();
    let headers: Vec<(&str, P)> = shield_headers(false).into_iter().map(|(n, q)| (n, f(q.x, q.y))).collect();
    let mounts = [f(13.97, 2.54), f(15.24, 50.8), f(66.04, 7.62), f(66.04, 35.56)];
    let holes: Vec<(P, f64)> = mounts.iter().map(|&q| (q, 3.2)).collect();
    let t = 1.6;
    let mut b = Build::board(BoardKind::ArduinoUno.name(), outline, &holes, t, [0.0, 0.42, 0.52]);
    let mut m = Model::default();
    // sockets
    female_header(&mut m, f(18.796, 50.8), p(1.0, 0.0), 10, t);
    female_header(&mut m, f(45.72, 50.8), p(1.0, 0.0), 8, t);
    female_header(&mut m, f(27.94, 2.54), p(1.0, 0.0), 8, t);
    female_header(&mut m, f(50.8, 2.54), p(1.0, 0.0), 6, t);
    // USB-B and DC jack overhang the left edge
    receptacle(&mut m, Receptacle::TypeB, p(-6.35, UNO_H - 37.775), p(-1.0, 0.0), t);
    receptacle(&mut m, Receptacle::Barrel, p(-1.9, UNO_H - 7.8), p(-1.0, 0.0), t);
    // ATmega328P in its DIP socket
    let (dx0, dx1, dy0, dy1) = (30.5, 66.1 - 0.5, UNO_H - 21.6, UNO_H - 13.8);
    m.boxx(BLACK, [dx0 - 0.6, dy0 - 0.3, t], [dx1 + 0.6, dy1 + 0.3, t + 3.0]);
    m.body(Mat::Rgb(28, 28, 30), [dx0, dy0 + 0.6, t + 3.0], [dx1, dy1 - 0.6, t + 6.3], 0.3);
    for i in 0..14 {
        let x = dx0 + 1.3 + i as f64 * 2.54;
        for y in [dy0 + 0.3, dy1 - 0.3] {
            m.boxx(Mat::Metal, [x - 0.25, y - 0.3, t + 3.0], [x + 0.25, y + 0.3, t + 4.6]);
        }
    }
    text(&mut m, MARK, "ATMEGA328P-PU", p((dx0 + dx1) / 2.0, (dy0 + dy1) / 2.0), 1.3, 0.0, t + 6.3);
    // USB interface chip, crystal, reset button, ICSP, capacitors
    qfn(&mut m, f(21.0, 36.5), 5.0, 5.0, t, "16U2");
    crystal(&mut m, f(29.5, 24.3), 4.6, 2.0, t, false);
    crystal(&mut m, f(26.0, 26.0), 11.0, 4.5, t, false);
    smd_button(&mut m, f(6.8, 48.6), 6.0, 3.5, t, BLACK);
    let icsp: Vec<P> = (0..6).map(|i| f(63.5 + (i % 2) as f64 * 2.54, 27.94 + (i / 2) as f64 * 2.54)).collect();
    male_pins(&mut m, &icsp, (t, t + 2.5), t - 0.5, t + 8.5);
    for &q in &icsp {
        m.annulus(TIN, q, 1.25, 1.5, t + 0.02, 4, false);
    }
    elcap(&mut m, f(18.0, 6.5), 3.0, 5.5, t);
    elcap(&mut m, f(24.0, 6.5), 3.0, 5.5, t);
    m.body(BLACK, [UNO_W - 25.0, UNO_H - 30.0, t], [UNO_W - 19.0, UNO_H - 26.5, t + 1.6], 0.2); // regulator
    for (i, (x, y)) in [(32.0, 33.0), (34.0, 33.0), (36.0, 33.0), (55.0, 33.0), (57.0, 30.0), (12.0, 22.0), (14.0, 22.0)].into_iter().enumerate() {
        chip(&mut m, f(x, y), 1.6, 0.8, i % 2 == 0, t, if i % 3 == 0 { Mat::Ceramic } else { BLACK });
    }
    // silkscreen
    text(&mut m, SILK, "UNO", f(48.0, 39.2), 5.5, 0.0, t);
    text(&mut m, SILK, "ARDUINO", f(48.0, 33.0), 2.6, 0.0, t);
    text(&mut m, SILK, "DIGITAL (PWM~)", f(47.0, 45.6), 1.3, 0.0, t);
    text(&mut m, SILK, "POWER", f(36.8, 8.6), 1.3, 0.0, t);
    text(&mut m, SILK, "ANALOG IN", f(57.2, 8.6), 1.3, 0.0, t);
    for &(n, q) in &headers {
        let label = n.trim_start_matches('D');
        let off = if q.y < UNO_H / 2.0 { 3.6 } else { -3.6 };
        text(&mut m, SILK, if n.starts_with('D') { label } else { n }, q + p(0.0, off), 0.9, 90.0, t);
    }
    for (name, (x, y)) in [("L", (28.5, 43.0)), ("TX", (28.5, 40.6)), ("RX", (28.5, 38.6)), ("ON", (61.5, 38.0))] {
        text(&mut m, SILK, name, f(x - 3.0, y), 1.0, 0.0, t);
    }
    b.model(&m);
    let (off, on) = LedColor::Yellow.rgb();
    b.lamp("L", off, on, &smd_led(f(28.5, 43.0), t));
    b.lamp("TX", off, on, &smd_led(f(28.5, 40.6), t));
    b.lamp("RX", off, on, &smd_led(f(28.5, 38.6), t));
    let (off, on) = LedColor::Green.rgb();
    b.lamp("ON", off, on, &smd_led(f(61.5, 38.0), t));
    for &(n, q) in &headers {
        b.pin(n, [q.x, q.y, t + 8.5]);
    }
    b.done(Kind::Board(BoardKind::ArduinoUno))
}

fn nano() -> DevicePart {
    let (w, l, t) = (18.0, 45.0, 1.6);
    let left = ["D12", "D11", "D10", "D9", "D8", "D7", "D6", "D5", "D4", "D3", "D2", "GND", "RESET", "D0", "D1"];
    let right = ["D13", "3V3", "AREF", "A0", "A1", "A2", "A3", "A4", "A5", "A6", "A7", "5V", "RESET", "GND", "VIN"];
    let pos = |col: usize, i: usize| p(if col == 0 { 1.27 } else { 16.51 }, 5.08 + i as f64 * 2.54 + 2.54);
    let mut pins: Vec<(&str, P)> = vec![];
    for i in 0..15 {
        pins.push((left[i], pos(0, i)));
        pins.push((right[i], pos(1, i)));
    }
    let mounts = [p(1.27, 1.7), p(16.73, 1.7), p(1.27, 43.3), p(16.73, 43.3)];
    let holes: Vec<(P, f64)> = mounts.iter().map(|&q| (q, 1.7)).collect();
    let mut b = Build::board(BoardKind::ArduinoNano.name(), rect_pts(P::ZERO, p(w, l)), &holes, t, [0.05, 0.28, 0.62]);
    let mut m = Model::default();
    let at: Vec<P> = pins.iter().map(|x| x.1).collect();
    male_pins(&mut m, &at, (-2.5, 0.0), -8.5, t + 1.0);
    for &q in &at {
        joint(&mut m, q, t);
    }
    receptacle(&mut m, Receptacle::MiniB, p(9.0, -1.5), p(0.0, -1.0), t);
    qfp(&mut m, p(9.0, 25.0), 7.0, 8, 0.8, 45.0, t, "ATMEGA328P");
    smd_button(&mut m, p(9.0, 36.5), 3.0, 6.0, t, Mat::White);
    crystal(&mut m, p(9.0, 15.5), 3.2, 1.6, t, false);
    for (i, (x, y)) in [(5.0, 18.0), (13.0, 18.0), (5.0, 32.0), (13.0, 32.0), (6.5, 40.5), (11.5, 40.5)].into_iter().enumerate() {
        chip(&mut m, p(x, y), 1.6, 0.8, i % 2 == 0, t, if i % 2 == 0 { Mat::Ceramic } else { BLACK });
    }
    text(&mut m, SILK, "NANO", p(9.0, 42.6), 1.4, 0.0, t);
    for &(n, q) in &pins {
        let x = if q.x < w / 2.0 { q.x + 2.4 } else { q.x - 2.4 };
        text(&mut m, SILK, n, p(x, q.y), 0.7, 0.0, t);
    }
    for (name, x) in [("TX", 6.15), ("RX", 8.05), ("PWR", 9.95), ("L", 11.85)] {
        text(&mut m, SILK, name, p(x, 10.2), 0.45, 0.0, t);
    }
    b.model(&m);
    let (off, on) = LedColor::Yellow.rgb();
    b.lamp("TX", off, on, &smd_led(p(6.15, 11.6), t));
    b.lamp("RX", off, on, &smd_led(p(8.05, 11.6), t));
    let (off, on) = LedColor::Blue.rgb();
    b.lamp("PWR", off, on, &smd_led(p(9.95, 11.6), t));
    let (off, on) = LedColor::Yellow.rgb();
    b.lamp("L", off, on, &smd_led(p(11.85, 11.6), t));
    for &(n, q) in &pins {
        b.pin(n, [q.x, q.y, t + 1.0]);
    }
    b.done(Kind::Board(BoardKind::ArduinoNano))
}

/// The ESP32 DevKitC and the Pi 4 use the module tables of the PCB editor.
fn from_module(kind: BoardKind, module: Module) -> DevicePart {
    let (data, board) = super::body3d::module_part(module).expect("module");
    let t = board.thickness;
    let mut b = Build { m: data, pins: vec![], lamps: vec![] };
    b.m.name = kind.name().into();
    let mut m = Model::default();
    let pin_top;
    match module {
        Module::Esp32 => {
            let black = [0.07, 0.07, 0.08];
            b.m.appearance.color = black;
            b.m.feature_colors.insert(10, black);
            text(&mut m, MARK, "ESP32-WROOM-32E", p(13.95, 7.0), 1.1, 0.0, t + 3.2);
            text(&mut m, MARK, "ESPRESSIF", p(13.95, 11.5), 1.4, 0.0, t + 3.2);
            text(&mut m, SILK, "ESP32-DEVKITC", p(13.95, 26.0), 1.3, 0.0, t);
            text(&mut m, SILK, "EN", p(6.6, 38.6), 0.9, 0.0, t);
            text(&mut m, SILK, "BOOT", p(21.4, 38.6), 0.9, 0.0, t);
            for pin in module.pins() {
                let x = if pin.pos.x < 10.0 { pin.pos.x + 2.6 } else { pin.pos.x - 2.6 };
                text(&mut m, SILK, pin.name.trim_start_matches("GPIO"), p(x, pin.pos.y), 0.75, 0.0, t);
            }
            pin_top = t + 0.8;
            b.model(&m);
            let (off, on) = LedColor::Red.rgb();
            b.lamp("PWR", off, on, &smd_led(p(19.5, 37.0), t));
        }
        Module::Pi4 => {
            text(&mut m, SILK, "RASPBERRY PI 4 MODEL B", p(23.0, 41.5), 1.7, 0.0, t);
            text(&mut m, MARK, "BCM2711", p(29.1, 23.5), 1.6, 0.0, t + 2.4);
            text(&mut m, SILK, "J8", p(4.5, 3.5), 1.2, 0.0, t);
            pin_top = t + 8.5;
            b.model(&m);
            let (off, on) = LedColor::Red.rgb();
            b.lamp("PWR", off, on, &smd_led(p(2.6, 45.5), t));
            let (off, on) = LedColor::Green.rgb();
            b.lamp("ACT", off, on, &smd_led(p(2.6, 48.5), t));
        }
    }
    for pin in module.pins() {
        b.pin(pin.name, [pin.pos.x, pin.pos.y, pin_top]);
    }
    b.done(Kind::Board(kind))
}

fn pico_variant(variant: Option<Sbc>) -> DevicePart {
    let kind = variant.map_or(BoardKind::PiPico, BoardKind::Sbc);
    let wireless = variant.is_some_and(|s| s.wireless_pico());
    let rp2350 = matches!(variant, Some(Sbc::Pico2 | Sbc::Pico2W));
    let debug = matches!(variant, Some(Sbc::PicoH | Sbc::PicoWH));
    let (w, l, t) = (21.0, 51.0, 1.0);
    let left = ["GP0", "GP1", "GND", "GP2", "GP3", "GP4", "GP5", "GND", "GP6", "GP7", "GP8", "GP9", "GND", "GP10", "GP11", "GP12", "GP13", "GND", "GP14", "GP15"];
    let right = ["VBUS", "VSYS", "GND", "3V3_EN", "3V3", "ADC_VREF", "GP28", "GND", "GP27", "GP26", "RUN", "GP22", "GND", "GP21", "GP20", "GP19", "GP18", "GND", "GP17", "GP16"];
    let mut pins: Vec<(&str, P)> = vec![];
    for i in 0..20 {
        let y = 1.37 + i as f64 * 2.54;
        pins.push((left[i], p(1.61, y)));
        pins.push((right[i], p(w - 1.61, y)));
    }
    let mounts = [p(4.8, 2.0), p(16.2, 2.0), p(4.8, 49.0), p(16.2, 49.0)];
    let holes: Vec<(P, f64)> = mounts.iter().map(|&q| (q, 2.1)).collect();
    let mut b = Build::board(kind.name(), rect_pts(P::ZERO, p(w, l)), &holes, t, [0.10, 0.46, 0.26]);
    let mut m = Model::default();
    for &(_, q) in &pins {
        m.boxx(Mat::Gold, [q.x - 0.8, q.y - 0.8, t], [q.x + 0.8, q.y + 0.8, t + 0.03]);
    }
    let at: Vec<P> = pins.iter().map(|x| x.1).collect();
    male_pins(&mut m, &at, (-2.5, 0.0), -8.5, t + 0.8);
    for &q in &at {
        m.cone(Mat::Rgb(190, 192, 190), [q.x, q.y, t + 0.03], [q.x, q.y, t + 0.5], 0.62, 0.36, 12);
    }
    receptacle(&mut m, Receptacle::MicroB, p(10.5, -1.3), p(0.0, -1.0), t);
    qfn(&mut m, p(10.5, 26.0), 7.0, 7.0, t, if rp2350 { "RP2350" } else { "RP2-B2" });
    qfn(&mut m, p(10.5, 16.0), 4.0, 5.0, t, if rp2350 { "W25Q32" } else { "W25Q16" });
    crystal(&mut m, p(10.5, 34.5), 3.2, 2.5, t, false);
    if wireless {
        // CYW43439 under its can, PCB antenna at the far end
        m.rbox(Mat::Rgb(200, 202, 205), [6.0, 39.0, t], [15.0, 46.0, t + 1.2], 0.4);
        for i in 0..5 {
            let x = 5.0 + i as f64 * 2.6;
            m.boxx(Mat::Gold, [x, 47.0, t], [x + 0.4, 50.2, t + 0.02]);
            m.boxx(Mat::Gold, [x, if i % 2 == 0 { 49.8 } else { 47.0 }, t], [x + 3.0, if i % 2 == 0 { 50.2 } else { 47.4 }, t + 0.02]);
        }
    }
    if debug {
        // 3-pin JST-SH debug connector
        m.boxx(Mat::Rgb(236, 236, 230), [7.0, l - 4.6, t], [14.0, l - 0.6, t + 2.9]);
        m.boxx(Mat::Rgb(60, 60, 60), [7.6, l - 0.62, t + 0.6], [13.4, l - 0.6, t + 2.3]);
    }
    m.boxx(Mat::Metal, [8.75, 8.0, t], [12.25, 12.5, t + 1.0]);
    m.boxx(Mat::Rgb(235, 235, 230), [9.5, 8.8, t + 1.0], [11.5, 11.7, t + 1.6]);
    if !wireless {
        m.body(BLACK, [7.5, 38.5, t], [13.5, 41.0, t + 1.1], 0.2);
    }
    for i in 0..3 {
        let x = 8.0 + i as f64 * 2.5;
        m.boxx(Mat::Gold, [x - 0.6, l - 1.6, t], [x + 0.6, l, t + 0.03]);
    }
    let title = match variant {
        Some(Sbc::Pico2) => "PICO 2",
        Some(Sbc::Pico2W) => "PICO 2 W",
        Some(Sbc::PicoW | Sbc::PicoWH) => "PICO W",
        _ => "PICO",
    };
    let (ty0, ty1) = if wireless { (31.4, 37.4) } else { (40.8, 43.4) };
    text(&mut m, SILK, "RASPBERRY PI", p(10.5, ty0), 0.9, 0.0, t);
    text(&mut m, SILK, title, p(10.5, ty1), 1.4, 0.0, t);
    text(&mut m, SILK, "BOOTSEL", p(10.5, 13.6), 0.7, 0.0, t);
    text(&mut m, SILK, "LED", p(4.3, 7.0), 0.7, 0.0, t);
    for &(n, q) in &pins {
        let x = if q.x < w / 2.0 { q.x + 2.6 } else { q.x - 2.6 };
        text(&mut m, SILK, n, p(x, q.y), 0.55, 0.0, t);
    }
    b.model(&m);
    let (off, on) = LedColor::Green.rgb();
    b.lamp("LED", off, on, &smd_led(p(4.3, 4.6), t));
    for &(n, q) in &pins {
        b.pin(n, [q.x, q.y, t + 0.8]);
    }
    b.done(Kind::Board(kind))
}

fn blue_pill() -> DevicePart {
    let (w, l, t) = (53.34, 22.86, 1.6);
    let top = ["PB12", "PB13", "PB14", "PB15", "PA8", "PA9", "PA10", "PA11", "PA12", "PA15", "PB3", "PB4", "PB5", "PB6", "PB7", "PB8", "PB9", "5V", "GND", "3V3"];
    let bottom = ["GND", "GND", "3V3", "NRST", "PB11", "PB10", "PB1", "PB0", "PA7", "PA6", "PA5", "PA4", "PA3", "PA2", "PA1", "PA0", "PC15", "PC14", "PC13", "VBAT"];
    let mut pins: Vec<(&str, P)> = vec![];
    for i in 0..20 {
        let x = 2.54 + i as f64 * 2.54;
        pins.push((top[i], p(x, 3.81)));
        pins.push((bottom[i], p(x, 19.05)));
    }
    let swd: Vec<P> = (0..4).map(|i| p(51.6, 7.62 + i as f64 * 2.54)).collect();
    let boot: Vec<P> = (0..6).map(|i| p(19.05 + (i % 3) as f64 * 2.54, 10.16 + (i / 3) as f64 * 2.54)).collect();
    let holes: Vec<(P, f64)> = vec![];
    let mut b = Build::board(BoardKind::BluePill.name(), rect_pts(P::ZERO, p(w, l)), &holes, t, [0.05, 0.22, 0.62]);
    let mut m = Model::default();
    let at: Vec<P> = pins.iter().map(|x| x.1).collect();
    male_pins(&mut m, &at, (-2.5, 0.0), -8.5, t + 1.0);
    male_pins(&mut m, &swd, (t, t + 2.5), t - 0.5, t + 8.5);
    male_pins(&mut m, &boot, (t, t + 2.5), t - 0.5, t + 8.5);
    for &q in &at {
        joint(&mut m, q, t);
    }
    for x in [19.05, 21.59] {
        m.boxx(Mat::Rgb(235, 190, 30), [x - 1.2, 8.9, t + 3.0], [x + 3.74, 11.4, t + 8.0]);
    }
    receptacle(&mut m, Receptacle::MicroB, p(-1.0, 11.43), p(-1.0, 0.0), t);
    qfp(&mut m, p(31.0, 11.43), 7.0, 12, 0.5, 45.0, t, "STM32F103");
    crystal(&mut m, p(42.0, 11.43), 8.0, 3.2, t, true);
    smd_button(&mut m, p(12.0, 11.43), 3.5, 6.0, t, Mat::White);
    for (i, (x, y)) in [(24.5, 6.6), (24.5, 16.3), (37.5, 6.6), (37.5, 16.3), (46.0, 7.0), (46.0, 16.0)].into_iter().enumerate() {
        chip(&mut m, p(x, y), 1.6, 0.8, false, t, if i % 2 == 0 { Mat::Ceramic } else { BLACK });
    }
    for &(n, q) in &pins {
        let y = if q.y < l / 2.0 { q.y + 2.6 } else { q.y - 2.6 };
        text(&mut m, SILK, n.trim_start_matches('P'), p(q.x, y), 0.6, 90.0, t);
    }
    text(&mut m, SILK, "BOOT0", p(20.3, 7.8), 0.6, 0.0, t);
    text(&mut m, SILK, "BOOT1", p(20.3, 15.1), 0.6, 0.0, t);
    text(&mut m, SILK, "PWR", p(7.0, 7.2), 0.6, 0.0, t);
    text(&mut m, SILK, "PC13", p(7.0, 15.6), 0.6, 0.0, t);
    b.model(&m);
    let (off, on) = LedColor::Red.rgb();
    b.lamp("PWR", off, on, &smd_led(p(7.0, 8.6), t));
    let (off, on) = LedColor::Green.rgb();
    b.lamp("PC13", off, on, &smd_led(p(7.0, 14.2), t));
    for &(n, q) in &pins {
        b.pin(n, [q.x, q.y, t + 1.0]);
    }
    b.done(Kind::Board(BoardKind::BluePill))
}

fn nucleo() -> DevicePart {
    let (w, l, t) = (70.0, 82.5, 1.6);
    // the Arduino shield pattern turned so the digital header is on the right
    let (x0, y0) = (8.3, 13.5);
    let headers: Vec<(&str, P)> = shield_headers(true).into_iter().map(|(n, q)| (n, p(x0 + q.y, y0 + q.x))).collect();
    let morpho: Vec<P> = [3.81, 6.35, 63.65, 66.19].iter().flat_map(|&x| (0..19).map(move |i| p(x, 31.0 + i as f64 * 2.54))).collect();
    let mounts = [p(4.5, 24.0), p(65.5, 24.0), p(4.5, 80.0), p(65.5, 80.0)];
    let holes: Vec<(P, f64)> = mounts.iter().map(|&q| (q, 3.2)).collect();
    let mut b = Build::board(BoardKind::Nucleo64.name(), rect_pts(P::ZERO, p(w, l)), &holes, t, [0.90, 0.90, 0.88]);
    let ink = Mat::Rgb(25, 25, 25);
    let mut m = Model::default();
    let dirs = |a: &str| headers.iter().find(|h| h.0 == a).unwrap().1;
    female_header(&mut m, dirs("D15"), p(0.0, 1.0), 10, t);
    female_header(&mut m, dirs("D7"), p(0.0, 1.0), 8, t);
    female_header(&mut m, dirs("NC"), p(0.0, 1.0), 8, t);
    female_header(&mut m, dirs("A0"), p(0.0, 1.0), 6, t);
    male_pins(&mut m, &morpho, (t, t + 2.5), t - 0.5, t + 8.5);
    receptacle(&mut m, Receptacle::MiniB, p(35.0, -1.0), p(0.0, -1.0), t);
    qfp(&mut m, p(35.0, 50.0), 10.0, 16, 0.5, 0.0, t, "STM32F401RE");
    qfp(&mut m, p(35.0, 15.0), 7.0, 12, 0.5, 0.0, t, "STM32F103CB");
    crystal(&mut m, p(46.0, 15.0), 3.2, 2.5, t, false);
    // ST-LINK cut line and connectors
    m.boxx(ink, [1.0, 23.0, t], [69.0, 23.25, t + 0.04]);
    let cn2: Vec<P> = (0..2).map(|i| p(53.0 + i as f64 * 2.54, 6.0)).collect();
    male_pins(&mut m, &cn2, (t, t + 2.5), t - 0.5, t + 8.5);
    m.boxx(Mat::Rgb(20, 20, 20), [51.7, 4.7, t + 3.0], [56.8, 7.3, t + 8.0]);
    // user and reset buttons
    m.boxx(BLACK, [17.0, 33.0, t], [23.0, 39.0, t + 3.5]);
    m.cyl(Mat::Rgb(30, 80, 200), [20.0, 36.0, t + 3.5], [20.0, 36.0, t + 5.0], 1.8, 20);
    m.boxx(BLACK, [47.0, 33.0, t], [53.0, 39.0, t + 3.5]);
    m.cyl(BLACK, [50.0, 36.0, t + 3.5], [50.0, 36.0, t + 5.0], 1.8, 20);
    for (i, (x, y)) in [(28.0, 42.0), (42.0, 42.0), (28.0, 58.0), (42.0, 58.0), (25.0, 18.0), (45.0, 20.0)].into_iter().enumerate() {
        chip(&mut m, p(x, y), 1.6, 0.8, i % 2 == 1, t, if i % 2 == 0 { Mat::Ceramic } else { BLACK });
    }
    text(&mut m, ink, "NUCLEO-F401RE", p(35.0, 66.0), 2.4, 0.0, t);
    text(&mut m, ink, "ST-LINK", p(20.0, 12.0), 1.5, 0.0, t);
    text(&mut m, ink, "B1 USER", p(20.0, 41.0), 1.0, 0.0, t);
    text(&mut m, ink, "B2 RESET", p(50.0, 41.0), 1.0, 0.0, t);
    text(&mut m, ink, "LD2", p(44.0, 30.0), 0.9, 0.0, t);
    text(&mut m, ink, "LD3", p(56.0, 21.0), 0.9, 0.0, t);
    text(&mut m, ink, "LD1", p(26.0, 6.5), 0.9, 0.0, t);
    for &(n, q) in &headers {
        let x = if q.x < w / 2.0 { q.x + 3.4 } else { q.x - 3.4 };
        text(&mut m, ink, n, p(x, q.y), 0.75, 0.0, t);
    }
    b.model(&m);
    let (off, on) = LedColor::Green.rgb();
    b.lamp("LD2", off, on, &smd_led(p(44.0, 28.0), t));
    let (off, on) = LedColor::Red.rgb();
    b.lamp("LD3", off, on, &smd_led(p(56.0, 19.0), t));
    b.lamp("LD1", off, on, &smd_led(p(26.0, 4.5), t));
    for &(n, q) in &headers {
        b.pin(n, [q.x, q.y, t + 8.5]);
    }
    b.done(Kind::Board(BoardKind::Nucleo64))
}

// ------------------------------------------------------------------ components

fn led(c: LedColor) -> DevicePart {
    let (off, on) = c.rgb();
    let mut b = Build::new(&Kind::Led(c).name(), off, 1.2e-6);
    b.extrude("Kragen", Sketch::circle(2.9, 0.0, 0.0), 1.0, off);
    let mut lens = Model::default();
    lens.cyl(Mat::White, [0.0, 0.0, 1.0], [0.0, 0.0, 6.1], 2.5, 32);
    lens.dome(Mat::White, [0.0, 0.0, 6.1], 2.5);
    b.lamp("LED", off, on, &lens);
    let mut legs = Model::default();
    legs.boxx(Mat::Metal, [1.02, -0.25, -27.0], [1.52, 0.25, 0.0]);
    legs.boxx(Mat::Metal, [-1.52, -0.25, -25.0], [-1.02, 0.25, 0.0]);
    b.model(&legs);
    b.pin("A", [1.27, 0.0, -27.0]);
    b.pin("K", [-1.27, 0.0, -25.0]);
    b.done(Kind::Led(c))
}

fn resistor(ohms: u32) -> DevicePart {
    let beige = Mat::Beige.rgb();
    let mut b = Build::new(&Kind::Resistor(ohms).name(), beige, 2.0e-6);
    // body along +X from the RECHTS plane, axis 1,25 mm above the underside
    let FeatureKind::Extrude(mut e) = FeatureKind::new_extrude() else { unreachable!() };
    e.plane = Some(PlaneRef::Datum(RIGHT_ID));
    e.sketch = Sketch::circle(1.25, 0.0, 1.25);
    e.depth = Dim::v(6.5);
    let id = b.push("Körper", FeatureKind::Extrude(e));
    b.m.feature_colors.insert(id, beige);
    let mut m = Model::default();
    let axis = 1.25;
    if let Some(bands) = super::model3d::bands(ohms as f64) {
        for (k, x) in bands.iter().zip([1.2, 2.1, 3.0, 5.0]) {
            m.cyl(Mat::Band(*k), [x, 0.0, axis], [x + 0.55, 0.0, axis], 1.3, 24);
        }
    }
    for (a, e) in [(0.0, -1.83), (6.5, 8.33)] {
        m.cyl(Mat::Metal, [a, 0.0, axis], [e, 0.0, axis], 0.3, 8);
        m.cyl(Mat::Metal, [e, 0.0, axis], [e, 0.0, -8.0], 0.3, 8);
    }
    b.model(&m);
    b.pin("1", [-1.83, 0.0, -8.0]);
    b.pin("2", [8.33, 0.0, -8.0]);
    b.done(Kind::Resistor(ohms))
}

fn tact(big: bool) -> DevicePart {
    let kind = if big { Kind::Tact12 } else { Kind::Tact6 };
    let black = BLACK.rgb();
    let mut b = Build::new(&kind.name(), black, 1.3e-6);
    let (s, h, lx, ly) = if big { (12.0, 3.8, 6.25, 2.5) } else { (6.0, 3.5, 3.25, 2.25) };
    b.extrude_rect("Gehäuse", p(-s / 2.0, -s / 2.0), p(s / 2.0, s / 2.0), h, black);
    let mut m = Model::default();
    // the cover is flush with the body top, so the switch can be mated against a panel
    m.boxx(Mat::Metal, [-s / 2.0 - 0.05, -s / 2.0 - 0.05, h - 0.3], [s / 2.0 + 0.05, s / 2.0 + 0.05, h + 0.01]);
    let mut cap = Model::default();
    if big {
        m.boxx(BLACK, [-2.0, -2.0, h], [2.0, 2.0, h + 2.5]);
        cap.cyl(Mat::Rgb(200, 30, 30), [0.0, 0.0, h + 2.5], [0.0, 0.0, h + 6.5], 5.75, 40);
    } else {
        cap.cyl(BLACK, [0.0, 0.0, h], [0.0, 0.0, 7.0], 1.75, 24);
    }
    for (x, y) in [(-lx, -ly), (lx, -ly), (-lx, ly), (lx, ly)] {
        m.boxx(Mat::Metal, [x - 0.35, y - 0.15, -3.5], [x + 0.35, y + 0.15, 0.5]);
    }
    b.model(&m);
    for (mat, v, t) in &cap.parts {
        b.import("Stößel", mat.rgb(), vec![world_face(v, t)]);
    }
    for (n, (x, y)) in [("1", (-lx, -ly)), ("2", (lx, -ly)), ("3", (-lx, ly)), ("4", (lx, ly))] {
        b.pin(n, [x, y, -3.5]);
    }
    b.done(kind)
}

fn header(n: u8) -> DevicePart {
    let n = n.clamp(1, 40);
    let black = BLACK.rgb();
    let mut b = Build::new(&Kind::Header(n).name(), black, 1.5e-6);
    let len = n as f64 * 2.54;
    b.extrude_rect("Leiste", p(0.0, -1.27), p(len, 1.27), 2.5, black);
    let mut m = Model::default();
    for i in 0..n {
        let x = 1.27 + i as f64 * 2.54;
        m.boxx(Mat::Gold, [x - 0.32, -0.32, -3.0], [x + 0.32, 0.32, 8.5]);
        b.pin(&(i + 1).to_string(), [x, 0.0, 8.5]);
    }
    b.model(&m);
    b.done(Kind::Header(n))
}

fn usb_breakout(c: bool) -> DevicePart {
    let kind = if c { Kind::UsbC } else { Kind::MicroUsb };
    let names: &[&str] = if c { &["GND", "VBUS"] } else { &["VBUS", "D-", "D+", "ID", "GND"] };
    let (w, l) = if c { (14.0, 12.0) } else { (15.0, 14.0) };
    let pins: Vec<(&str, P)> = names.iter().enumerate().map(|(i, &n)| (n, p(w / 2.0 + (i as f64 - (names.len() as f64 - 1.0) / 2.0) * 2.54, l - 2.0))).collect();
    let holes: Vec<(P, f64)> = vec![];
    let color = if c { [0.08, 0.08, 0.09] } else { [0.45, 0.10, 0.45] };
    let mut b = Build::board(&kind.name(), rect_pts(P::ZERO, p(w, l)), &holes, 1.6, color);
    let t = 1.6;
    let mut m = Model::default();
    receptacle(&mut m, if c { Receptacle::TypeC } else { Receptacle::MicroB }, p(w / 2.0, -1.0), p(0.0, -1.0), t);
    let at: Vec<P> = pins.iter().map(|x| x.1).collect();
    male_pins(&mut m, &at, (-2.5, 0.0), -8.5, t + 1.0);
    for &q in &at {
        joint(&mut m, q, t);
    }
    for &(n, q) in &pins {
        text(&mut m, SILK, n, q - p(0.0, 2.6), 0.7, 90.0, t);
    }
    b.model(&m);
    for &(n, q) in &pins {
        b.pin(n, [q.x, q.y, -8.5]);
    }
    b.done(kind)
}

fn dht22() -> DevicePart {
    let white = [0.94, 0.94, 0.92];
    let mut b = Build::new(&Kind::Dht22.name(), white, 1.1e-6);
    let (w, l, h) = (15.1, 25.0, 7.7);
    b.extrude_rect("Gehäuse", P::ZERO, p(w, l), h, white);
    let mut m = Model::default();
    for i in 0..4 {
        for j in 0..6 {
            let (x, y) = (2.2 + i as f64 * 3.0, 2.0 + j as f64 * 2.6);
            m.boxx(Mat::Rgb(70, 70, 70), [x, y, h], [x + 1.8, y + 1.6, h + 0.02]);
        }
    }
    text(&mut m, Mat::Rgb(30, 30, 30), "DHT22", p(w / 2.0, 20.3), 1.6, 0.0, h);
    let names = ["VCC", "DATA", "NC", "GND"];
    for i in 0..4 {
        let x = w / 2.0 + (i as f64 - 1.5) * 2.54;
        m.boxx(Mat::Metal, [x - 0.3, l, 0.8], [x + 0.3, l + 7.5, 1.2]);
        text(&mut m, Mat::Rgb(30, 30, 30), &(i + 1).to_string(), p(x, 23.3), 0.9, 0.0, h);
    }
    b.model(&m);
    for (i, n) in names.iter().enumerate() {
        b.pin(n, [w / 2.0 + (i as f64 - 1.5) * 2.54, l + 7.5, 1.0]);
    }
    b.done(Kind::Dht22)
}

fn hcsr04() -> DevicePart {
    let (w, l, t) = (45.0, 20.0, 1.6);
    let names = ["VCC", "TRIG", "ECHO", "GND"];
    let pins: Vec<(&str, P)> = names.iter().enumerate().map(|(i, &n)| (n, p(w / 2.0 + (i as f64 - 1.5) * 2.54, 18.3))).collect();
    let holes: Vec<(P, f64)> = vec![(p(1.5, 1.5), 1.0), (p(43.5, 1.5), 1.0), (p(1.5, 18.5), 1.0), (p(43.5, 18.5), 1.0)];
    let mut b = Build::board(&Kind::HcSr04.name(), rect_pts(P::ZERO, p(w, l)), &holes, t, [0.05, 0.25, 0.60]);
    let mut m = Model::default();
    for (x, label) in [(9.5, "T"), (35.5, "R")] {
        m.cyl(Mat::Metal, [x, 10.0, t], [x, 10.0, t + 11.6], 8.0, 48);
        m.cone(Mat::Metal, [x, 10.0, t + 11.6], [x, 10.0, t + 12.0], 8.0, 7.6, 48);
        m.cyl(Mat::Rgb(45, 45, 48), [x, 10.0, t + 11.9], [x, 10.0, t + 12.03], 6.8, 48);
        // woven grille over the opening
        let wire = Mat::Rgb(150, 152, 158);
        for k in -6..=6 {
            let o = k as f64 * 1.0;
            let half = (6.6f64 * 6.6 - o * o).max(0.0).sqrt();
            m.boxx(wire, [x - half, 10.0 + o - 0.12, t + 12.03], [x + half, 10.0 + o + 0.12, t + 12.08]);
            m.boxx(wire, [x + o - 0.12, 10.0 - half, t + 12.08], [x + o + 0.12, 10.0 + half, t + 12.13]);
        }
        text(&mut m, SILK, label, p(x + if x < 20.0 { 9.5 } else { -9.5 }, 2.2), 1.2, 0.0, t);
    }
    crystal(&mut m, p(22.5, 4.5), 6.0, 2.4, t, false);
    qfn(&mut m, p(22.5, 12.0), 3.0, 4.0, t, "");
    text(&mut m, SILK, "HC-SR04", p(22.5, 8.2), 1.2, 0.0, t);
    let at: Vec<P> = pins.iter().map(|x| x.1).collect();
    male_pins(&mut m, &at, (-2.5, 0.0), -8.5, t + 1.0);
    for &q in &at {
        joint(&mut m, q, t);
    }
    for &(n, q) in &pins {
        text(&mut m, SILK, n, q - p(0.0, 2.9), 0.6, 90.0, t);
    }
    b.model(&m);
    for &(n, q) in &pins {
        b.pin(n, [q.x, q.y, -8.5]);
    }
    b.done(Kind::HcSr04)
}

fn battery() -> DevicePart {
    let black = [0.09, 0.09, 0.10];
    let mut b = Build::new(&Kind::Battery9V.name(), black, 2.4e-6);
    let (w, d, h) = (26.5, 17.5, 46.0);
    b.extrude_rect("Gehäuse", P::ZERO, p(w, d), h, black);
    let mut m = Model::default();
    // gold label band around the middle
    let (z0, z1, e) = (8.0, 38.0, 0.05);
    m.boxx(Mat::Rgb(210, 150, 30), [-e, -e, z0], [w + e, 0.0, z1]);
    m.boxx(Mat::Rgb(210, 150, 30), [-e, d, z0], [w + e, d + e, z1]);
    m.boxx(Mat::Rgb(210, 150, 30), [-e, 0.0, z0], [0.0, d, z1]);
    m.boxx(Mat::Rgb(210, 150, 30), [w, 0.0, z0], [w + e, d, z1]);
    m.boxx(Mat::Metal, [1.0, 1.0, h], [w - 1.0, d - 1.0, h + 0.6]);
    let (xp, xn, y) = (w / 2.0 - 6.35, w / 2.0 + 6.35, 6.5);
    m.cyl(Mat::Metal, [xp, y, h + 0.6], [xp, y, h + 3.0], 2.9, 24);
    m.prism(Mat::Metal, &circle_ring(p(xn, y), 4.0, 6), h + 0.6, h + 3.0);
    text(&mut m, Mat::Rgb(30, 30, 30), "+", p(xp, 13.5), 2.5, 0.0, h + 0.6);
    text(&mut m, Mat::Rgb(30, 30, 30), "-", p(xn, 13.5), 2.5, 0.0, h + 0.6);
    text(&mut m, Mat::Rgb(30, 30, 30), "9V", p(w / 2.0, 14.0), 2.2, 0.0, h + 0.6);
    b.model(&m);
    b.pin("+", [xp, y, h + 3.0]);
    b.pin("-", [xn, y, h + 3.0]);
    b.done(Kind::Battery9V)
}

/// Geometry and pins of a library electronics part.
pub fn build(kind: Kind) -> DevicePart {
    match kind {
        Kind::Board(BoardKind::ArduinoUno) => uno(),
        Kind::Board(BoardKind::ArduinoNano) => nano(),
        Kind::Board(BoardKind::Esp32DevKit) => from_module(BoardKind::Esp32DevKit, Module::Esp32),
        Kind::Board(BoardKind::PiPico) => pico_variant(None),
        Kind::Board(BoardKind::Sbc(s)) => sbc::build(s),
        Kind::Board(BoardKind::BluePill) => blue_pill(),
        Kind::Board(BoardKind::Nucleo64) => nucleo(),
        Kind::Board(BoardKind::Pi4) => from_module(BoardKind::Pi4, Module::Pi4),
        Kind::Led(c) => led(c),
        Kind::Resistor(r) => resistor(r),
        Kind::Tact6 => tact(false),
        Kind::Tact12 => tact(true),
        Kind::Header(n) => header(n),
        Kind::UsbC => usb_breakout(true),
        Kind::MicroUsb => usb_breakout(false),
        Kind::Dht22 => dht22(),
        Kind::HcSr04 => hcsr04(),
        Kind::Battery9V => battery(),
    }
}
