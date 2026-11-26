//! Weld seams on part edges: seam geometry, filler metal, welding time and cost, and a static
//! strength check by the simplified method of EN 1993-1-8 (4.5.3.3).

use crate::kernel::topology::{EdgeRef, Topology};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WeldKind {
    /// Kehlnaht, a = throat thickness
    Fillet,
    /// HV-Naht (half-V butt), a = plate thickness
    HalfV,
    /// V-Naht 60°, a = plate thickness
    V,
    /// I-Naht (square butt), a = plate thickness
    Square,
}

impl WeldKind {
    pub const ALL: [WeldKind; 4] = [WeldKind::Fillet, WeldKind::HalfV, WeldKind::V, WeldKind::Square];
    pub fn name(self) -> &'static str {
        match self {
            WeldKind::Fillet => "Kehlnaht",
            WeldKind::HalfV => "HV-Naht",
            WeldKind::V => "V-Naht",
            WeldKind::Square => "I-Naht",
        }
    }
    pub fn symbol(self) -> &'static str {
        match self {
            WeldKind::Fillet => "◺",
            WeldKind::HalfV => "V̲",
            WeldKind::V => "V",
            WeldKind::Square => "‖",
        }
    }
    /// Seam cross-section (mm²) for size `a`: weld metal including about 10 % reinforcement.
    pub fn area(self, a: f64) -> f64 {
        let gap = 1.5;
        let core = match self {
            WeldKind::Fillet => a * a,
            WeldKind::HalfV => a * a * 0.5 + gap * a,
            WeldKind::V => a * a * (30f64.to_radians().tan()) + gap * a,
            WeldKind::Square => gap * a + 0.4 * a,
        };
        core * 1.1
    }
    /// Effective throat for the strength check: the fillet's a, a full-penetration butt weld's t.
    pub fn throat(self, a: f64) -> f64 {
        a
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct WeldSeam {
    pub id: u64,
    pub name: String,
    pub kind: WeldKind,
    /// a-Maß (fillet) or plate thickness (butt welds), mm
    pub a: f64,
    pub both_sides: bool,
    /// Unterbrochene Naht: (Nahtlänge, Teilung) in mm
    #[serde(default)]
    pub intermittent: Option<(f64, f64)>,
    pub edges: Vec<EdgeRef>,
}

impl WeldSeam {
    /// Edge chains of the seam in the current geometry (missing ones left out).
    pub fn chains(&self, topo: &Topology) -> Vec<usize> {
        self.edges.iter().filter_map(|r| topo.match_chain(r)).collect()
    }
    /// Edge length the seam runs along.
    pub fn edge_length(&self, topo: &Topology) -> f64 {
        self.chains(topo).iter().map(|&c| topo.chains[c].length()).sum()
    }
    /// Length actually welded (intermittent seams only weld part of it).
    pub fn weld_length(&self, topo: &Topology) -> f64 {
        let l = self.edge_length(topo);
        match self.intermittent {
            Some((seg, pitch)) if pitch > seg && seg > 0.0 => {
                let n = ((l - seg) / pitch).floor().max(0.0) + 1.0;
                (n * seg).min(l)
            }
            _ => l,
        }
    }
    pub fn sides(&self) -> f64 {
        if self.both_sides {
            2.0
        } else {
            1.0
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Process {
    pub name: &'static str,
    /// Abschmelzleistung kg/h
    pub deposition: f64,
    /// Ausbringung (filler that ends up in the seam)
    pub efficiency: f64,
    /// Lichtbogenzeitanteil
    pub duty: f64,
    /// Zusatzwerkstoff €/kg
    pub filler_cost: f64,
}

pub const PROCESSES: [Process; 4] = [
    Process { name: "MAG (135)", deposition: 3.0, efficiency: 0.95, duty: 0.40, filler_cost: 3.5 },
    Process { name: "MIG Aluminium (131)", deposition: 2.0, efficiency: 0.93, duty: 0.35, filler_cost: 14.0 },
    Process { name: "WIG (141)", deposition: 0.8, efficiency: 0.98, duty: 0.30, filler_cost: 18.0 },
    Process { name: "E-Hand (111)", deposition: 1.3, efficiency: 0.60, duty: 0.25, filler_cost: 6.0 },
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WeldMaterial {
    pub name: &'static str,
    /// Zugfestigkeit f_u MPa
    pub fu: f64,
    /// Korrelationsbeiwert β_w
    pub beta: f64,
    /// kg/mm³
    pub rho: f64,
}

pub const MATERIALS: [WeldMaterial; 5] = [
    WeldMaterial { name: "S235", fu: 360.0, beta: 0.80, rho: 7.85e-6 },
    WeldMaterial { name: "S355", fu: 490.0, beta: 0.90, rho: 7.85e-6 },
    WeldMaterial { name: "S460", fu: 540.0, beta: 1.00, rho: 7.85e-6 },
    WeldMaterial { name: "Edelstahl 1.4301", fu: 500.0, beta: 1.00, rho: 7.9e-6 },
    WeldMaterial { name: "EN AW-5083 (WEZ)", fu: 270.0, beta: 1.00, rho: 2.66e-6 },
];

pub const GAMMA_M2: f64 = 1.25;

/// Design strength of a fillet weld per unit throat area (MPa), EN 1993-1-8 eq. 4.4.
pub fn fvw_d(m: &WeldMaterial) -> f64 {
    m.fu / (3f64.sqrt() * m.beta * GAMMA_M2)
}

#[derive(Clone, Debug, Default)]
pub struct SeamCalc {
    pub edge_length: f64,
    pub weld_length: f64,
    pub volume: f64,
    pub mass: f64,
    pub filler: f64,
    pub hours: f64,
    pub throat_area: f64,
    pub notes: Vec<String>,
}

pub fn calc(seam: &WeldSeam, topo: &Topology, m: &WeldMaterial, p: &Process, plate: f64) -> SeamCalc {
    let edge_length = seam.edge_length(topo);
    let weld_length = seam.weld_length(topo) * seam.sides();
    let volume = seam.kind.area(seam.a) * weld_length;
    let mass = volume * m.rho;
    let filler = mass / p.efficiency;
    let hours = filler / p.deposition / p.duty;
    let throat_area = seam.kind.throat(seam.a) * weld_length;
    let mut notes = vec![];
    if seam.chains(topo).len() < seam.edges.len() {
        notes.push(format!("{}: {} Kante(n) im aktuellen Modell nicht mehr gefunden.", seam.name, seam.edges.len() - seam.chains(topo).len()));
    }
    if seam.kind == WeldKind::Fillet && plate > 0.0 {
        // DIN EN 1993-1-8 / DIN 18800 rules of thumb for fillet welds
        if seam.a < 3.0 {
            notes.push(format!("{}: a = {} mm < 3 mm (Mindestmaß für tragende Kehlnähte).", seam.name, super::num(seam.a)));
        }
        if seam.a > 0.7 * plate {
            notes.push(format!("{}: a = {} mm > 0,7 · t = {} mm (Naht zu dick für das Blech).", seam.name, super::num(seam.a), super::num(0.7 * plate)));
        }
        if seam.a < plate.sqrt() - 0.5 {
            notes.push(format!("{}: a ≥ √t − 0,5 = {} mm empfohlen (Wärmeeinbringung).", seam.name, super::num(plate.sqrt() - 0.5)));
        }
    }
    if seam.kind == WeldKind::Fillet && weld_length / seam.sides() < 6.0 * seam.a.max(5.0) {
        notes.push(format!("{}: Nahtlänge unter 6·a bzw. 30 mm zählt nach EN 1993-1-8 nicht als tragend.", seam.name));
    }
    SeamCalc { edge_length, weld_length, volume, mass, filler, hours, throat_area, notes }
}

/// Utilisation of all seams sharing a load `force` (N): (τ_w MPa, f_vw,d MPa, η).
pub fn strength(throat_area: f64, force: f64, m: &WeldMaterial) -> (f64, f64, f64) {
    let tau = if throat_area > 0.0 { force / throat_area } else { f64::INFINITY };
    let f = fvw_d(m);
    (tau, f, tau / f)
}

/// Bead symbol for the graphics window: line pieces (pairs of points) drawn across the edge,
/// spaced along it, leaning into both faces.
pub fn bead_lines(seam: &WeldSeam, topo: &Topology) -> Vec<(glam::DVec3, glam::DVec3)> {
    let mut out = vec![];
    let size = seam.a.max(1.0) * 1.2;
    for c in seam.chains(topo) {
        let ch = &topo.chains[c];
        let pts = &ch.points;
        let total = ch.length();
        if total <= 0.0 || pts.len() < 2 {
            continue;
        }
        let step = (size * 0.9).max(total / 400.0);
        let mut s = 0.0;
        let mut acc = 0.0;
        let segs: Vec<usize> = if ch.closed { (0..pts.len()).collect() } else { (0..pts.len() - 1).collect() };
        for i in segs {
            let (a, b) = (pts[i], pts[(i + 1) % pts.len()]);
            let l = (b - a).length();
            if l <= 0.0 {
                continue;
            }
            let ia = i.min(ch.n_a.len().saturating_sub(1));
            let (na, nb) = (ch.n_a.get(ia).copied().unwrap_or(glam::DVec3::Y), ch.n_b.get(ia).copied().unwrap_or(glam::DVec3::Y));
            let dir = (b - a) / l;
            while s <= acc + l {
                let on = match seam.intermittent {
                    Some((seg, pitch)) if pitch > seg => (s % pitch) < seg,
                    _ => true,
                };
                if on {
                    let p = a + dir * (s - acc);
                    // along each face away from the edge: the other face's normal, projected into
                    // it (pointing away on an outside edge, towards it in an inside corner)
                    let sg = if ch.convex { -1.0 } else { 1.0 };
                    let ta = crate::kernel::math::safe_norm((nb - na * nb.dot(na)) * sg, na);
                    let tb = crate::kernel::math::safe_norm((na - nb * na.dot(nb)) * sg, nb);
                    let lift = (na + nb).normalize_or_zero() * (size * 0.25);
                    out.push((p + ta * size, p + lift + dir * (size * 0.3)));
                    out.push((p + lift + dir * (size * 0.3), p + tb * size));
                }
                s += step;
            }
            acc += l;
        }
    }
    out
}
