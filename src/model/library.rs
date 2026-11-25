//! The Assembler's built-in part library: microcontroller boards, electronic parts,
//! fasteners and enclosures, each built as an ordinary part (features and colour bodies)
//! that is placed like any other. Electronics carry a [`Device`] for the circuit simulation.

use super::features::*;
use super::threads::ISO;
use crate::kernel::math::{V2, V3};
use crate::pcb::devices::{self, BoardKind, Device, Kind, LedColor};
use crate::sketch::sketch::Sketch;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Boards,
    RaspberryPi,
    OrangePi,
    Electronics,
    Fasteners,
    Enclosures,
}

impl Category {
    pub const ALL: [Category; 6] = [Category::Boards, Category::RaspberryPi, Category::OrangePi, Category::Electronics, Category::Fasteners, Category::Enclosures];
    pub fn label(self) -> &'static str {
        match self {
            Category::Boards => "Mikrocontroller",
            Category::RaspberryPi => "Raspberry Pi",
            Category::OrangePi => "Orange Pi",
            Category::Electronics => "Bauteile",
            Category::Fasteners => "Befestigung",
            Category::Enclosures => "Gehäuse",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Item {
    Device(Kind),
    /// open box (outer size, wall), centred on the origin, bottom on OBEN
    Box { w: f64, d: f64, h: f64, wall: f64 },
    /// flat lid with optional LED (Ø 5,1) and button (Ø 4) holes
    Lid { w: f64, d: f64, wall: f64, led: bool, button: bool },
    /// ISO 4032 hex nut, index into the ISO thread table
    Nut(usize),
    /// brass hex standoff M3, female–female
    Standoff(u16),
    /// opens the screw dialog
    Screw,
}

pub struct LibPart {
    pub data: ModelData,
    pub device: Option<Device>,
}

/// The library with its default sizes and values.
pub fn catalog() -> Vec<Item> {
    let mut v: Vec<Item> = BoardKind::all().into_iter().map(|b| Item::Device(Kind::Board(b))).collect();
    v.extend(LedColor::ALL.iter().map(|&c| Item::Device(Kind::Led(c))));
    v.extend([
        Item::Device(Kind::Resistor(220)),
        Item::Device(Kind::Tact6),
        Item::Device(Kind::Tact12),
        Item::Device(Kind::Header(8)),
        Item::Device(Kind::UsbC),
        Item::Device(Kind::MicroUsb),
        Item::Device(Kind::Dht22),
        Item::Device(Kind::HcSr04),
        Item::Device(Kind::Battery9V),
        Item::Screw,
        Item::Nut(2),
        Item::Standoff(10),
        Item::Box { w: 130.0, d: 70.0, h: 35.0, wall: 2.0 },
        Item::Lid { w: 130.0, d: 70.0, wall: 2.0, led: true, button: true },
    ]);
    v
}

impl Item {
    pub fn category(self) -> Category {
        match self {
            Item::Device(Kind::Board(b)) if b.raspberry() => Category::RaspberryPi,
            Item::Device(Kind::Board(b)) if b.orange() => Category::OrangePi,
            Item::Device(Kind::Board(_)) => Category::Boards,
            Item::Device(_) => Category::Electronics,
            Item::Nut(_) | Item::Standoff(_) | Item::Screw => Category::Fasteners,
            Item::Box { .. } | Item::Lid { .. } => Category::Enclosures,
        }
    }
    pub fn name(self) -> String {
        match self {
            Item::Device(k) => k.name(),
            Item::Box { w, d, h, .. } => format!("Gehäuse {} × {} × {} mm", w, d, h),
            Item::Lid { w, d, .. } => format!("Deckel {} × {} mm", w, d),
            Item::Nut(i) => format!("Sechskantmutter {}", ISO.get(i).map_or("?", |s| s.name)),
            Item::Standoff(l) => format!("Abstandsbolzen M3 × {l}"),
            Item::Screw => "Schraube…".into(),
        }
    }
    pub fn description(self) -> String {
        match self {
            Item::Device(k) => k.description(),
            Item::Box { wall, .. } => format!("PLA, oben offen, Wandstärke {wall} mm"),
            Item::Lid { led, button, .. } => {
                let mut s = "PLA-Deckel zum Gehäuse".to_string();
                if led {
                    s += ", LED-Bohrung Ø 5,1";
                }
                if button {
                    s += ", Tasterbohrung Ø 4";
                }
                s
            }
            Item::Nut(_) => "ISO 4032, Stahl verzinkt".into(),
            Item::Standoff(_) => "Messing, Schlüsselweite 5,5, Innengewinde beidseitig".into(),
            Item::Screw => "Metrische Schraube mit Kopf, Gewinde und Länge wählen".into(),
        }
    }

    pub fn build(self) -> Result<LibPart, String> {
        match self {
            Item::Device(k) => {
                let p = devices::build(k);
                Ok(LibPart { data: p.data, device: Some(p.device) })
            }
            Item::Box { w, d, h, wall } => enclosure(w, d, h, wall).map(|data| LibPart { data, device: None }),
            Item::Lid { w, d, wall, led, button } => Ok(LibPart { data: lid(w, d, wall, led, button), device: None }),
            Item::Nut(i) => nut(i).map(|data| LibPart { data, device: None }),
            Item::Standoff(l) => Ok(LibPart { data: standoff(l), device: None }),
            Item::Screw => Err("Schrauben werden im Schraubendialog gewählt.".into()),
        }
    }
}

fn feature(m: &mut ModelData, name: &str, kind: FeatureKind) -> u64 {
    let id = m.next_id;
    m.features.push(Feature { id, name: name.into(), kind, suppressed: false, hidden: false });
    m.next_id += 1;
    id
}

fn extrude(sketch: Sketch, depth: f64) -> FeatureKind {
    let FeatureKind::Extrude(mut e) = FeatureKind::new_extrude() else { unreachable!() };
    e.plane = Some(PlaneRef::Datum(TOP_ID));
    e.sketch = sketch;
    e.depth = Dim::v(depth);
    FeatureKind::Extrude(e)
}

fn param(m: &mut ModelData, name: &str, v: f64, note: &str) {
    m.parameters.push(Parameter { name: name.into(), expr: format!("{v}"), note: note.into() });
}

const PLA: f64 = 1.24e-6;

pub fn enclosure(w: f64, d: f64, h: f64, wall: f64) -> Result<ModelData, String> {
    if wall <= 0.0 || 2.0 * wall >= w.min(d) || wall >= h {
        return Err("Die Wandstärke passt nicht zum Gehäuse.".into());
    }
    let mut m = ModelData::new_part_template(&format!("GEHAEUSE_{}x{}x{}", w, d, h));
    m.appearance.color = [0.28, 0.33, 0.40];
    m.density = PLA;
    param(&mut m, "BREITE", w, "Außenmaß X");
    param(&mut m, "TIEFE", d, "Außenmaß Z");
    param(&mut m, "HOEHE", h, "Außenhöhe");
    feature(&mut m, "Block", extrude(Sketch::rect(w, d, 0.0, 0.0), h));
    feature(&mut m, "Schale", FeatureKind::Shell(ShellDef { faces: vec![FaceRef { point: V3::new(0.0, h, 0.0), normal: V3::Y }], thickness: Dim::v(wall) }));
    Ok(m)
}

pub fn lid(w: f64, d: f64, wall: f64, led: bool, button: bool) -> ModelData {
    let mut m = ModelData::new_part_template(&format!("DECKEL_{}x{}", w, d));
    m.appearance.color = [0.78, 0.80, 0.83];
    m.density = PLA;
    let mut sk = Sketch::rect(w, d, 0.0, 0.0);
    if led {
        let c = sk.add_point(V2::new(-w / 4.0, 0.0));
        sk.add_circle(c, 2.55);
    }
    if button {
        let c = sk.add_point(V2::new(w / 4.0, 0.0));
        sk.add_circle(c, 2.0);
    }
    feature(&mut m, "Deckel", extrude(sk, wall));
    m
}

pub fn nut(i: usize) -> Result<ModelData, String> {
    let spec = ISO.get(i).ok_or("Unbekanntes Gewinde")?;
    // ISO 4032 width across flats and height
    let (s, h) = match spec.name {
        "M2" => (4.0, 1.6),
        "M2.5" => (5.0, 2.0),
        "M3" => (5.5, 2.4),
        "M4" => (7.0, 3.2),
        "M5" => (8.0, 4.7),
        "M6" => (10.0, 5.2),
        "M8" => (13.0, 6.8),
        "M10" => (16.0, 8.4),
        _ => (1.5 * spec.major + 1.0, 0.8 * spec.major),
    };
    let mut m = ModelData::new_part_template(&format!("MUTTER_{}", spec.name.replace('.', "_")));
    m.appearance.color = [0.70, 0.72, 0.75];
    m.density = 7.85e-6;
    m.features.push(Feature { id: m.next_id, name: "Sechskant".into(), kind: extrude(hex(s, spec.major / 2.0), h), suppressed: false, hidden: false });
    m.next_id += 1;
    Ok(m)
}

fn hex(s: f64, hole: f64) -> Sketch {
    let r = s / 3f64.sqrt();
    let mut sk = Sketch::default();
    let ids: Vec<i64> = (0..6).map(|i| sk.add_point(V2::new(r * (i as f64 * std::f64::consts::FRAC_PI_3).cos(), r * (i as f64 * std::f64::consts::FRAC_PI_3).sin()))).collect();
    for i in 0..6 {
        sk.add_line(ids[i], ids[(i + 1) % 6]);
    }
    let c = sk.add_point(V2::ZERO);
    sk.add_circle(c, hole);
    sk
}

pub fn standoff(l: u16) -> ModelData {
    let mut m = ModelData::new_part_template(&format!("ABSTANDSBOLZEN_M3x{l}"));
    m.appearance.color = [0.80, 0.66, 0.32];
    m.density = 8.5e-6;
    feature(&mut m, "Sechskant", extrude(hex(5.5, 1.25), f64::from(l.max(1))));
    m
}

/// Demo product: enclosure with an Arduino Uno on the floor, a 9 V battery beside it, and a
/// lid carrying an LED (through a 220 Ω resistor on D9) and a push button on D2. The program
/// lights the LED while the button is pressed and blinks the on-board LED.
pub fn demo_product() -> super::assembly::AssemblyData {
    use super::assembly::*;
    use crate::pcb::circuit::{Mode, PinRef};
    let mut asm = AssemblyData::new("PROTOTYP_ASM");
    let (w, d, h, wall) = (130.0, 70.0, 35.0, 2.0);
    let add = |asm: &mut AssemblyData, item: Item, pl: Placement| -> u64 {
        let lp = item.build().expect("library part");
        let name = asm.add_part(lp.data, None);
        asm.part_mut(&name).unwrap().device = lp.device;
        let id = asm.next_id;
        let mut c = Component::new(id, &name, pl);
        c.constraints.push(AsmConstraint::new(if id == FIRST_COMPONENT_ID { AsmCType::Default } else { AsmCType::Fix }));
        asm.components.push(c);
        asm.next_id += 1;
        id
    };
    let at = |x: f64, y: f64, z: f64| Placement::translation(V3::new(x, y, z));
    let enclosure = add(&mut asm, Item::Box { w, d, h, wall }, Placement::default());
    let uno = add(&mut asm, Item::Device(Kind::Board(BoardKind::ArduinoUno)), at(-56.0, wall, -26.67));
    let battery = add(&mut asm, Item::Device(Kind::Battery9V), Placement { rot: glam::DQuat::from_rotation_x(std::f64::consts::FRAC_PI_2), pos: V3::new(22.0, wall + 17.5, -25.0) });
    let _lid = add(&mut asm, Item::Lid { w, d, wall, led: true, button: true }, at(0.0, h, 0.0));
    let led = add(&mut asm, Item::Device(Kind::Led(LedColor::Red)), at(-w / 4.0, h - 1.0, 0.0));
    let button = add(&mut asm, Item::Device(Kind::Tact6), at(w / 4.0, h - 3.5, 0.0));
    let resistor = add(&mut asm, Item::Device(Kind::Resistor(220)), at(-30.0, 20.0, 12.0));
    let _ = enclosure;
    let w_ = &mut asm.wiring;
    let pr = PinRef::new;
    for (a, b) in [
        (pr(battery, "+"), pr(uno, "VIN")),
        (pr(battery, "-"), pr(uno, "GND")),
        (pr(uno, "D9"), pr(resistor, "1")),
        (pr(resistor, "2"), pr(led, "A")),
        (pr(led, "K"), pr(uno, "GND")),
        (pr(uno, "D2"), pr(button, "1")),
        (pr(button, "3"), pr(uno, "GND")),
    ] {
        w_.connect(a, b);
    }
    w_.set_mode(uno, "D2", Mode::Pullup);
    w_.set_mode(uno, "D9", Mode::Follow { input: "D2".into(), invert: true });
    w_.set_mode(uno, "D13", Mode::Blink(500));
    asm
}
