//! Mould and casting design checks (Spritzguss/Guss) and the standard mould base (Expert
//! Moldbase): draft per face against the pull direction with undercut detection, projected
//! area and clamping force, parting plane, shrinkage allowance, cooling and cycle time, and
//! the plate stack of a two-plate mould picked from standard sizes.

use crate::kernel::math::*;
use crate::kernel::topology::Topology;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Process {
    Injection,
    DieCast,
    Sand,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MoldMaterial {
    pub name: &'static str,
    pub process: Process,
    /// Schwindung %
    pub shrink: f64,
    /// Massetemperatur, Werkzeugtemperatur, Entformungstemperatur °C
    pub t_melt: f64,
    pub t_mold: f64,
    pub t_eject: f64,
    /// Temperaturleitfähigkeit mm²/s
    pub diffusivity: f64,
    /// mittlerer Werkzeuginnendruck MPa
    pub cavity_mpa: f64,
    /// effective melt viscosity at processing shear rates, Pa·s
    pub viscosity: f64,
    /// kg/m³
    pub rho: f64,
    /// sinnvolle Wanddicken mm
    pub wall: (f64, f64),
    /// largest flow length / wall thickness ratio the melt fills
    pub flow_ratio: f64,
    /// empfohlene Mindest-Entformungsschräge °
    pub draft: f64,
    /// max. Einspritzdruck der Maschine, MPa
    pub max_inj_mpa: f64,
}

pub const MATERIALS: [MoldMaterial; 9] = [
    MoldMaterial {
        name: "ABS",
        process: Process::Injection,
        shrink: 0.55,
        t_melt: 240.0,
        t_mold: 60.0,
        t_eject: 90.0,
        diffusivity: 0.11,
        cavity_mpa: 35.0,
        viscosity: 280.0,
        rho: 1050.0,
        wall: (1.2, 3.5),
        flow_ratio: 180.0,
        draft: 1.0,
        max_inj_mpa: 150.0,
    },
    MoldMaterial {
        name: "PP",
        process: Process::Injection,
        shrink: 1.6,
        t_melt: 230.0,
        t_mold: 40.0,
        t_eject: 95.0,
        diffusivity: 0.09,
        cavity_mpa: 28.0,
        viscosity: 160.0,
        rho: 905.0,
        wall: (0.8, 3.8),
        flow_ratio: 250.0,
        draft: 1.0,
        max_inj_mpa: 150.0,
    },
    MoldMaterial {
        name: "PA6 GF30",
        process: Process::Injection,
        shrink: 0.45,
        t_melt: 280.0,
        t_mold: 85.0,
        t_eject: 170.0,
        diffusivity: 0.13,
        cavity_mpa: 50.0,
        viscosity: 220.0,
        rho: 1360.0,
        wall: (0.8, 3.0),
        flow_ratio: 150.0,
        draft: 1.0,
        max_inj_mpa: 180.0,
    },
    MoldMaterial {
        name: "PC",
        process: Process::Injection,
        shrink: 0.65,
        t_melt: 300.0,
        t_mold: 90.0,
        t_eject: 130.0,
        diffusivity: 0.12,
        cavity_mpa: 50.0,
        viscosity: 480.0,
        rho: 1200.0,
        wall: (1.0, 4.0),
        flow_ratio: 120.0,
        draft: 1.0,
        max_inj_mpa: 180.0,
    },
    MoldMaterial {
        name: "POM",
        process: Process::Injection,
        shrink: 2.0,
        t_melt: 210.0,
        t_mold: 90.0,
        t_eject: 150.0,
        diffusivity: 0.09,
        cavity_mpa: 45.0,
        viscosity: 240.0,
        rho: 1410.0,
        wall: (0.8, 3.0),
        flow_ratio: 150.0,
        draft: 0.5,
        max_inj_mpa: 160.0,
    },
    MoldMaterial {
        name: "PE-HD",
        process: Process::Injection,
        shrink: 2.2,
        t_melt: 230.0,
        t_mold: 30.0,
        t_eject: 100.0,
        diffusivity: 0.15,
        cavity_mpa: 25.0,
        viscosity: 180.0,
        rho: 955.0,
        wall: (0.9, 4.0),
        flow_ratio: 250.0,
        draft: 1.0,
        max_inj_mpa: 140.0,
    },
    MoldMaterial {
        name: "AlSi9Cu3 (Druckguss)",
        process: Process::DieCast,
        shrink: 0.6,
        t_melt: 680.0,
        t_mold: 220.0,
        t_eject: 450.0,
        diffusivity: 35.0,
        cavity_mpa: 70.0,
        viscosity: 0.0015,
        rho: 2750.0,
        wall: (1.0, 6.0),
        flow_ratio: 400.0,
        draft: 1.0,
        max_inj_mpa: 120.0,
    },
    MoldMaterial {
        name: "ZnAl4Cu1 (Zink-Druckguss)",
        process: Process::DieCast,
        shrink: 0.5,
        t_melt: 420.0,
        t_mold: 180.0,
        t_eject: 300.0,
        diffusivity: 38.0,
        cavity_mpa: 30.0,
        viscosity: 0.003,
        rho: 6700.0,
        wall: (0.6, 4.0),
        flow_ratio: 400.0,
        draft: 0.5,
        max_inj_mpa: 80.0,
    },
    MoldMaterial {
        name: "EN-GJL-250 (Sandguss)",
        process: Process::Sand,
        shrink: 1.0,
        t_melt: 1380.0,
        t_mold: 20.0,
        t_eject: 600.0,
        diffusivity: 6.0,
        cavity_mpa: 0.08,
        viscosity: 0.006,
        rho: 7200.0,
        wall: (3.0, 60.0),
        flow_ratio: 1000.0,
        draft: 2.0,
        max_inj_mpa: 1.0,
    },
];

/// Cooling time to the ejection temperature for a plate of thickness `s` mm (s).
pub fn cooling_time(m: &MoldMaterial, s: f64) -> f64 {
    let r = 4.0 / std::f64::consts::PI * (m.t_melt - m.t_mold) / (m.t_eject - m.t_mold).max(1.0);
    (s * s / (std::f64::consts::PI.powi(2) * m.diffusivity) * r.max(1.0001).ln()).max(0.0)
}

/// Two unit vectors completing `d` to a right-handed frame.
pub fn frame(d: V3) -> (V3, V3) {
    let e1 = safe_norm(d.cross(if d.y.abs() < 0.9 { V3::Y } else { V3::X }), V3::X);
    let e2 = d.cross(e1);
    (e1, e2)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DraftClass {
    /// draft enough, released by the half moving along +d (Düsenseite)
    Cavity,
    /// draft enough, released by the half moving along −d (Auswerferseite)
    Core,
    /// some draft but less than required
    Low,
    /// parallel to the pull direction
    None,
    /// blocked by the part when the half opens
    Undercut,
}

impl DraftClass {
    pub const ALL: [DraftClass; 5] = [DraftClass::Cavity, DraftClass::Core, DraftClass::Low, DraftClass::None, DraftClass::Undercut];
    pub fn label(self) -> &'static str {
        match self {
            DraftClass::Cavity => "Schräge ausreichend – Düsenseite",
            DraftClass::Core => "Schräge ausreichend – Auswerferseite",
            DraftClass::Low => "Schräge zu gering",
            DraftClass::None => "Ohne Schräge (0°)",
            DraftClass::Undercut => "Hinterschnitt",
        }
    }
    pub fn color(self) -> [f32; 4] {
        match self {
            DraftClass::Cavity => [0.30, 0.72, 0.36, 1.0],
            DraftClass::Core => [0.25, 0.52, 0.88, 1.0],
            DraftClass::Low => [0.98, 0.80, 0.18, 1.0],
            DraftClass::None => [0.98, 0.52, 0.12, 1.0],
            DraftClass::Undercut => [0.88, 0.16, 0.18, 1.0],
        }
    }
}

pub struct MoldCheck {
    /// class of every polygon of the topology
    pub poly_class: Vec<DraftClass>,
    /// area per class (mm²), in `DraftClass::ALL` order
    pub area: [f64; 5],
    /// faces with too little draft or undercut
    pub bad_faces: Vec<usize>,
    pub undercut_faces: Vec<usize>,
    /// projected area normal to the pull direction (mm²)
    pub projected: f64,
    /// extent along e1, e2 and d
    pub extent: V3,
    /// level of the parting plane along d, and the part's range along d
    pub parting: f64,
    pub range: (f64, f64),
    /// depth of the part on the cavity (+d) and core (−d) side of the parting plane
    pub depth_cavity: f64,
    pub depth_core: f64,
}

/// Classifies every polygon by its draft against pull direction `d` (unit), `min_draft` in degrees.
pub fn check(topo: &Topology, d: V3, min_draft: f64) -> MoldCheck {
    let d = safe_norm(d, V3::Y);
    let s_min = min_draft.to_radians().sin();
    let diag = topo.bbox.diagonal().max(1e-6);
    let mut poly_class = Vec::with_capacity(topo.polys.len());
    let mut area = [0.0; 5];
    let mut face_bad = vec![false; topo.faces.len()];
    let mut face_under = vec![0.0f64; topo.faces.len()];
    for pi in 0..topo.polys.len() {
        let n = topo.planes[pi].n;
        let c = n.dot(d);
        let a = topo.poly_area(pi);
        let mut cls = if c.abs() < 1e-3 {
            DraftClass::None
        } else if c.abs() < s_min {
            DraftClass::Low
        } else if c > 0.0 {
            DraftClass::Cavity
        } else {
            DraftClass::Core
        };
        // the half that releases this surface moves along sign(c)·d: anything of the part in
        // that direction keeps it from opening
        if c.abs() >= 1e-3 {
            let dir = d * c.signum();
            let p = topo.poly_centroid(pi) + n * (diag * 1e-5);
            if topo.ray_first_hit(p, dir, diag * 1e-6).is_some() {
                cls = DraftClass::Undercut;
            }
        }
        let k = DraftClass::ALL.iter().position(|x| *x == cls).unwrap();
        area[k] += a;
        if let Some(&f) = topo.poly_face.get(pi) {
            if f < face_bad.len() {
                if matches!(cls, DraftClass::Low | DraftClass::None) {
                    face_bad[f] = true;
                }
                if cls == DraftClass::Undercut {
                    face_under[f] += a;
                }
            }
        }
        poly_class.push(cls);
    }
    let (e1, e2) = frame(d);
    let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    for p in &topo.positions {
        for (k, ax) in [e1, e2, d].iter().enumerate() {
            let v = p.dot(*ax);
            lo[k] = lo[k].min(v);
            hi[k] = hi[k].max(v);
        }
    }
    let projected = projected_area(topo, d, e1, e2, (lo[0], hi[0]), (lo[1], hi[1]));
    let parting = parting_level(topo, d, (lo[2], hi[2]));
    let undercut_faces: Vec<usize> = (0..topo.faces.len()).filter(|&f| face_under[f] > topo.faces[f].area * 0.3).collect();
    MoldCheck {
        poly_class,
        area,
        bad_faces: (0..face_bad.len()).filter(|&f| face_bad[f]).collect(),
        undercut_faces,
        projected,
        extent: V3::new(hi[0] - lo[0], hi[1] - lo[1], hi[2] - lo[2]),
        parting,
        range: (lo[2], hi[2]),
        depth_cavity: hi[2] - parting,
        depth_core: parting - lo[2],
    }
}

/// Shadow of the part on a plane normal to `d` (rasterised, 512² cells).
fn projected_area(topo: &Topology, d: V3, e1: V3, e2: V3, r1: (f64, f64), r2: (f64, f64)) -> f64 {
    const N: usize = 512;
    let (w, h) = ((r1.1 - r1.0).max(1e-9), (r2.1 - r2.0).max(1e-9));
    let mut cov = vec![false; N * N];
    for (pi, p) in topo.polys.iter().enumerate() {
        if topo.planes[pi].n.dot(d).abs() < 1e-6 {
            continue;
        }
        for k in 1..p.len() - 1 {
            let q = [topo.positions[p[0]], topo.positions[p[k]], topo.positions[p[k + 1]]];
            let uv: Vec<(f64, f64)> = q.iter().map(|v| ((v.dot(e1) - r1.0) / w * N as f64, (v.dot(e2) - r2.0) / h * N as f64)).collect();
            let (x0, x1) =
                (uv.iter().map(|a| a.0).fold(f64::INFINITY, f64::min).floor().max(0.0) as usize, (uv.iter().map(|a| a.0).fold(f64::NEG_INFINITY, f64::max).ceil() as usize).min(N));
            let (y0, y1) =
                (uv.iter().map(|a| a.1).fold(f64::INFINITY, f64::min).floor().max(0.0) as usize, (uv.iter().map(|a| a.1).fold(f64::NEG_INFINITY, f64::max).ceil() as usize).min(N));
            let edge = |a: (f64, f64), b: (f64, f64), p: (f64, f64)| (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0);
            let area = edge(uv[0], uv[1], uv[2]);
            if area.abs() < 1e-12 {
                continue;
            }
            for y in y0..y1 {
                for x in x0..x1 {
                    let c = (x as f64 + 0.5, y as f64 + 0.5);
                    let (a, b, cc) = (edge(uv[0], uv[1], c), edge(uv[1], uv[2], c), edge(uv[2], uv[0], c));
                    if (a >= 0.0 && b >= 0.0 && cc >= 0.0) || (a <= 0.0 && b <= 0.0 && cc <= 0.0) {
                        cov[y * N + x] = true;
                    }
                }
            }
        }
    }
    cov.iter().filter(|c| **c).count() as f64 * (w * h) / (N * N) as f64
}

/// Parting plane: the level of the largest section (where the silhouette lies).
fn parting_level(topo: &Topology, d: V3, range: (f64, f64)) -> f64 {
    let n = 48;
    let levels: Vec<(f64, f64)> = (1..n)
        .map(|i| {
            let w = range.0 + (range.1 - range.0) * i as f64 / n as f64;
            (w, section_area(topo, d, w))
        })
        .collect();
    let max = levels.iter().map(|l| l.1).fold(0.0, f64::max);
    // the lowest level with (nearly) the largest section: the part stays on the moving half
    let k = levels.iter().position(|l| l.1 >= max * 0.98).unwrap_or(0);
    if k == 0 {
        // the largest section reaches down to the bottom: part the mould at the bottom face
        return range.0;
    }
    // the silhouette lies at the top of the run of largest sections only if it narrows below:
    // part where the section first reaches its largest size
    levels[k].0
}

/// Area of the cut of the solid by the plane d·x = w.
pub fn section_area(topo: &Topology, d: V3, w: f64) -> f64 {
    let o = d * w;
    let mut a2 = 0.0;
    for (pi, p) in topo.polys.iter().enumerate() {
        let pn = topo.planes[pi].n;
        for k in 1..p.len() - 1 {
            let q = [topo.positions[p[0]], topo.positions[p[k]], topo.positions[p[k + 1]]];
            let mut pts = vec![];
            for i in 0..3 {
                let (a, b) = (q[i], q[(i + 1) % 3]);
                let (da, db) = (d.dot(a) - w, d.dot(b) - w);
                if (da > 0.0) != (db > 0.0) {
                    pts.push(a + (b - a) * (da / (da - db)));
                }
            }
            if pts.len() != 2 {
                continue;
            }
            let (mut s, mut e) = (pts[0] - o, pts[1] - o);
            if (e - s).dot(d.cross(pn)) < 0.0 {
                std::mem::swap(&mut s, &mut e);
            }
            a2 += s.cross(e).dot(d);
        }
    }
    (a2 * 0.5).abs()
}

// ---------------------------------------------------------------- mould base

#[derive(Clone, Debug)]
pub struct Plate {
    pub name: &'static str,
    pub size: (f64, f64),
    pub thickness: f64,
    pub material: &'static str,
}

#[derive(Clone, Debug)]
pub struct MoldBase {
    pub cavities: usize,
    pub layout: (usize, usize),
    /// Formeinsatz (insert) size per half: (a, b, height)
    pub insert: (f64, f64, f64, f64),
    pub plates: Vec<Plate>,
    pub guide_d: f64,
    pub screw: &'static str,
    pub ejector_stroke: f64,
    pub height: f64,
    pub steel_kg: f64,
    pub opening: f64,
    pub clamp_kn: f64,
    pub machine: Option<Machine>,
    pub notes: Vec<String>,
}

#[derive(Clone, Copy, Debug)]
pub struct Machine {
    pub kn: f64,
    /// lichte Weite zwischen den Holmen (mm)
    pub tie_bar: f64,
    pub min_height: f64,
    pub max_height: f64,
    pub opening: f64,
}

pub const MACHINES: [Machine; 10] = [
    Machine { kn: 250.0, tie_bar: 260.0, min_height: 150.0, max_height: 300.0, opening: 250.0 },
    Machine { kn: 500.0, tie_bar: 320.0, min_height: 180.0, max_height: 360.0, opening: 320.0 },
    Machine { kn: 800.0, tie_bar: 370.0, min_height: 200.0, max_height: 420.0, opening: 380.0 },
    Machine { kn: 1000.0, tie_bar: 420.0, min_height: 220.0, max_height: 470.0, opening: 420.0 },
    Machine { kn: 1500.0, tie_bar: 470.0, min_height: 250.0, max_height: 520.0, opening: 480.0 },
    Machine { kn: 2000.0, tie_bar: 570.0, min_height: 280.0, max_height: 600.0, opening: 560.0 },
    Machine { kn: 3000.0, tie_bar: 670.0, min_height: 320.0, max_height: 700.0, opening: 650.0 },
    Machine { kn: 4000.0, tie_bar: 760.0, min_height: 350.0, max_height: 800.0, opening: 750.0 },
    Machine { kn: 6000.0, tie_bar: 900.0, min_height: 400.0, max_height: 950.0, opening: 900.0 },
    Machine { kn: 10000.0, tie_bar: 1200.0, min_height: 500.0, max_height: 1200.0, opening: 1200.0 },
];

const PLATE_SIZES: [f64; 15] = [156.0, 196.0, 246.0, 296.0, 346.0, 396.0, 446.0, 496.0, 546.0, 596.0, 696.0, 796.0, 896.0, 996.0, 1196.0];
const THICKNESSES: [f64; 16] = [17.0, 22.0, 27.0, 36.0, 46.0, 56.0, 66.0, 76.0, 96.0, 116.0, 136.0, 156.0, 176.0, 196.0, 246.0, 296.0];

fn std_up(list: &[f64], v: f64) -> f64 {
    list.iter().copied().find(|x| *x >= v - 1e-9).unwrap_or_else(|| (v / 50.0).ceil() * 50.0)
}

pub fn layout(cavities: usize) -> (usize, usize) {
    match cavities {
        0 | 1 => (1, 1),
        2 => (2, 1),
        3 | 4 => (2, 2),
        5 | 6 => (3, 2),
        _ => (4, 2),
    }
}

/// Plate stack of a two-plate mould with `cavities` parts of size `ext` (e1, e2, d) whose
/// depth is split `depth_cavity` / `depth_core` by the parting plane.
pub fn mold_base(ext: V3, depth_cavity: f64, depth_core: f64, cavities: usize, shrink: f64, clamp_kn_per_cavity: f64) -> MoldBase {
    let s = 1.0 + shrink / 100.0;
    let (a, b) = (ext.x.max(ext.y) * s, ext.x.min(ext.y) * s);
    let (nx, ny) = layout(cavities);
    let margin = (0.18 * a.max(b)).clamp(18.0, 60.0);
    let gap = margin.max(24.0); // room for the runner between cavities
    let ins_a = a * nx as f64 + gap * (nx as f64 - 1.0) + 2.0 * margin;
    let ins_b = b * ny as f64 + gap * (ny as f64 - 1.0) + 2.0 * margin;
    let h_cav = depth_cavity * s + (0.2 * ins_b).clamp(20.0, 60.0);
    let h_core = depth_core * s + (0.2 * ins_b).clamp(20.0, 60.0);
    // frame around the inserts: guide pillars, screws, cooling
    let frame = (0.22 * ins_a.max(ins_b)).clamp(38.0, 110.0);
    let wa = std_up(&PLATE_SIZES, ins_a + 2.0 * frame);
    let wb = std_up(&PLATE_SIZES, ins_b + 2.0 * frame);
    let (w_long, w_short) = (wa.max(wb), wa.min(wb));
    let clamp = if w_short <= 296.0 {
        27.0
    } else if w_short <= 496.0 {
        36.0
    } else {
        46.0
    };
    let a_plate = std_up(&THICKNESSES, h_cav + 15.0);
    let b_plate = std_up(&THICKNESSES, h_core + 15.0);
    let support = std_up(&THICKNESSES, (0.12 * w_short).max(27.0));
    let ejector_stroke = (depth_core * s + 10.0).max(15.0).ceil();
    let (ej_plate, ej_retainer) = if w_short <= 296.0 { (17.0, 22.0) } else { (22.0, 27.0) };
    let rails = std_up(&THICKNESSES, ejector_stroke + ej_plate + ej_retainer + 5.0);
    let rail_w = (0.17 * w_short).clamp(28.0, 110.0).round();
    let plates = vec![
        Plate { name: "Aufspannplatte Düsenseite", size: (w_long, w_short + 50.0), thickness: clamp, material: "1.1730" },
        Plate { name: "Formplatte A (Düsenseite)", size: (w_long, w_short), thickness: a_plate, material: "1.2312" },
        Plate { name: "Formplatte B (Auswerferseite)", size: (w_long, w_short), thickness: b_plate, material: "1.2312" },
        Plate { name: "Zwischenplatte / Stützplatte", size: (w_long, w_short), thickness: support, material: "1.1730" },
        Plate { name: "Leisten (2 Stück)", size: (w_long, rail_w), thickness: rails, material: "1.1730" },
        Plate { name: "Auswerfer-Halteplatte", size: (w_long - 2.0 * rail_w - 6.0, w_short - 10.0), thickness: ej_retainer, material: "1.1730" },
        Plate { name: "Auswerfer-Grundplatte", size: (w_long - 2.0 * rail_w - 6.0, w_short - 10.0), thickness: ej_plate, material: "1.1730" },
        Plate { name: "Aufspannplatte Auswerferseite", size: (w_long, w_short + 50.0), thickness: clamp, material: "1.1730" },
    ];
    let height = clamp * 2.0 + a_plate + b_plate + support + rails;
    let steel_kg: f64 = plates.iter().map(|p| p.size.0 * p.size.1 * p.thickness * if p.name.starts_with("Leisten") { 2.0 } else { 1.0 }).sum::<f64>() * 7.85e-6;
    let guide_d = if w_short <= 196.0 {
        16.0
    } else if w_short <= 296.0 {
        20.0
    } else if w_short <= 396.0 {
        24.0
    } else if w_short <= 546.0 {
        30.0
    } else {
        40.0
    };
    let screw = if w_short <= 196.0 {
        "M8"
    } else if w_short <= 346.0 {
        "M10"
    } else if w_short <= 546.0 {
        "M12"
    } else {
        "M16"
    };
    // the part comes out between the halves: open by twice its height plus room for the sprue
    let opening = (2.0 * (depth_cavity + depth_core) * s + 60.0).max(100.0);
    let clamp_kn = clamp_kn_per_cavity * cavities.max(1) as f64;
    let machine = MACHINES.iter().copied().find(|m| m.kn >= clamp_kn && m.tie_bar >= w_short && m.max_height >= height && m.opening >= opening);
    let mut notes = vec![];
    if machine.is_none() {
        notes.push("Keine Maschine der Standardreihe passt (Schließkraft, Holmabstand, Werkzeughöhe oder Öffnungsweg).".into());
    }
    if let Some(m) = machine {
        if height < m.min_height {
            notes.push(format!("Werkzeughöhe {} mm unter der Mindesthöhe der Maschine ({} mm): Distanzplatte vorsehen.", height, m.min_height));
        }
    }
    MoldBase {
        cavities: cavities.max(1),
        layout: (nx, ny),
        insert: (ins_a, ins_b, h_cav, h_core),
        plates,
        guide_d,
        screw,
        ejector_stroke,
        height,
        steel_kg,
        opening,
        clamp_kn,
        machine,
        notes,
    }
}
