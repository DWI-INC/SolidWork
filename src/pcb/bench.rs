//! Static GPIO bench. Contacts are grouped by touching copper, independently of net names.
//! The virtual USB supplies and GPIO outputs are ideal sources. The LED load model covers
//! a single series resistor on either side of a red LED (nominal forward drop 2 V).

use super::board::{Board, Item};
use super::modules::Module;
use super::symbols::PinType;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Drive {
    #[default]
    Open,
    Low,
    High,
}
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Level {
    Floating,
    Low,
    High,
    Five,
    Volts(f64),
    Conflict,
}

impl Level {
    pub fn label(self) -> String {
        match self {
            Self::Floating => "Ohne Testpegel".into(),
            Self::Low => "LOW · 0 V".into(),
            Self::High => "HIGH · 3,3 V".into(),
            Self::Five => "5 V".into(),
            Self::Volts(v) => format!("{v:.2} V"),
            Self::Conflict => "Treiberkonflikt".into(),
        }
    }
    fn measured(v: f64) -> Self {
        if v.abs() < 1e-9 {
            Self::Low
        } else if (v - 3.3).abs() < 1e-9 {
            Self::High
        } else if (v - 5.0).abs() < 1e-9 {
            Self::Five
        } else {
            Self::Volts(v)
        }
    }
    fn voltage(self) -> Option<f64> {
        match self {
            Self::Low => Some(0.0),
            Self::High => Some(3.3),
            Self::Five => Some(5.0),
            Self::Volts(v) => Some(v),
            _ => None,
        }
    }
}

pub struct Contact {
    pub reference: String,
    pub number: String,
    pub name: String,
    pub net: String,
    pub output: bool,
    pub drive: Drive,
    chip: Option<usize>,
    node: usize,
}

#[derive(Default)]
struct Rails {
    gnd: Option<usize>,
    five: Option<usize>,
    logic: Option<usize>,
    enable: Option<usize>,
}

pub struct Chip {
    module: Module,
    rails: Rails,
    pub label: String,
    pub usb: bool,
}

pub struct Supply {
    pub on: bool,
    pub five_volts: bool,
    pub positive: usize,
    pub ground: usize,
}

pub struct Bench {
    pub supply: Supply,
    pub contacts: Vec<Contact>,
    pub chips: Vec<Chip>,
    nodes: usize,
    resistors: Vec<(usize, usize, f64)>,
    leds: Vec<(String, usize, usize)>,
}

#[derive(Debug, PartialEq)]
pub enum LedState {
    Current(f64),
    MissingResistor,
    Unresolved,
}
pub struct Reading {
    pub pins: Vec<Level>,
    pub running: Vec<bool>,
    pub leds: Vec<(String, LedState)>,
}

impl Bench {
    pub fn new(board: &Board) -> Self {
        let cu = board.copper();
        let mut parent = board.copper_clusters(&cu, false);
        let mut contacts = Vec::new();
        let mut chips = vec![];
        let mut devices = HashMap::new();
        let mut rails = HashMap::new();
        let mut pads = HashMap::new();
        for (fi, f) in board.footprints.iter().enumerate() {
            if let Some(module) = Module::of(&f.fp) {
                devices.insert(fi, (chips.len(), module.pins()));
                chips.push(Chip { module, rails: Rails::default(), label: format!("{} · {}", f.reference, module.name()), usb: false });
            }
        }
        fn root(parent: &[usize], mut i: usize) -> usize {
            while i != parent[i] {
                i = parent[i];
            }
            i
        }
        for (i, c) in cu.iter().enumerate() {
            let Item::Pad(fi, k) = c.what else { continue };
            let f = &board.footprints[fi];
            let number = &f.fp.pads[k].num;
            let dev = devices.get(&fi);
            let pin = dev.and_then(|(_, pins)| pins.iter().find(|q| &q.number == number));
            let name = pin.map_or("", |q| q.name);
            // GNDs and duplicate supply terminals are joined on the module itself.
            if matches!(name, "GND" | "3V3" | "5V") {
                if let Some(prev) = rails.insert((fi, name), i) {
                    let (a, b) = (root(&parent, i), root(&parent, prev));
                    parent[a] = b;
                }
            }
            pads.insert((fi, number.as_str()), i);
            contacts.push(Contact {
                reference: f.reference.clone(),
                number: number.clone(),
                name: name.into(),
                net: c.net.clone(),
                output: pin.is_some_and(|q| q.kind == PinType::Bidi),
                drive: Drive::Open,
                chip: dev.map(|d| d.0),
                node: i,
            });
        }
        for contact in &mut contacts {
            contact.node = root(&parent, contact.node);
            if let Some(i) = contact.chip {
                let rails = &mut chips[i].rails;
                match contact.name.as_str() {
                    "5V" => rails.five = Some(contact.node),
                    "3V3" => rails.logic = Some(contact.node),
                    "GND" => rails.gnd = Some(contact.node),
                    "EN" => rails.enable = Some(contact.node),
                    _ => {}
                }
            }
        }
        let mut resistors = vec![];
        let mut leds = vec![];
        for (fi, f) in board.footprints.iter().enumerate() {
            let (Some(&one), Some(&two)) = (pads.get(&(fi, "1")), pads.get(&(fi, "2"))) else { continue };
            let (one, two) = (root(&parent, one), root(&parent, two));
            if f.fp.id.starts_with("Resistor_") {
                if let Some(r) = super::model3d::ohms(&f.value).filter(|r| r.is_finite() && *r > 0.0) {
                    resistors.push((one, two, r));
                }
            } else if f.fp.id.starts_with("LED_") {
                leds.push((f.reference.clone(), two, one));
            }
        }
        let supply = Supply {
            on: false,
            five_volts: true,
            positive: contacts.iter().position(|p| p.name == "5V").unwrap_or(0),
            ground: contacts.iter().position(|p| p.name == "GND").unwrap_or(0),
        };
        Self { contacts, chips, nodes: cu.len(), resistors, leds, supply }
    }

    pub fn read(&self) -> Reading {
        let mut masks = vec![0u8; self.nodes];
        if self.supply.on {
            if let (Some(pos), Some(gnd)) = (self.contacts.get(self.supply.positive), self.contacts.get(self.supply.ground)) {
                masks[pos.node] |= if self.supply.five_volts { 4 } else { 2 };
                masks[gnd.node] |= 1;
            }
        }
        for c in self.chips.iter().filter(|c| c.usb) {
            if let Some(n) = c.rails.five {
                masks[n] |= 4;
            }
            if let Some(n) = c.rails.gnd {
                masks[n] |= 1;
            }
        }
        let powered_5v = |c: &Chip, masks: &[u8]| c.rails.gnd.is_some_and(|n| masks[n] == 1) && c.rails.five.is_some_and(|n| masks[n] == 4);
        for c in &self.chips {
            if powered_5v(c, &masks) {
                if let Some(n) = c.rails.logic {
                    masks[n] |= 2;
                }
            }
        }
        let running: Vec<_> = self
            .chips
            .iter()
            .map(|c| {
                let r = &c.rails;
                let normal_rails = r.gnd.is_some_and(|n| masks[n] == 1) && r.logic.is_some_and(|n| masks[n] == 2);
                let supply_ok = powered_5v(c, &masks) || (c.module == Module::Esp32 && r.five.is_some_and(|n| masks[n] == 0));
                normal_rails && supply_ok && r.enable.is_none_or(|n| matches!(masks[n], 0 | 2))
            })
            .collect();
        for c in &self.contacts {
            if c.output && c.chip.is_some_and(|i| running[i]) {
                masks[c.node] |= match c.drive {
                    Drive::Low => 1,
                    Drive::High => 2,
                    Drive::Open => 0,
                };
            }
        }
        let mut levels: Vec<Level> = masks
            .into_iter()
            .map(|m| match m {
                0 => Level::Floating,
                1 => Level::Low,
                2 => Level::High,
                4 => Level::Five,
                _ => Level::Conflict,
            })
            .collect();
        // Thévenin equivalent for a pad biased by resistors to known sources. Cascaded
        // unknown networks remain unresolved. Never infer a voltage from a net label.
        let mut feeds = HashMap::<usize, (f64, f64)>::new();
        for &(a, b, r) in &self.resistors {
            if a == b {
                continue;
            }
            for (node, other) in [(a, b), (b, a)] {
                let (current, g) = feeds.entry(node).or_default();
                *g += 1.0 / r;
                *current += levels[other].voltage().unwrap_or(f64::NAN) / r;
            }
        }
        let mut loads = HashMap::<usize, usize>::new();
        for (_, a, k) in &self.leds {
            *loads.entry(*a).or_default() += 1;
            *loads.entry(*k).or_default() += 1;
        }
        let sources: HashMap<_, _> = feeds
            .into_iter()
            .filter_map(|(node, (current, g))| (levels[node] == Level::Floating && current.is_finite() && g > 0.0).then_some((node, (current / g, 1.0 / g))))
            .collect();
        for (&node, &(v, _)) in &sources {
            if !loads.contains_key(&node) {
                levels[node] = Level::measured(v);
            }
        }
        let mut leds = vec![];
        for (name, a, k) in &self.leds {
            let supply = |n: &usize| {
                if let Some(v) = levels[*n].voltage() {
                    return Some((v, 0.0));
                }
                if loads[n] == 1 {
                    sources.get(n).copied()
                } else {
                    None
                }
            };
            let state = match (supply(a), supply(k)) {
                (Some((va, ra)), Some((vk, rk))) if va - vk <= 2.0 || ra + rk > 0.0 => {
                    let amps = if va - vk <= 2.0 { 0.0 } else { (va - vk - 2.0) / (ra + rk) };
                    levels[*a] = Level::measured(va - amps * ra);
                    levels[*k] = Level::measured(vk + amps * rk);
                    LedState::Current(1000.0 * amps)
                }
                (Some(_), Some(_)) => LedState::MissingResistor,
                _ => LedState::Unresolved,
            };
            leds.push((name.clone(), state));
        }
        Reading { pins: self.contacts.iter().map(|c| levels[c.node]).collect(), leds, running }
    }
}
