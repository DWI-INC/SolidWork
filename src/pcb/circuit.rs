//! DC simulation of an assembled product: the library parts of an assembly (boards, LEDs,
//! resistors, buttons, batteries, USB sockets, sensors) connected by the wires drawn in
//! the Assembler. Like the GPIO bench, sources are ideal apart from a small internal
//! resistance and GPIOs are static levels, but every node is solved by nodal analysis, so
//! series resistors, LEDs of any colour, battery-fed boards and buttons read correctly.
//!
//! Elements: resistors, Norton sources (rails, GPIO drivers, pull-ups, batteries), LEDs as
//! piecewise-linear diodes (open, or Vf with 15 Ω), closed buttons. A board runs when USB is
//! plugged in or one of its supply inputs is fed from elsewhere; then its regulators drive
//! the other rails.

use super::bench::Drive;
use super::devices::{BoardKind, Device, Kind};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PinRef {
    pub comp: u64,
    pub pin: String,
}

impl PinRef {
    pub fn new(comp: u64, pin: &str) -> PinRef {
        PinRef { comp, pin: pin.into() }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Wire {
    pub a: PinRef,
    pub b: PinRef,
}

/// What the firmware does with a pin (the "program" of a board in the simulation).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Mode {
    Input,
    Pullup,
    Low,
    High,
    /// toggles every `ms` milliseconds
    Blink(u32),
    /// output copies the level read at another pin of the same board
    Follow { input: String, invert: bool },
}

impl Mode {
    pub fn label(&self) -> String {
        match self {
            Mode::Input => "Eingang".into(),
            Mode::Pullup => "Eingang mit Pull-up".into(),
            Mode::Low => "Ausgang LOW".into(),
            Mode::High => "Ausgang HIGH".into(),
            Mode::Blink(ms) => format!("Blinken {ms} ms"),
            Mode::Follow { input, invert } => format!("Ausgang = {}{input}", if *invert { "NICHT " } else { "" }),
        }
    }
    pub fn is_output(&self) -> bool {
        !matches!(self, Mode::Input | Mode::Pullup)
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Program {
    pub comp: u64,
    pub pin: String,
    pub mode: Mode,
}

/// Wires and pin programs, saved with the assembly.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Wiring {
    pub wires: Vec<Wire>,
    pub programs: Vec<Program>,
}

impl Wiring {
    pub fn is_empty(&self) -> bool {
        self.wires.is_empty() && self.programs.is_empty()
    }
    pub fn connect(&mut self, a: PinRef, b: PinRef) -> bool {
        if a == b || self.wires.iter().any(|w| (w.a == a && w.b == b) || (w.a == b && w.b == a)) {
            return false;
        }
        self.wires.push(Wire { a, b });
        true
    }
    pub fn mode(&self, comp: u64, pin: &str) -> Mode {
        self.programs.iter().find(|p| p.comp == comp && p.pin == pin).map(|p| p.mode.clone()).unwrap_or(Mode::Input)
    }
    pub fn set_mode(&mut self, comp: u64, pin: &str, mode: Mode) {
        self.programs.retain(|p| !(p.comp == comp && p.pin == pin));
        if mode != Mode::Input {
            self.programs.push(Program { comp, pin: pin.into(), mode });
        }
    }
    /// Drops wires and programs of components that no longer exist.
    pub fn retain_comps(&mut self, alive: &dyn Fn(u64) -> bool) {
        self.wires.retain(|w| alive(w.a.comp) && alive(w.b.comp));
        self.programs.retain(|p| alive(p.comp));
    }
}

/// Things the user does on the test bench (not saved).
#[derive(Clone, Debug, Default)]
pub struct Stimulus {
    /// boards and USB sockets with a cable plugged in
    pub usb: HashSet<u64>,
    /// pressed buttons: (component, "" for a switch part or the board button's name)
    pub pressed: HashSet<(u64, String)>,
    /// batteries taken out
    pub removed: HashSet<u64>,
    /// simulation time in seconds (blinking)
    pub time: f64,
}

pub struct Part<'a> {
    pub id: u64,
    pub label: String,
    pub device: &'a Device,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PinLevel {
    Floating,
    Volts(f64),
}

impl PinLevel {
    pub fn label(self) -> String {
        match self {
            PinLevel::Floating => "offen".into(),
            PinLevel::Volts(v) => format!("{:.2} V", v).replace('.', ","),
        }
    }
    pub fn volts(self) -> Option<f64> {
        match self {
            PinLevel::Volts(v) => Some(v),
            PinLevel::Floating => None,
        }
    }
}

#[derive(Default, Debug)]
pub struct Solution {
    /// every pin of every part (relative to the ground of its circuit)
    pub pins: HashMap<PinRef, PinLevel>,
    /// lamps that light up: (component, lamp) → brightness 0–1
    pub lamps: HashMap<(u64, String), f32>,
    pub running: HashSet<u64>,
    /// LED currents in mA
    pub led_ma: HashMap<u64, f64>,
    /// one status line per part
    pub status: Vec<(u64, String)>,
    pub warnings: Vec<String>,
}

impl Solution {
    pub fn level(&self, comp: u64, pin: &str) -> PinLevel {
        self.pins.get(&PinRef::new(comp, pin)).copied().unwrap_or(PinLevel::Floating)
    }
}

#[derive(Clone, Copy)]
enum El {
    R(usize, usize, f64),
    /// v(p) − v(n) = v behind conductance g
    Src { p: usize, n: usize, v: f64, g: f64, owner: usize },
    Led(usize, usize, usize),
}

const LED_G: f64 = 1.0 / 15.0;
const GPIO_G: f64 = 1.0 / 25.0;
const RAIL_G: f64 = 1.0 / 0.3;
const PULLUP_G: f64 = 1.0 / 35_000.0;

struct Net<'a> {
    parts: &'a [Part<'a>],
    /// node of (part index, pin name)
    node: HashMap<(usize, String), usize>,
    nodes: usize,
}

fn find(parent: &mut [usize], mut i: usize) -> usize {
    while parent[i] != i {
        parent[i] = parent[parent[i]];
        i = parent[i];
    }
    i
}

impl<'a> Net<'a> {
    fn build(parts: &'a [Part<'a>], wiring: &Wiring) -> Net<'a> {
        let mut raw: HashMap<(usize, String), usize> = HashMap::new();
        let add = |k: (usize, String), raw: &mut HashMap<(usize, String), usize>| {
            let n = raw.len();
            *raw.entry(k).or_insert(n)
        };
        for (i, part) in parts.iter().enumerate() {
            for name in part.device.pin_names() {
                add((i, name), &mut raw);
            }
            if let Kind::Board(b) = part.device.kind {
                let s = b.spec();
                for n in s.internal {
                    add((i, n.to_string()), &mut raw);
                }
                for (r, _) in s.usb {
                    add((i, r.to_string()), &mut raw);
                }
                add((i, "GND".into()), &mut raw);
            }
        }
        let mut parent: Vec<usize> = (0..raw.len()).collect();
        let join = |a: Option<usize>, b: Option<usize>, parent: &mut Vec<usize>| {
            if let (Some(a), Some(b)) = (a, b) {
                let (ra, rb) = (find(parent, a), find(parent, b));
                parent[ra] = rb;
            }
        };
        let index: HashMap<u64, usize> = parts.iter().enumerate().map(|(i, p)| (p.id, i)).collect();
        for w in &wiring.wires {
            let (Some(&ia), Some(&ib)) = (index.get(&w.a.comp), index.get(&w.b.comp)) else { continue };
            join(raw.get(&(ia, w.a.pin.clone())).copied(), raw.get(&(ib, w.b.pin.clone())).copied(), &mut parent);
        }
        for (i, part) in parts.iter().enumerate() {
            let get = |n: &str| raw.get(&(i, n.to_string())).copied();
            match part.device.kind {
                Kind::Tact6 | Kind::Tact12 => {
                    join(get("1"), get("2"), &mut parent);
                    join(get("3"), get("4"), &mut parent);
                }
                Kind::Board(b) => {
                    for (x, y) in b.spec().aliases {
                        join(get(x), get(y), &mut parent);
                    }
                }
                _ => {}
            }
        }
        let mut roots: HashMap<usize, usize> = HashMap::new();
        let mut node = HashMap::new();
        for (k, v) in raw {
            let r = find(&mut parent, v);
            let n = roots.len();
            let id = *roots.entry(r).or_insert(n);
            node.insert(k, id);
        }
        Net { parts, nodes: roots.len(), node }
    }
    fn at(&self, part: usize, pin: &str) -> Option<usize> {
        self.node.get(&(part, pin.to_string())).copied()
    }
}

/// Dense Gaussian elimination with partial pivoting; leaks keep every node defined.
fn solve_nodes(n: usize, els: &[El], led_on: &[bool], leds: &[(usize, usize, f64)]) -> Vec<f64> {
    let mut g = vec![0.0; n * n];
    let mut rhs = vec![0.0; n];
    for i in 0..n {
        g[i * n + i] += 1e-9;
    }
    let stamp = |a: usize, b: usize, c: f64, g: &mut Vec<f64>| {
        g[a * n + a] += c;
        g[b * n + b] += c;
        g[a * n + b] -= c;
        g[b * n + a] -= c;
    };
    for e in els {
        match *e {
            El::R(a, b, c) => stamp(a, b, c, &mut g),
            El::Src { p, n: m, v, g: c, .. } => {
                stamp(p, m, c, &mut g);
                rhs[p] += v * c;
                rhs[m] -= v * c;
            }
            El::Led(a, k, i) => {
                if led_on[i] {
                    stamp(a, k, LED_G, &mut g);
                    rhs[a] += leds[i].2 * LED_G;
                    rhs[k] -= leds[i].2 * LED_G;
                }
            }
        }
    }
    for col in 0..n {
        let piv = (col..n).max_by(|&a, &b| g[a * n + col].abs().partial_cmp(&g[b * n + col].abs()).unwrap()).unwrap();
        if g[piv * n + col].abs() < 1e-30 {
            continue;
        }
        if piv != col {
            for k in 0..n {
                g.swap(col * n + k, piv * n + k);
            }
            rhs.swap(col, piv);
        }
        let d = g[col * n + col];
        for row in col + 1..n {
            let f = g[row * n + col] / d;
            if f != 0.0 {
                for k in col..n {
                    g[row * n + k] -= f * g[col * n + k];
                }
                rhs[row] -= f * rhs[col];
            }
        }
    }
    let mut x = vec![0.0; n];
    for row in (0..n).rev() {
        let mut s = rhs[row];
        for k in row + 1..n {
            s -= g[row * n + k] * x[k];
        }
        let d = g[row * n + row];
        x[row] = if d.abs() < 1e-30 { 0.0 } else { s / d };
    }
    x
}

/// Solves with the LEDs settling on or off; returns node voltages and LED states.
fn solve(n: usize, els: &[El], leds: &[(usize, usize, f64)]) -> (Vec<f64>, Vec<bool>) {
    let mut on = vec![false; leds.len()];
    let mut v = solve_nodes(n, els, &on, leds);
    for _ in 0..40 {
        let mut changed = false;
        for (i, &(a, k, vf)) in leds.iter().enumerate() {
            // on: the current g·(v_a − v_k − vf) stays positive; off: the drop exceeds vf
            let want = v[a] - v[k] > vf + if on[i] { -1e-9 } else { 1e-6 };
            if want != on[i] {
                on[i] = want;
                changed = true;
            }
        }
        if !changed {
            break;
        }
        v = solve_nodes(n, els, &on, leds);
    }
    (v, on)
}

/// How a board is supplied in this solve.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Power {
    Off,
    Usb,
    /// index into `spec.inputs`
    Input(usize),
}

struct Sim<'a> {
    net: Net<'a>,
    wiring: &'a Wiring,
    stim: &'a Stimulus,
}

impl<'a> Sim<'a> {
    fn board(&self, i: usize) -> Option<BoardKind> {
        match self.net.parts[i].device.kind {
            Kind::Board(b) => Some(b),
            _ => None,
        }
    }

    /// GPIO drive of every board pin, given the power states and the levels read last time.
    fn drives(&self, power: &[Power], read: &HashMap<(usize, String), bool>) -> HashMap<(usize, String), Drive> {
        let mut out = HashMap::new();
        for p in &self.wiring.programs {
            let Some(i) = self.net.parts.iter().position(|x| x.id == p.comp) else { continue };
            let Some(b) = self.board(i) else { continue };
            if power[i] == Power::Off || !b.is_gpio(&p.pin) && !b.spec().internal.contains(&p.pin.as_str()) || b.input_only(&p.pin) {
                continue;
            }
            let d = match &p.mode {
                Mode::Low => Drive::Low,
                Mode::High => Drive::High,
                Mode::Blink(ms) => {
                    let k = (self.stim.time * 1000.0 / (*ms).max(1) as f64).floor() as i64;
                    if k % 2 == 0 {
                        Drive::High
                    } else {
                        Drive::Low
                    }
                }
                Mode::Follow { input, invert } => {
                    let level = read.get(&(i, input.clone())).copied().unwrap_or(false);
                    if level != *invert {
                        Drive::High
                    } else {
                        Drive::Low
                    }
                }
                _ => continue,
            };
            out.insert((i, p.pin.clone()), d);
        }
        out
    }

    fn elements(&self, power: &[Power], drives: &HashMap<(usize, String), Drive>, leds: &mut Vec<(usize, usize, f64)>, led_of: &mut Vec<usize>) -> Vec<El> {
        let mut els = vec![];
        leds.clear();
        led_of.clear();
        for (i, part) in self.net.parts.iter().enumerate() {
            let at = |n: &str| self.net.at(i, n);
            match part.device.kind {
                Kind::Resistor(r) => {
                    if let (Some(a), Some(b)) = (at("1"), at("2")) {
                        els.push(El::R(a, b, 1.0 / (r.max(1) as f64)));
                    }
                }
                Kind::Led(c) => {
                    if let (Some(a), Some(k)) = (at("A"), at("K")) {
                        els.push(El::Led(a, k, leds.len()));
                        leds.push((a, k, c.vf()));
                        led_of.push(i);
                    }
                }
                Kind::Tact6 | Kind::Tact12 => {
                    if self.stim.pressed.contains(&(part.id, String::new())) {
                        if let (Some(a), Some(b)) = (at("1"), at("3")) {
                            els.push(El::R(a, b, 20.0));
                        }
                    }
                }
                Kind::Battery9V => {
                    if !self.stim.removed.contains(&part.id) {
                        if let (Some(a), Some(b)) = (at("+"), at("-")) {
                            els.push(El::Src { p: a, n: b, v: 9.0, g: 1.0 / 1.5, owner: i });
                        }
                    }
                }
                Kind::UsbC | Kind::MicroUsb => {
                    if self.stim.usb.contains(&part.id) {
                        if let (Some(a), Some(b)) = (at("VBUS"), at("GND")) {
                            els.push(El::Src { p: a, n: b, v: 5.0, g: RAIL_G, owner: i });
                        }
                    }
                }
                Kind::Dht22 => {
                    if let (Some(a), Some(b)) = (at("VCC"), at("GND")) {
                        els.push(El::R(a, b, 1.0 / 3300.0));
                    }
                }
                Kind::HcSr04 => {
                    if let (Some(a), Some(b)) = (at("VCC"), at("GND")) {
                        els.push(El::R(a, b, 1.0 / 330.0));
                    }
                }
                Kind::Header(_) => {}
                Kind::Board(b) => {
                    let s = b.spec();
                    let Some(gnd) = at("GND") else { continue };
                    let rails: &[(&str, f64)] = match power[i] {
                        Power::Off => &[],
                        Power::Usb => s.usb,
                        Power::Input(k) => s.inputs[k].2,
                    };
                    for &(rail, v) in rails {
                        if let Some(n) = at(rail) {
                            els.push(El::Src { p: n, n: gnd, v, g: RAIL_G, owner: i });
                        }
                    }
                    if power[i] == Power::Off {
                        continue;
                    }
                    // the board's own consumption (about 45 mA) from the input feeding it
                    if let Power::Input(k) = power[i] {
                        if let Some(n) = at(s.inputs[k].0) {
                            els.push(El::R(n, gnd, 0.045 / s.inputs[k].1.max(3.3)));
                        }
                    }
                    for p in &self.wiring.programs {
                        if p.comp == part.id && p.mode == Mode::Pullup {
                            if let Some(n) = at(&p.pin) {
                                els.push(El::Src { p: n, n: gnd, v: s.logic, g: PULLUP_G, owner: i });
                            }
                        }
                    }
                    if let Some((name, pin)) = s.button {
                        if let Some(n) = at(pin) {
                            els.push(El::Src { p: n, n: gnd, v: s.logic, g: 1.0 / 4700.0, owner: i });
                            if self.stim.pressed.contains(&(part.id, name.to_string())) {
                                els.push(El::R(n, gnd, 1.0));
                            }
                        }
                    }
                }
            }
        }
        for ((i, pin), d) in drives {
            let (Some(n), Some(gnd)) = (self.net.at(*i, pin), self.net.at(*i, "GND")) else { continue };
            let Some(b) = self.board(*i) else { continue };
            let v = if *d == Drive::High { b.spec().logic } else { 0.0 };
            els.push(El::Src { p: n, n: gnd, v, g: GPIO_G, owner: *i });
        }
        els
    }

    fn reads(&self, power: &[Power], v: &[f64]) -> HashMap<(usize, String), bool> {
        let mut out = HashMap::new();
        for (i, _) in self.net.parts.iter().enumerate() {
            let Some(b) = self.board(i) else { continue };
            if power[i] == Power::Off {
                continue;
            }
            let Some(gnd) = self.net.at(i, "GND") else { continue };
            let s = b.spec();
            for ((pi, name), &n) in &self.net.node {
                if *pi == i && (b.is_gpio(name) || s.internal.contains(&name.as_str())) {
                    out.insert((i, name.clone()), v[n] - v[gnd] > 0.5 * s.logic);
                }
            }
        }
        out
    }
}

/// Simulates the parts with the given wiring and bench actions.
pub fn simulate(parts: &[Part], wiring: &Wiring, stim: &Stimulus) -> Solution {
    let net = Net::build(parts, wiring);
    let sim = Sim { net, wiring, stim };
    let n = sim.net.nodes.max(1);
    let count = parts.len();
    let mut power: Vec<Power> = (0..count).map(|i| if sim.board(i).is_some() && stim.usb.contains(&parts[i].id) { Power::Usb } else { Power::Off }).collect();
    let mut leds = vec![];
    let mut led_of = vec![];
    let mut read = HashMap::new();
    // boards fed through a supply pin: decide each with its own regulators switched off, so a
    // board cannot keep itself running
    for _ in 0..4 {
        let mut changed = false;
        for i in 0..count {
            let Some(b) = sim.board(i) else { continue };
            if power[i] == Power::Usb {
                continue;
            }
            let mut trial = power.clone();
            trial[i] = Power::Off;
            let drives = sim.drives(&trial, &read);
            let els = sim.elements(&trial, &drives, &mut leds, &mut led_of);
            let (v, _) = solve(n, &els, &leds);
            let gnd = sim.net.at(i, "GND").map_or(0.0, |g| v[g]);
            let s = b.spec();
            let new = s.inputs.iter().position(|(rail, min, _)| sim.net.at(i, rail).is_some_and(|r| v[r] - gnd >= *min - 1e-9)).map_or(Power::Off, Power::Input);
            if new != power[i] {
                power[i] = new;
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    // firmware: outputs that follow inputs settle in a few passes
    let mut v = vec![0.0; n];
    let mut on = vec![];
    let mut els = vec![];
    let mut drives = HashMap::new();
    for _ in 0..4 {
        drives = sim.drives(&power, &read);
        els = sim.elements(&power, &drives, &mut leds, &mut led_of);
        (v, on) = solve(n, &els, &leds);
        let new = sim.reads(&power, &v);
        if new == read {
            break;
        }
        read = new;
    }
    let mut sol = Solution::default();
    // circuits with a source: their nodes are defined, measured from the source's negative side
    let mut parent: Vec<usize> = (0..n).collect();
    for e in &els {
        let (a, b) = match *e {
            El::R(a, b, _) => (a, b),
            El::Src { p, n, .. } => (p, n),
            El::Led(a, k, i) => {
                if !on[i] {
                    continue;
                }
                (a, k)
            }
        };
        let (ra, rb) = (find(&mut parent, a), find(&mut parent, b));
        parent[ra] = rb;
    }
    let mut reference: HashMap<usize, usize> = HashMap::new();
    for e in &els {
        if let El::Src { n: m, .. } = *e {
            let r = find(&mut parent, m);
            reference.entry(r).or_insert(m);
        }
    }
    let level = |node: usize, parent: &mut Vec<usize>| -> PinLevel {
        match reference.get(&find(parent, node)) {
            Some(&r) => PinLevel::Volts(v[node] - v[r]),
            None => PinLevel::Floating,
        }
    };
    for ((i, name), &node) in &sim.net.node {
        sol.pins.insert(PinRef::new(parts[*i].id, name), level(node, &mut parent));
    }
    for (i, p) in power.iter().enumerate() {
        if *p != Power::Off {
            sol.running.insert(parts[i].id);
        }
    }
    // source currents (short circuits, GPIO overload)
    for e in &els {
        if let El::Src { p, n: m, v: vs, g, owner } = *e {
            let amps = (vs - (v[p] - v[m])) * g;
            let part = &parts[owner];
            let gpio = sim.board(owner).is_some_and(|b| g == GPIO_G && drives.keys().any(|(i, pin)| *i == owner && sim.net.at(owner, pin) == Some(p)) && b.spec().max_ma > 0.0);
            if gpio {
                let max = sim.board(owner).unwrap().spec().max_ma;
                if amps.abs() * 1000.0 > max {
                    sol.warnings.push(format!("{}: GPIO liefert {:.0} mA (empfohlen höchstens {:.0} mA).", part.label, amps.abs() * 1000.0, max));
                }
            } else if amps.abs() > 2.0 {
                sol.warnings.push(format!("{}: Kurzschluss, {:.1} A aus der Versorgung.", part.label, amps.abs()));
            }
        }
    }
    let gpio_at = |node: usize| -> bool {
        sim.net.node.iter().any(|((i, name), &nd)| nd == node && power[*i] != Power::Off && sim.board(*i).is_some_and(|b| b.is_gpio(name)))
    };
    for (i, part) in parts.iter().enumerate() {
        let lv = |pin: &str| sim.net.at(i, pin).map(|nd| v[nd]);
        let diff = |a: &str, b: &str| match (lv(a), lv(b)) {
            (Some(x), Some(y)) => x - y,
            _ => 0.0,
        };
        let id = part.id;
        let status = match part.device.kind {
            Kind::Board(b) => {
                let s = b.spec();
                let running = power[i] != Power::Off;
                if running {
                    if let Some(l) = s.power_lamp {
                        sol.lamps.insert((id, l.into()), 1.0);
                    }
                    if let Some((lamp, pin, high)) = s.led {
                        let x = diff(pin, "GND");
                        if (x > 0.6 * s.logic) == high {
                            sol.lamps.insert((id, lamp.into()), 1.0);
                        }
                    }
                }
                // 5 V on a 3.3 V GPIO
                if s.logic < 4.0 && b != BoardKind::Nucleo64 {
                    for name in part.device.pin_names() {
                        if b.is_gpio(&name) && diff(&name, "GND") > 3.6 && sim.net.at(i, &name).is_some_and(|nd| reference.contains_key(&find(&mut parent, nd))) {
                            sol.warnings.push(format!("{}: {:.1} V an {name} – der Eingang verträgt höchstens 3,6 V.", part.label, diff(&name, "GND")));
                        }
                    }
                }
                match power[i] {
                    Power::Off => "aus – keine Versorgung".to_string(),
                    Power::Usb => "läuft über USB".to_string(),
                    Power::Input(k) => format!("läuft über {} ({:.1} V)", s.inputs[k].0, diff(s.inputs[k].0, "GND")).replace('.', ","),
                }
            }
            Kind::Led(_) => {
                let k = led_of.iter().position(|&x| x == i);
                let ma = k.map_or(0.0, |k| if on[k] { (v[leds[k].0] - v[leds[k].1] - leds[k].2) * LED_G * 1000.0 } else { 0.0 }).max(0.0);
                sol.led_ma.insert(id, ma);
                if ma > 0.3 {
                    sol.lamps.insert((id, "LED".into()), (ma / 15.0).clamp(0.35, 1.0) as f32);
                }
                if ma > 30.0 {
                    sol.warnings.push(format!("{}: {:.0} mA – Vorwiderstand fehlt oder ist zu klein.", part.label, ma));
                }
                if ma > 0.3 {
                    format!("leuchtet · {:.1} mA", ma).replace('.', ",")
                } else if diff("K", "A") > 0.5 {
                    "aus · in Sperrrichtung".into()
                } else {
                    "aus".into()
                }
            }
            Kind::Resistor(r) => {
                let x = diff("1", "2");
                let ma = x / r.max(1) as f64 * 1000.0;
                let mw = x * x / r.max(1) as f64 * 1000.0;
                if mw > 250.0 {
                    sol.warnings.push(format!("{}: {:.0} mW – mehr als 0,25 W.", part.label, mw));
                }
                format!("{:.1} mA · {:.0} mW", ma.abs(), mw).replace('.', ",")
            }
            Kind::Tact6 | Kind::Tact12 => if stim.pressed.contains(&(id, String::new())) { "gedrückt".into() } else { "offen".into() },
            Kind::Battery9V => {
                if stim.removed.contains(&id) {
                    "entnommen".into()
                } else {
                    let x = diff("+", "-");
                    format!("{:.2} V · {:.0} mA", x, (9.0 - x) / 1.5 * 1000.0).replace('.', ",")
                }
            }
            Kind::UsbC | Kind::MicroUsb => if stim.usb.contains(&id) { "Kabel eingesteckt · 5 V".into() } else { "kein Kabel".into() },
            Kind::Dht22 => {
                let x = diff("VCC", "GND");
                if x > 6.0 {
                    sol.warnings.push(format!("{}: {:.1} V Versorgung, erlaubt 3,3–6 V.", part.label, x));
                    "Überspannung".into()
                } else if x < 3.0 {
                    "ohne Versorgung".into()
                } else if sim.net.at(i, "DATA").is_some_and(gpio_at) {
                    "misst 22,4 °C · 48 % rF".into()
                } else {
                    "versorgt · DATA an keinem GPIO".into()
                }
            }
            Kind::HcSr04 => {
                let x = diff("VCC", "GND");
                if x < 4.5 {
                    "ohne Versorgung (5 V nötig)".into()
                } else if sim.net.at(i, "TRIG").is_some_and(gpio_at) && sim.net.at(i, "ECHO").is_some_and(gpio_at) {
                    "misst Abstand · 23 cm".into()
                } else {
                    "versorgt · TRIG/ECHO an keinem GPIO".into()
                }
            }
            Kind::Header(_) => "—".into(),
        };
        sol.status.push((id, status));
    }
    sol.warnings.dedup();
    sol
}
