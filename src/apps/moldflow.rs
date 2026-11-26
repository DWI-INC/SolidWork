//! Filling simulation (Spritzgussanalyse) on the voxel grid of the part: local wall thickness
//! from a distance transform, the melt's flow resistance from Hele-Shaw flow (∝ 1/h²), the
//! fill order by the least-resistance paths from the gate at constant volume flow, the
//! pressure at the end of filling, cooling time per wall thickness, weld lines where flow
//! fronts meet head-on and air traps where the melt arrives last.

use super::mold::{cooling_time, MoldMaterial};
use crate::kernel::math::*;
use crate::kernel::topology::Topology;
use crate::model::sim::StudyKind;
use crate::sim::mesh::{Grid, Mesh};
use crate::sim::run::{build_surface, fmt, Field, Shared, SimOutput};
use std::cmp::Ordering as CmpOrd;
use std::collections::BinaryHeap;
use std::sync::atomic::Ordering;
use std::sync::Arc;

#[derive(Clone)]
pub struct FillInput {
    pub topo: Arc<Topology>,
    pub material: MoldMaterial,
    /// gate position on the part (None: chosen automatically)
    pub gate: Option<V3>,
    /// Einspritzzeit s
    pub fill_time: f64,
    /// solid cells of the grid
    pub cells: usize,
    pub cavities: usize,
}

#[derive(Clone, Copy, PartialEq)]
struct Item(f64, u32);
impl Eq for Item {}
impl PartialOrd for Item {
    fn partial_cmp(&self, o: &Self) -> Option<CmpOrd> {
        Some(self.cmp(o))
    }
}
impl Ord for Item {
    fn cmp(&self, o: &Self) -> CmpOrd {
        o.0.partial_cmp(&self.0).unwrap_or(CmpOrd::Equal)
    }
}

const NB6: [[i32; 3]; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];

fn offsets26() -> Vec<[i32; 3]> {
    let mut v = vec![];
    for k in -1..=1 {
        for j in -1..=1 {
            for i in -1..=1 {
                if (i, j, k) != (0, 0, 0) {
                    v.push([i, j, k]);
                }
            }
        }
    }
    v
}

struct G<'a> {
    g: &'a Grid,
}
impl G<'_> {
    fn ijk(&self, c: usize) -> [i32; 3] {
        let n = self.g.n;
        [(c % n[0]) as i32, ((c / n[0]) % n[1]) as i32, (c / (n[0] * n[1])) as i32]
    }
    fn at(&self, p: [i32; 3]) -> Option<usize> {
        let n = self.g.n;
        if p[0] < 0 || p[1] < 0 || p[2] < 0 || p[0] >= n[0] as i32 || p[1] >= n[1] as i32 || p[2] >= n[2] as i32 {
            return None;
        }
        Some(self.g.cell(p[0] as usize, p[1] as usize, p[2] as usize))
    }
    fn solid(&self, p: [i32; 3]) -> bool {
        self.at(p).is_some_and(|c| self.g.solid[c])
    }
    fn center(&self, c: usize) -> V3 {
        let q = self.ijk(c);
        self.g.center(q[0] as usize, q[1] as usize, q[2] as usize)
    }
}

/// Distance of every solid cell centre to the outside (mm), by Dijkstra over 26 neighbours.
fn distance_to_wall(gg: &G) -> Vec<f64> {
    let g = gg.g;
    let h = g.h;
    let mut d = vec![f64::INFINITY; g.cells()];
    let mut heap = BinaryHeap::new();
    let offs = offsets26();
    for c in 0..g.cells() {
        if !g.solid[c] {
            continue;
        }
        let q = gg.ijk(c);
        // a solid cell next to empty space (or the grid border): half a cell from the wall
        for o in NB6 {
            if !gg.solid([q[0] + o[0], q[1] + o[1], q[2] + o[2]]) {
                let w = 0.5 * (o[0].abs() as f64 * h.x + o[1].abs() as f64 * h.y + o[2].abs() as f64 * h.z);
                if w < d[c] {
                    d[c] = w;
                }
            }
        }
        if d[c].is_finite() {
            heap.push(Item(d[c], c as u32));
        }
    }
    while let Some(Item(dist, c)) = heap.pop() {
        let c = c as usize;
        if dist > d[c] {
            continue;
        }
        let q = gg.ijk(c);
        for o in &offs {
            let Some(n) = gg.at([q[0] + o[0], q[1] + o[1], q[2] + o[2]]) else { continue };
            if !g.solid[n] {
                continue;
            }
            let w = V3::new(o[0] as f64 * h.x, o[1] as f64 * h.y, o[2] as f64 * h.z).length();
            if dist + w < d[n] {
                d[n] = dist + w;
                heap.push(Item(d[n], n as u32));
            }
        }
    }
    d
}

/// Wall thickness at every cell: twice the distance of the medial cells, carried down to the
/// cells near the surface (each cell takes the largest value of a deeper neighbour).
fn thickness(gg: &G, dist: &[f64]) -> Vec<f64> {
    let g = gg.g;
    let mut order: Vec<usize> = (0..g.cells()).filter(|&c| g.solid[c]).collect();
    order.sort_by(|a, b| dist[*b].partial_cmp(&dist[*a]).unwrap_or(CmpOrd::Equal));
    let mut t = vec![0.0; g.cells()];
    let offs = offsets26();
    for &c in &order {
        let mut v = 2.0 * dist[c];
        let q = gg.ijk(c);
        for o in &offs {
            if let Some(n) = gg.at([q[0] + o[0], q[1] + o[1], q[2] + o[2]]) {
                // only from strictly deeper cells: along a wall's ridge (equal depth) nothing
                // spreads, so a thick region does not leak into the thin walls next to it
                if g.solid[n] && dist[n] > dist[c] + 1e-9 {
                    v = v.max(t[n]);
                }
            }
        }
        t[c] = v;
    }
    t
}

/// Thinnest and thickest wall of the part (mm): twice the distance from the wall at the
/// medial cells (the centres of inscribed spheres), on a voxel grid of about `cells` cells.
pub fn wall_range(topo: &Topology, cells: usize) -> Option<(f64, f64)> {
    let mesh = Mesh::build(topo, cells).ok()?;
    let g = &mesh.grid;
    let gg = G { g };
    let dist = distance_to_wall(&gg);
    let (mut lo, mut hi) = (f64::INFINITY, 0.0f64);
    for c in 0..g.cells() {
        if !g.solid[c] {
            continue;
        }
        let q = gg.ijk(c);
        let medial = NB6.iter().all(|o| gg.at([q[0] + o[0], q[1] + o[1], q[2] + o[2]]).is_none_or(|n| !g.solid[n] || dist[n] <= dist[c] + 1e-9));
        if medial {
            lo = lo.min(2.0 * dist[c]);
            hi = hi.max(2.0 * dist[c]);
        }
    }
    (hi > 0.0).then_some((lo, hi))
}

pub struct FillResult {
    pub out: SimOutput,
    /// fill time at every display vertex (s), for the animation
    pub fill_at_vertex: Vec<f32>,
    pub gate: V3,
    pub fill_time: f64,
    pub pressure_mpa: f64,
}

pub fn run(inp: &FillInput, sh: &Shared) -> Result<FillResult, String> {
    let set = |f: f32, s: &str| {
        sh.progress.store(f.to_bits(), Ordering::Relaxed);
        *sh.stage.lock().unwrap() = s.into();
    };
    let cancelled = || sh.cancel.load(Ordering::Relaxed);
    let t0 = std::time::Instant::now();
    let topo = &*inp.topo;
    let m = inp.material;
    set(0.02, "Vernetzen");
    let mesh = Mesh::build(topo, inp.cells.max(2000))?;
    let g = &mesh.grid;
    let gg = G { g };
    let h = g.h;
    let hmin = h.min_element();
    let vol_cell = h.x * h.y * h.z;
    set(0.15, "Wanddicken bestimmen");
    let dist = distance_to_wall(&gg);
    let thick = thickness(&gg, &dist);
    if cancelled() {
        return Err("abgebrochen".into());
    }
    // ---- gate
    let solid: Vec<usize> = (0..g.cells()).filter(|&c| g.solid[c]).collect();
    let surface_cell = |c: usize| {
        let q = gg.ijk(c);
        NB6.iter().any(|o| !gg.solid([q[0] + o[0], q[1] + o[1], q[2] + o[2]]))
    };
    let tmax = solid.iter().map(|&c| thick[c]).fold(0.0, f64::max).max(1e-9);
    let gate_cell = match inp.gate {
        Some(p) => solid.iter().copied().min_by(|a, b| (gg.center(*a) - p).length_squared().partial_cmp(&(gg.center(*b) - p).length_squared()).unwrap()).unwrap(),
        None => {
            // on the surface, in thick material, central: short flow paths, packing from the thick end
            let cen = solid.iter().map(|&c| gg.center(c)).sum::<V3>() / solid.len() as f64;
            let diag = topo.bbox.diagonal().max(1e-9);
            solid
                .iter()
                .copied()
                .filter(|&c| surface_cell(c))
                .max_by(|a, b| {
                    let s = |c: usize| thick[c] / tmax - 1.2 * (gg.center(c) - cen).length() / diag;
                    s(*a).partial_cmp(&s(*b)).unwrap()
                })
                .ok_or("Keine Oberfläche gefunden.")?
        }
    };
    // ---- fill order: least flow resistance ∫ ds / h²
    set(0.3, "Füllung berechnen");
    let offs = offsets26();
    let mut res = vec![f64::INFINITY; g.cells()];
    let mut parent = vec![u32::MAX; g.cells()];
    let mut order = Vec::with_capacity(solid.len());
    let mut heap = BinaryHeap::new();
    res[gate_cell] = 0.0;
    heap.push(Item(0.0, gate_cell as u32));
    let floor_t = hmin * 0.75;
    while let Some(Item(r, c)) = heap.pop() {
        let c = c as usize;
        if r > res[c] {
            continue;
        }
        order.push(c);
        let q = gg.ijk(c);
        for o in &offs {
            let Some(n) = gg.at([q[0] + o[0], q[1] + o[1], q[2] + o[2]]) else { continue };
            if !g.solid[n] {
                continue;
            }
            let l = V3::new(o[0] as f64 * h.x, o[1] as f64 * h.y, o[2] as f64 * h.z).length();
            let tt = (0.5 * (thick[c] + thick[n])).max(floor_t);
            let nr = r + l / (tt * tt);
            if nr < res[n] {
                res[n] = nr;
                parent[n] = c as u32;
                heap.push(Item(nr, n as u32));
            }
        }
    }
    if cancelled() {
        return Err("abgebrochen".into());
    }
    let filled = order.len();
    // constant volume flow: a cell's fill time is the volume filled before it
    let mut tfill = vec![f32::NAN; g.cells()];
    for (i, &c) in order.iter().enumerate() {
        tfill[c] = (inp.fill_time * (i + 1) as f64 / filled as f64) as f32;
    }
    // ---- pressure at the end of filling (Hele-Shaw): Δp = 12 η v ∫ ds/h²
    let last = *order.last().unwrap();
    let mut flow_len = 0.0;
    {
        let mut c = last;
        while parent[c] != u32::MAX {
            let p = parent[c] as usize;
            flow_len += (gg.center(c) - gg.center(p)).length();
            c = p;
        }
    }
    let v_front = flow_len / inp.fill_time.max(1e-3); // mm/s
    let rmax = res[last];
    let p_gate = 12.0 * m.viscosity * v_front * rmax / 1e6; // MPa
    let pressure: Vec<f64> = (0..g.cells()).map(|c| if g.solid[c] && res[c].is_finite() { p_gate * (1.0 - res[c] / rmax.max(1e-30)) } else { f64::NAN }).collect();
    set(0.6, "Bindenähte und Lufteinschlüsse suchen");
    // ---- flow direction per cell: from an ancestor a few steps back
    let back = |mut c: usize, k: usize| {
        for _ in 0..k {
            if parent[c] == u32::MAX {
                break;
            }
            c = parent[c] as usize;
        }
        c
    };
    let mut weld = vec![false; g.cells()];
    let mut trap = vec![false; g.cells()];
    let steps = 4;
    for &c in &order {
        let q = gg.ijk(c);
        let pc = gg.center(c);
        let dc = safe_norm(pc - gg.center(back(c, steps)), V3::ZERO);
        let mut later = 0;
        let mut nsolid = 0;
        for o in &NB6 {
            let Some(n) = gg.at([q[0] + o[0], q[1] + o[1], q[2] + o[2]]) else { continue };
            if !g.solid[n] {
                continue;
            }
            nsolid += 1;
            if tfill[n] > tfill[c] {
                later += 1;
            }
            if parent[n] as usize == c || parent[c] as usize == n || dc == V3::ZERO {
                continue;
            }
            let dn = safe_norm(gg.center(n) - gg.center(back(n, steps)), V3::ZERO);
            // fronts meeting head-on
            if dn != V3::ZERO && dc.dot(dn) < -0.5 && (res[c] - res[n]).abs() < rmax * 0.05 {
                weld[c] = true;
            }
        }
        // the melt arrives here after all its neighbours: air is pushed into this corner
        if later == 0 && nsolid > 0 && surface_cell(c) && tfill[c] as f64 > inp.fill_time * 0.2 {
            trap[c] = true;
        }
    }
    // ---- surface sampling
    set(0.75, "Ergebnisse aufbereiten");
    let (surface, face_n) = build_surface(topo, hmin);
    let pick = |p: [f32; 3], n: V3| -> Option<usize> {
        let p = V3::new(p[0] as f64, p[1] as f64, p[2] as f64);
        let probe = p - n * (hmin * 0.5);
        let [i, j, k] = g.cell_at(probe).or_else(|| g.cell_at(p))?;
        let c0 = g.cell(i, j, k);
        if g.solid[c0] {
            return Some(c0);
        }
        let q = [i as i32, j as i32, k as i32];
        let mut best: Option<(f64, usize)> = None;
        for o in offsets26() {
            if let Some(c) = gg.at([q[0] + o[0], q[1] + o[1], q[2] + o[2]]) {
                if g.solid[c] {
                    let d = (gg.center(c) - p).length_squared();
                    if best.map_or(true, |b| d < b.0) {
                        best = Some((d, c));
                    }
                }
            }
        }
        best.map(|b| b.1)
    };
    let vcell: Vec<Option<usize>> = surface.pos.iter().zip(&face_n).map(|(p, n)| pick(*p, *n)).collect();
    let sample = |v: &dyn Fn(usize) -> f64| -> Vec<f32> { vcell.iter().map(|c| c.map_or(f32::NAN, |c| v(c) as f32)).collect() };
    let fill_v = sample(&|c| tfill[c] as f64);
    let press_v = sample(&|c| pressure[c]);
    let thick_v = sample(&|c| thick[c]);
    let cool_v = sample(&|c| cooling_time(&m, thick[c]));
    let quality_v = sample(&|c| {
        // Füllqualität: 1 good … 0 risky (thin far from the gate, weld lines, traps)
        let mut q = 1.0;
        let tc = thick[c];
        if tc < m.wall.0 {
            q -= 0.35;
        }
        if tc > m.wall.1 {
            q -= 0.25;
        }
        if weld[c] {
            q -= 0.4;
        }
        if trap[c] {
            q -= 0.5;
        }
        q -= 0.3 * (pressure[c].is_finite() as i32 as f64) * (1.0 - pressure[c] / p_gate.max(1e-9)) * (p_gate / m.max_inj_mpa).min(1.0);
        q.clamp(0.0, 1.0)
    });
    let mut fields = vec![
        Field::new("Füllzeit", "s", fill_v.clone(), None),
        Field::new("Druck am Füllende", "MPa", press_v, None),
        Field::new("Wanddicke", "mm", thick_v, None),
        Field::new("Abkühlzeit", "s", cool_v, None),
    ];
    let mut q = Field::new("Füllqualität", "1 = gut", quality_v, None);
    q.low_is_bad = true;
    fields.push(q);
    // weld lines and air traps as marks (drawn in the colour of the fill-time maximum)
    let vmark = fields[0].max;
    let mut lines: Vec<Vec<(V3, f32)>> = vec![];
    let mark = hmin * 0.9;
    let (mut n_weld, mut n_trap) = (0, 0);
    for &c in &order {
        if !surface_cell(c) {
            continue;
        }
        let p = gg.center(c);
        if weld[c] {
            n_weld += 1;
            lines.push(vec![(p - V3::X * mark, vmark), (p + V3::X * mark, vmark)]);
            lines.push(vec![(p - V3::Z * mark, vmark), (p + V3::Z * mark, vmark)]);
        }
        if trap[c] {
            n_trap += 1;
            let r = mark * 2.0;
            for (a, b) in [(V3::X, V3::Y), (V3::Y, V3::Z), (V3::Z, V3::X)] {
                let ring: Vec<(V3, f32)> = (0..=12)
                    .map(|i| {
                        let t = i as f64 / 12.0 * std::f64::consts::TAU;
                        (p + (a * t.cos() + b * t.sin()) * r, vmark)
                    })
                    .collect();
                lines.push(ring);
            }
        }
    }
    // the gate as a cone of short lines
    let gp = gg.center(gate_cell);
    let gate_len = topo.bbox.diagonal() * 0.08;
    for k in 0..8 {
        let a = k as f64 / 8.0 * std::f64::consts::TAU;
        let off = V3::new(a.cos(), 0.0, a.sin()) * (gate_len * 0.25);
        lines.push(vec![(gp, 0.0), (gp + V3::Y * gate_len + off, 0.0)]);
    }
    // ---- summary
    let vol = solid.len() as f64 * vol_cell;
    let shot_g = vol * 1e-9 * m.rho * 1000.0;
    let tw: Vec<f64> = solid.iter().map(|&c| thick[c]).collect();
    let tmax_wall = tw.iter().cloned().fold(0.0, f64::max);
    let mean_wall = {
        // the wall: medial cells' thickness, weighted by their volume
        let med: Vec<f64> = solid.iter().filter(|&&c| !surface_cell(c)).map(|&c| thick[c]).collect();
        if med.is_empty() {
            tmax_wall
        } else {
            med.iter().sum::<f64>() / med.len() as f64
        }
    };
    let t_cool = cooling_time(&m, mean_wall);
    let t_cool_max = cooling_time(&m, tmax_wall);
    let ratio = flow_len / mean_wall.max(1e-6);
    let mut summary = vec![
        ("Werkstoff".to_string(), m.name.to_string()),
        ("Anschnitt".to_string(), format!("({}, {}, {}) mm{}", fmt(gp.x), fmt(gp.y), fmt(gp.z), if inp.gate.is_none() { " – automatisch gewählt" } else { "" })),
        ("Füllzeit".to_string(), format!("{} s", fmt(inp.fill_time))),
        ("Schussvolumen".to_string(), format!("{} cm³ ({} g je Kavität)", fmt(vol / 1000.0), fmt(shot_g))),
        ("Max. Einspritzdruck (Schätzung)".to_string(), format!("{} MPa ({} bar)", fmt(p_gate), fmt(p_gate * 10.0))),
        ("Max. Fließweg".to_string(), format!("{} mm", fmt(flow_len))),
        ("Fließweg / Wanddicke".to_string(), format!("{} : 1 (Grenze {} : 1)", fmt(ratio), fmt(m.flow_ratio))),
        ("Mittlere / max. Wanddicke".to_string(), format!("{} / {} mm", fmt(mean_wall), fmt(tmax_wall))),
        ("Abkühlzeit (Wand / dickste Stelle)".to_string(), format!("{} / {} s", fmt(t_cool), fmt(t_cool_max))),
        ("Zykluszeit (Schätzung)".to_string(), format!("{} s", fmt(inp.fill_time + 0.3 * t_cool + t_cool + 4.0))),
        ("Bindenähte".to_string(), if n_weld == 0 { "keine erkannt".into() } else { format!("{} Oberflächenzellen (rote Kreuze)", n_weld) }),
        ("Lufteinschlüsse".to_string(), if n_trap == 0 { "keine erkannt".into() } else { format!("{} Stelle(n) (rote Ringe) – Entlüftung vorsehen", n_trap) }),
        ("Netz".to_string(), format!("{} Zellen ({} × {} × {} mm)", solid.len(), fmt(h.x), fmt(h.y), fmt(h.z))),
    ];
    let _ = inp.cavities;
    let mut warnings = vec![];
    if p_gate > m.max_inj_mpa {
        warnings.push(format!(
            "Der nötige Einspritzdruck ({} MPa) übersteigt den üblichen Maschinendruck ({} MPa): Füllung wahrscheinlich unvollständig. Wand dicker, Anschnitt näher oder mehrere Anschnitte.",
            fmt(p_gate),
            fmt(m.max_inj_mpa)
        ));
    }
    if ratio > m.flow_ratio {
        warnings.push(format!("Fließweg-Wanddicken-Verhältnis {} : 1 über der Grenze von {} für {}.", fmt(ratio), fmt(m.flow_ratio), m.name));
    }
    if mean_wall < m.wall.0 {
        warnings.push(format!("Mittlere Wanddicke {} mm unter dem Bereich für {} ({}–{} mm).", fmt(mean_wall), m.name, fmt(m.wall.0), fmt(m.wall.1)));
    }
    if tmax_wall > m.wall.1 * 1.5 {
        warnings.push(format!("Masseanhäufung: {} mm Wanddicke – Einfallstellen und lange Kühlzeit. Wand ausräumen (Rippen statt Vollmaterial).", fmt(tmax_wall)));
    }
    if mesh.dropped > 0.01 {
        warnings.push(format!("{:.0} % des Volumens hängen nicht am Hauptkörper und wurden nicht gefüllt.", mesh.dropped * 100.0));
    }
    if filled < solid.len() {
        summary.push(("Nicht erreichbar".into(), format!("{} Zellen", solid.len() - filled)));
    }
    let table = (
        vec!["x_mm".into(), "y_mm".into(), "z_mm".into(), "fuellzeit_s".into(), "druck_MPa".into(), "wanddicke_mm".into()],
        order
            .iter()
            .map(|&c| {
                let p = gg.center(c);
                vec![p.x as f32, p.y as f32, p.z as f32, tfill[c], pressure[c] as f32, thick[c] as f32]
            })
            .collect(),
    );
    set(1.0, "fertig");
    let out = SimOutput {
        kind: StudyKind::Fluid,
        surface: Arc::new(surface),
        fields,
        disps: vec![],
        lines,
        summary,
        warnings,
        table,
        partial: false,
        seconds: t0.elapsed().as_secs_f64(),
        see_through: false,
    };
    Ok(FillResult { out, fill_at_vertex: fill_v, gate: gp, fill_time: inp.fill_time, pressure_mpa: p_gate })
}

/// The fill-time picture at time `t`: what is not filled yet is grey.
pub fn progress_output(base: &SimOutput, fill: &[f32], t: f32) -> SimOutput {
    let vals: Vec<f32> = fill.iter().map(|v| if v.is_finite() && *v <= t { *v } else { f32::NAN }).collect();
    let mut f = Field::new(&format!("Füllfortschritt bei {} s", fmt(t as f64)), "s", vals, None);
    if let Some(full) = base.fields.first() {
        f.min = full.min;
        f.max = full.max;
    }
    let mut fields = vec![f];
    fields.extend(base.fields.iter().map(|x| {
        let mut y = Field::new(&x.name, &x.unit, x.vals.clone(), x.disp);
        y.low_is_bad = x.low_is_bad;
        y
    }));
    SimOutput {
        kind: base.kind,
        surface: base.surface.clone(),
        fields,
        disps: vec![],
        lines: base.lines.clone(),
        summary: base.summary.clone(),
        warnings: vec![],
        table: (vec![], vec![]),
        partial: false,
        seconds: base.seconds,
        see_through: false,
    }
}
