//! Generative design: minimum-compliance topology optimisation (SIMP with the optimality
//! criteria update and a sensitivity filter) on the voxel mesh of the part. The design space
//! is the part itself; loads and supports come from the active structural Live-Simulation
//! study, and material where they act is kept. The result is a smoothed surface of the cells
//! that stay, with the stress and deformation of the optimised shape.

use crate::kernel::math::*;
use crate::kernel::topology::Topology;
use crate::model::sim::{material, Study, StudyKind};
use crate::sim::fea::{self, Ctl, Op};
use crate::sim::mesh::{Mesh, NONE};
use crate::sim::run::{fmt, struct_input, DispSet, Field, Shared, SimOutput, Surface};
use std::sync::Arc;

#[derive(Clone)]
pub struct OptInput {
    pub topo: Arc<Topology>,
    pub study: Study,
    /// share of the design space to keep (0.05 … 0.9)
    pub volume: f64,
    pub iterations: usize,
    /// target element count
    pub elements: usize,
    /// filter radius in cells
    pub filter: f64,
    /// Fertigung: symmetry plane normal through the part's centre (0 = X, 1 = Y, 2 = Z)
    pub symmetry: Option<usize>,
    /// keep a skin of this many cells on the outside of the design space
    pub keep_skin: bool,
}

pub struct OptResult {
    pub out: SimOutput,
    pub history: Vec<(f64, f64)>,
    pub stl: Vec<u8>,
}

const PENAL: f64 = 3.0;
const EMIN: f64 = 1e-4;

pub fn run(inp: &OptInput, sh: &Shared) -> Result<OptResult, String> {
    let t0 = std::time::Instant::now();
    let stage = |s: &str| *sh.stage.lock().unwrap() = s.into();
    let ctl = Ctl { cancel: &sh.cancel, progress: &sh.progress };
    if inp.study.kind != StudyKind::Structural {
        return Err("Generatives Design braucht eine Struktur-Studie mit Lasten und Randbedingungen.".into());
    }
    let topo = &*inp.topo;
    let mat = material(&inp.study.material);
    stage("Bauraum vernetzen");
    let mesh = Mesh::build(topo, inp.elements.max(500))?;
    let mut warnings = vec![];
    let sin = struct_input(&mesh, topo, &inp.study, &mat, &mut warnings)?;
    if sin.forces.is_empty() && sin.accel == V3::ZERO && sin.spin.is_none() {
        return Err("Die Studie hat keine Last: Kraft, Druck, Moment oder Gravitation hinzufügen.".into());
    }
    let ne = mesh.elems.len();
    let nn = mesh.nodes.len();
    let el = fea::elem_struct(mesh.grid.h, mat.e, mat.nu);
    let (free, u0, extra) = fea::supports(&mesh, &sin, el.ke[0]);
    if free.iter().all(|f| *f) && extra.is_empty() {
        return Err("Die Studie ist nicht eingespannt: mindestens eine Randbedingung (Fest …) definieren.".into());
    }
    // force vector
    let mut f = vec![0.0; 3 * nn];
    for (n, v) in &sin.forces {
        for c in 0..3 {
            f[*n as usize * 3 + c] += v[c];
        }
    }
    let vol = mesh.elem_volume();
    if sin.accel != V3::ZERO {
        for en in &mesh.elems {
            let fe = sin.accel * (sin.rho * vol / 8.0);
            for &n in en {
                for c in 0..3 {
                    f[n as usize * 3 + c] += fe[c];
                }
            }
        }
    }
    // non-design cells: where loads and supports act (and optionally the outer skin)
    let mut keep = vec![false; ne];
    let mut node_key = vec![false; nn];
    for (n, _) in &sin.forces {
        node_key[*n as usize] = true;
    }
    for (n, _) in &sin.fixed {
        node_key[*n as usize] = true;
    }
    for (n, _) in &sin.slide {
        node_key[*n as usize] = true;
    }
    let g = &mesh.grid;
    for (e, en) in mesh.elems.iter().enumerate() {
        if en.iter().any(|&n| node_key[n as usize]) {
            keep[e] = true;
        }
        if inp.keep_skin {
            let [i, j, k] = mesh.elem_ijk[e];
            let (i, j, k) = (i as i32, j as i32, k as i32);
            let n = g.n;
            let outside = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]].iter().any(|o| {
                let (a, b, c) = (i + o[0], j + o[1], k + o[2]);
                a < 0 || b < 0 || c < 0 || a >= n[0] as i32 || b >= n[1] as i32 || c >= n[2] as i32 || mesh.cell_elem[g.cell(a as usize, b as usize, c as usize)] == NONE
            });
            if outside {
                keep[e] = true;
            }
        }
    }
    // filter neighbourhoods (cells within the radius)
    let r = inp.filter.max(1.0);
    let ri = r.ceil() as i32;
    let mut nbr: Vec<Vec<(u32, f64)>> = Vec::with_capacity(ne);
    for e in 0..ne {
        let [i, j, k] = mesh.elem_ijk[e];
        let mut v = vec![];
        for dk in -ri..=ri {
            for dj in -ri..=ri {
                for di in -ri..=ri {
                    let d = ((di * di + dj * dj + dk * dk) as f64).sqrt();
                    if d > r {
                        continue;
                    }
                    let (a, b, c) = (i as i32 + di, j as i32 + dj, k as i32 + dk);
                    if a < 0 || b < 0 || c < 0 || a >= g.n[0] as i32 || b >= g.n[1] as i32 || c >= g.n[2] as i32 {
                        continue;
                    }
                    let o = mesh.cell_elem[g.cell(a as usize, b as usize, c as usize)];
                    if o != NONE {
                        v.push((o, r - d));
                    }
                }
            }
        }
        nbr.push(v);
    }
    // symmetry partner of each element
    let sym: Option<Vec<u32>> = inp.symmetry.map(|ax| {
        (0..ne)
            .map(|e| {
                let mut q = mesh.elem_ijk[e];
                q[ax] = (g.n[ax] - 1) as u16 - q[ax];
                mesh.cell_elem[g.cell(q[0] as usize, q[1] as usize, q[2] as usize)]
            })
            .collect()
    });
    let target = inp.volume.clamp(0.05, 0.95);
    let mut x = vec![target; ne];
    for e in 0..ne {
        if keep[e] {
            x[e] = 1.0;
        }
    }
    let mut u = u0.clone();
    let mut history = vec![];
    let mut fe_buf = vec![];
    let iters = inp.iterations.clamp(5, 200);
    let mut c0 = 0.0;
    let mut scale = vec![1.0; ne];
    let mut ce = vec![0.0; ne];
    for it in 0..iters {
        if ctl.cancelled() {
            return Err("abgebrochen".into());
        }
        stage(&format!("Iteration {} von {}", it + 1, iters));
        for e in 0..ne {
            scale[e] = EMIN + x[e].powf(PENAL) * (1.0 - EMIN);
        }
        let op = Op { mesh: &mesh, d: 3, ke: &el.ke, free: &free, extra: &extra, scale: Some(&scale) };
        let rhs: Vec<f64> = (0..3 * nn).map(|i| if free[i] { f[i] } else { u0[i] }).collect();
        let span = (it as f32 / iters as f32 * 0.95, (it as f32 + 0.9) / iters as f32 * 0.95);
        let (_, resid) = fea::pcg(&op, &rhs, &mut u, 1e-5, 6000, &ctl, span)?;
        if resid > 0.05 && it == 0 {
            return Err("Die Lösung konvergiert nicht: die Studie ist nicht ausreichend eingespannt.".into());
        }
        // element energies u_eᵀ K0 u_e
        let mut comp = 0.0;
        for (e, en) in mesh.elems.iter().enumerate() {
            let mut ue = [0.0; 24];
            for a in 0..8 {
                for c in 0..3 {
                    ue[a * 3 + c] = u[en[a] as usize * 3 + c];
                }
            }
            let mut s = 0.0;
            for i in 0..24 {
                let row = &el.ke[i * 24..(i + 1) * 24];
                let mut t = 0.0;
                for j in 0..24 {
                    t += row[j] * ue[j];
                }
                s += ue[i] * t;
            }
            ce[e] = s;
            comp += scale[e] * s;
        }
        if it == 0 {
            c0 = comp;
        }
        let vol_frac = x.iter().sum::<f64>() / ne as f64;
        history.push((comp, vol_frac));
        // sensitivities, filtered
        let dc: Vec<f64> = (0..ne).map(|e| -PENAL * x[e].powf(PENAL - 1.0) * (1.0 - EMIN) * ce[e]).collect();
        let mut dcf = vec![0.0; ne];
        for e in 0..ne {
            let (mut s, mut w) = (0.0, 0.0);
            for &(o, h) in &nbr[e] {
                s += h * x[o as usize] * dc[o as usize];
                w += h;
            }
            dcf[e] = s / (x[e].max(1e-3) * w.max(1e-12));
        }
        if let Some(sym) = &sym {
            let copy = dcf.clone();
            for e in 0..ne {
                let p = sym[e];
                if p != NONE {
                    dcf[e] = 0.5 * (copy[e] + copy[p as usize]);
                }
            }
        }
        // optimality criteria with bisection on the volume multiplier
        let (mut l1, mut l2) = (1e-12f64, 1e12f64);
        let mv = 0.2;
        let mut xn = x.clone();
        while (l2 - l1) / (l1 + l2) > 1e-4 {
            let lm = (l1 * l2).sqrt();
            for e in 0..ne {
                if keep[e] {
                    xn[e] = 1.0;
                    continue;
                }
                let be = (-dcf[e] / lm).max(0.0).sqrt();
                xn[e] = (x[e] * be).clamp((x[e] - mv).max(0.001), (x[e] + mv).min(1.0));
            }
            if xn.iter().sum::<f64>() > target * ne as f64 {
                l1 = lm;
            } else {
                l2 = lm;
            }
        }
        let change = x.iter().zip(&xn).map(|(a, b)| (a - b).abs()).fold(0.0, f64::max);
        x = xn;
        if it % 5 == 4 || it + 1 == iters {
            if let Ok(o) = output(&mesh, &x, &u, &el, &mat, true, inp, &history, c0, comp, t0, &warnings) {
                *sh.partial.lock().unwrap() = Some(o.0);
            }
        }
        if it > 10 && change < 0.01 {
            break;
        }
    }
    // final state: solve once more with the last densities for stresses
    for e in 0..ne {
        scale[e] = EMIN + x[e].powf(PENAL) * (1.0 - EMIN);
    }
    let op = Op { mesh: &mesh, d: 3, ke: &el.ke, free: &free, extra: &extra, scale: Some(&scale) };
    let rhs: Vec<f64> = (0..3 * nn).map(|i| if free[i] { f[i] } else { u0[i] }).collect();
    fea::pcg(&op, &rhs, &mut u, 1e-6, 8000, &ctl, (0.95, 0.99))?;
    let comp: f64 = {
        let mut kx = vec![0.0; 3 * nn];
        op.apply(&u, &mut kx, &mut fe_buf);
        u.iter().zip(&kx).enumerate().filter(|(i, _)| free[*i]).map(|(_, (a, b))| a * b).sum()
    };
    let (out, stl) = output(&mesh, &x, &u, &el, &mat, false, inp, &history, c0, comp, t0, &warnings)?;
    ctl.set(1.0);
    Ok(OptResult { out, history, stl })
}

#[allow(clippy::too_many_arguments)]
fn output(
    mesh: &Mesh,
    x: &[f64],
    u: &[f64],
    el: &fea::ElemStruct,
    mat: &crate::model::sim::Material,
    partial: bool,
    inp: &OptInput,
    history: &[(f64, f64)],
    c0: f64,
    comp: f64,
    t0: std::time::Instant,
    warnings: &[String],
) -> Result<(SimOutput, Vec<u8>), String> {
    let g = &mesh.grid;
    let ne = mesh.elems.len();
    let on = |e: u32| e != NONE && x[e as usize] >= 0.5;
    // exposed faces of the cells that stay, as quads on grid nodes
    let nn1 = [g.n[0] + 1, g.n[1] + 1];
    let node_key = |i: usize, j: usize, k: usize| i + nn1[0] * (j + nn1[1] * k);
    let mut vid: std::collections::HashMap<usize, u32> = std::collections::HashMap::new();
    let mut verts: Vec<V3> = vec![];
    let mut vnode: Vec<u32> = vec![];
    let mut quads: Vec<[u32; 4]> = vec![];
    let node_index: std::collections::HashMap<usize, u32> = {
        let mut m = std::collections::HashMap::new();
        for (e, en) in mesh.elems.iter().enumerate() {
            let [i, j, k] = mesh.elem_ijk[e];
            for a in 0..8 {
                m.insert(node_key(i as usize + (a & 1), j as usize + (a >> 1 & 1), k as usize + (a >> 2 & 1)), en[a]);
            }
        }
        m
    };
    let mut vert = |i: usize, j: usize, k: usize, verts: &mut Vec<V3>, vnode: &mut Vec<u32>| -> u32 {
        let key = node_key(i, j, k);
        *vid.entry(key).or_insert_with(|| {
            verts.push(g.origin + V3::new(i as f64 * g.h.x, j as f64 * g.h.y, k as f64 * g.h.z));
            vnode.push(*node_index.get(&key).unwrap_or(&0));
            (verts.len() - 1) as u32
        })
    };
    let mut kept = 0usize;
    for e in 0..ne {
        if !on(e as u32) {
            continue;
        }
        kept += 1;
        let [i, j, k] = mesh.elem_ijk[e];
        let (i, j, k) = (i as usize, j as usize, k as usize);
        for (ax, sgn) in [(0, 1i32), (0, -1), (1, 1), (1, -1), (2, 1), (2, -1)] {
            let mut q = [i as i32, j as i32, k as i32];
            q[ax] += sgn;
            let inside = q.iter().zip(g.n.iter()).all(|(a, n)| *a >= 0 && (*a as usize) < *n) && on(mesh.cell_elem[g.cell(q[0] as usize, q[1] as usize, q[2] as usize)]);
            if inside {
                continue;
            }
            // the four corners of the face, counter-clockwise seen from outside
            let base = [i, j, k];
            let (a1, a2) = ((ax + 1) % 3, (ax + 2) % 3);
            let mut c = [[0usize; 3]; 4];
            for (n, (s1, s2)) in [(0, 0), (1, 0), (1, 1), (0, 1)].iter().enumerate() {
                let mut p = base;
                if sgn > 0 {
                    p[ax] += 1;
                }
                p[a1] += s1;
                p[a2] += s2;
                c[n] = p;
            }
            if sgn < 0 {
                c.swap(1, 3);
            }
            let ids = c.map(|p| vert(p[0], p[1], p[2], &mut verts, &mut vnode));
            quads.push(ids);
        }
    }
    if quads.is_empty() {
        return Err("Es bleibt kein Material übrig: Volumenanteil erhöhen.".into());
    }
    // Taubin smoothing of the stair steps (no shrinkage)
    let mut adj: Vec<Vec<u32>> = vec![vec![]; verts.len()];
    for q in &quads {
        for k in 0..4 {
            let (a, b) = (q[k], q[(k + 1) % 4]);
            if !adj[a as usize].contains(&b) {
                adj[a as usize].push(b);
            }
            if !adj[b as usize].contains(&a) {
                adj[b as usize].push(a);
            }
        }
    }
    let rest = verts.clone();
    for pass in 0..12 {
        let lam = if pass % 2 == 0 { 0.5 } else { -0.53 };
        let prev = verts.clone();
        for v in 0..verts.len() {
            if adj[v].is_empty() {
                continue;
            }
            let avg = adj[v].iter().map(|&n| prev[n as usize]).sum::<V3>() / adj[v].len() as f64;
            verts[v] = prev[v] + (avg - prev[v]) * lam;
        }
    }
    // never wander more than a cell from the voxel corner
    let hmax = g.h.max_element();
    for v in 0..verts.len() {
        let d = verts[v] - rest[v];
        if d.length() > hmax {
            verts[v] = rest[v] + d.normalize() * hmax;
        }
    }
    // results per vertex: displacement of its node, stress of its cells
    let nn = mesh.nodes.len();
    let un: Vec<V3> = (0..nn).map(|n| V3::new(u[3 * n], u[3 * n + 1], u[3 * n + 2])).collect();
    let mut vm_node = vec![0.0f64; nn];
    let mut cnt = vec![0u32; nn];
    for (e, en) in mesh.elems.iter().enumerate() {
        if x[e] < 0.5 {
            continue;
        }
        let mut ue = [0.0; 24];
        for a in 0..8 {
            for c in 0..3 {
                ue[a * 3 + c] = u[en[a] as usize * 3 + c];
            }
        }
        let mut s = [0.0f64; 6];
        for a in 0..8 {
            for r in 0..6 {
                for j in 0..24 {
                    s[r] += el.stress[a][r][j] * ue[j] / 8.0;
                }
            }
        }
        let sc = x[e].powf(PENAL);
        let v = fea::von_mises(&s) * sc;
        for &n in en {
            vm_node[n as usize] += v;
            cnt[n as usize] += 1;
        }
    }
    for n in 0..nn {
        vm_node[n] /= cnt[n].max(1) as f64;
    }
    let mut s = Surface::default();
    let mut vals_vm = vec![];
    let mut vals_u = vec![];
    let mut dvec = vec![];
    let mut stl: Vec<u8> = vec![0u8; 80];
    let ntri = quads.len() * 2;
    stl.extend_from_slice(&(ntri as u32).to_le_bytes());
    let f3 = |v: V3| [v.x as f32, v.y as f32, v.z as f32];
    // smooth vertex normals
    let mut vn = vec![V3::ZERO; verts.len()];
    for q in &quads {
        let n = (verts[q[2] as usize] - verts[q[0] as usize]).cross(verts[q[3] as usize] - verts[q[1] as usize]);
        for &i in q {
            vn[i as usize] += n;
        }
    }
    for q in &quads {
        for tri in [[q[0], q[1], q[2]], [q[0], q[2], q[3]]] {
            let fnrm = safe_norm((verts[tri[1] as usize] - verts[tri[0] as usize]).cross(verts[tri[2] as usize] - verts[tri[0] as usize]), V3::Z);
            stl.extend(f3(fnrm).iter().flat_map(|v| v.to_le_bytes()));
            for &i in &tri {
                let i = i as usize;
                s.pos.push(f3(verts[i]));
                s.nrm.push(f3(safe_norm(vn[i], fnrm)));
                let node = vnode[i] as usize;
                vals_vm.push(vm_node[node] as f32);
                vals_u.push(un[node].length() as f32);
                dvec.push(f3(un[node]));
                stl.extend(f3(verts[i]).iter().flat_map(|v| v.to_le_bytes()));
            }
            stl.extend_from_slice(&[0, 0]);
        }
    }
    let umax = un.iter().map(|v| v.length()).fold(0.0, f64::max) as f32;
    let disps = vec![DispSet { d: dvec, edge_d: vec![], max: umax, harmonic: false }];
    let fields = vec![Field::new("Von-Mises-Spannung", "MPa", vals_vm, Some(0)), Field::new("Verschiebung (Betrag)", "mm", vals_u, Some(0))];
    let vol0 = ne as f64 * mesh.elem_volume();
    let vol1 = kept as f64 * mesh.elem_volume();
    let rho = mat.rho * 1e-9; // kg/mm³
    let mut summary = vec![
        ("Material".to_string(), mat.name.to_string()),
        ("Bauraum".to_string(), format!("{} cm³, {} kg", fmt(vol0 / 1000.0), fmt(vol0 * rho))),
        ("Ergebnis".to_string(), format!("{} cm³, {} kg ({:.0} % des Bauraums)", fmt(vol1 / 1000.0), fmt(vol1 * rho), 100.0 * vol1 / vol0.max(1e-9))),
        ("Masseeinsparung".to_string(), format!("{} kg", fmt((vol0 - vol1) * rho))),
        ("Nachgiebigkeit voll / optimiert".to_string(), format!("{} / {} N·mm", fmt(c0), fmt(comp))),
        ("Steifigkeit je Masse".to_string(), if comp > 0.0 && c0 > 0.0 { format!("{:.2} × Bauraum", (c0 / comp) / (vol1 / vol0.max(1e-9))) } else { "–".into() }),
        ("Max. Verschiebung".to_string(), format!("{} mm", fmt(umax as f64))),
        ("Iterationen".to_string(), format!("{}", history.len())),
        ("Netz".to_string(), format!("{} Elemente ({} mm)", ne, fmt(mesh.grid.h.x))),
    ];
    if let Some(ax) = inp.symmetry {
        summary.push(("Symmetrie".into(), format!("Ebene normal zu {}", ["X", "Y", "Z"][ax])));
    }
    let mut w = warnings.to_vec();
    if mesh.dropped > 0.01 {
        w.push(format!("{:.0} % des Bauraums hängen nicht am Hauptkörper und wurden weggelassen.", mesh.dropped * 100.0));
    }
    let vmax = fields[0].max as f64;
    if vmax > mat.yield_mpa {
        w.push(format!(
            "Die Spannung der optimierten Form ({} MPa) überschreitet die Streckgrenze von {} ({} MPa): Volumenanteil erhöhen.",
            fmt(vmax),
            mat.name,
            fmt(mat.yield_mpa)
        ));
    }
    Ok((
        SimOutput {
            kind: StudyKind::Structural,
            surface: Arc::new(s),
            fields,
            disps,
            lines: vec![],
            summary,
            warnings: w,
            table: (vec![], vec![]),
            partial,
            seconds: t0.elapsed().as_secs_f64(),
            see_through: false,
        },
        stl,
    ))
}
