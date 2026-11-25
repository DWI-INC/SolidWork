//! Support. First every layer is analysed: overhangs steeper than the threshold angle, the
//! ones the printer can bridge on its own (held on two or more sides, short enough), and
//! islands that start in mid-air. What really needs support is then held up either by
//! normal grid support (projected straight down) or by tree support: thin tips under the
//! overhang whose branches lean towards each other, merge, thicken on the way down and steer
//! around the part to the bed (or onto the part). Both keep `xy` away from the part, `top_z`
//! below and `bottom_z` above it, with dense interface layers right under the part.

use super::config::{Process, SupportStyle};
use super::poly::{self, Pt, Shape, Shapes};

#[derive(Clone, Debug, Default)]
pub struct Report {
    /// overhang area (summed over layers, mm²) that needs support
    pub overhang_area: f64,
    /// overhangs printed as bridges instead of supported, and their area
    pub bridges: usize,
    pub bridged_area: f64,
    /// part islands that start in mid-air (nothing of the part below them)
    pub floating: usize,
    pub first_floating_z: Option<f64>,
    /// highest overhang that needs support (mm)
    pub top_overhang_z: Option<f64>,
    /// tree support: branch tips placed, branches that reached the bed / rest on the part
    pub tips: usize,
    pub on_bed: usize,
    pub on_part: usize,
    /// tips that found no way down (build-plate-only support blocked by the part)
    pub stranded: usize,
}

impl Report {
    /// The part will not print well without support.
    pub fn needs_support(&self) -> bool {
        self.floating > 0 || self.overhang_area > 20.0
    }
}

pub struct SupportLayers {
    /// sparse support area per layer (line centres)
    pub base: Vec<Shapes>,
    /// dense interface area per layer
    pub interface: Vec<Shapes>,
    /// tree support: the base is printed as walls around branches, not as grid lines
    pub tree: bool,
    pub report: Report,
}

/// Overhang analysis of every layer: (overhang that needs support, report).
pub fn analyse(slices: &[Shapes], heights: &[f64], q: &Process, w: f64) -> (Vec<Shapes>, Report) {
    let n = slices.len();
    let mut rep = Report::default();
    let mut need: Vec<Shapes> = vec![vec![]; n];
    let tan = q.support_threshold.clamp(1.0, 89.0).to_radians().tan();
    let mut z = heights.first().copied().unwrap_or(0.0);
    for i in 1..n {
        z += heights[i];
        if slices[i].is_empty() {
            continue;
        }
        let allowed = heights[i] / tan;
        let lower = poly::offset(&slices[i - 1], allowed);
        // thin slivers matter: under a shallow slope every layer adds one and together they
        // cover the whole surface, so only numerical crumbs are dropped here
        let over = poly::drop_small(poly::diff(&slices[i], &lower), 1e-3);
        if over.is_empty() {
            continue;
        }
        // islands in mid-air: a part of the layer that does not touch the layer below
        let touch = poly::offset(&slices[i - 1], w * 0.5);
        for sh in &slices[i] {
            let one: Shapes = vec![sh.clone()];
            if poly::shape_area(sh) > w * w && poly::inter(&one, &touch).is_empty() {
                rep.floating += 1;
                rep.first_floating_z.get_or_insert(z);
            }
        }
        let mut keep: Shapes = vec![];
        for piece in over {
            if q.support_smart_bridges && bridgeable(&piece, &slices[i - 1], allowed + w, q.bridge_max, w) {
                rep.bridges += 1;
                rep.bridged_area += poly::shape_area(&piece);
            } else {
                keep.push(piece);
            }
        }
        let a = poly::area(&keep);
        // slivers under shallow slopes add up; single crumbs do not count as "needs support"
        if a > w * w * 0.25 {
            rep.overhang_area += a;
            rep.top_overhang_z = Some(z);
        }
        need[i] = keep;
    }
    (need, rep)
}

/// An overhang the printer can bridge: anchored on the layer below in two or more places
/// (or all around, like the ceiling of a hole), with every point no further than half the
/// longest bridge from an anchor. A ledge held on one side only is a cantilever and is not.
pub fn bridgeable(piece: &Shape, lower: &Shapes, grow: f64, max_span: f64, w: f64) -> bool {
    if max_span <= 0.0 || lower.is_empty() {
        return false;
    }
    let one: Shapes = vec![piece.clone()];
    let anchors = poly::drop_small(poly::inter(&poly::offset_join(&one, grow, true), lower), w * w);
    let held = anchors.len() >= 2 || anchors.iter().any(|a| a.len() > 1);
    if !held {
        return false;
    }
    let reach = poly::offset_join(&anchors, max_span / 2.0 + grow, true);
    poly::area(&poly::diff(&one, &reach)) < w * w
}

/// `slices`: model region per layer, `h`: layer heights, `w`: support line width.
pub fn generate(slices: &[Shapes], heights: &[f64], q: &Process, w: f64) -> SupportLayers {
    let n = slices.len();
    let mut base = vec![vec![]; n];
    let mut interface = vec![vec![]; n];
    let tree = q.support_style == SupportStyle::Tree;
    if n < 2 {
        return SupportLayers { base, interface, tree, report: Report::default() };
    }
    let (overhang, mut report) = analyse(slices, heights, q, w);
    if !q.support {
        return SupportLayers { base, interface, tree, report };
    }
    let h = q.layer_height.max(1e-3);
    let gap_top = if q.support_top_z <= 1e-6 { 0 } else { ((q.support_top_z / h).round() as usize).max(1) };
    let gap_bottom = if q.support_bottom_z <= 1e-6 { 0 } else { ((q.support_bottom_z / h).round() as usize).max(1) };
    // part grown by the xy distance, per layer
    let keep_out: Vec<Shapes> = slices.iter().map(|s| poly::offset_join(s, q.support_xy, true)).collect();
    // bed-only: everything above the part's shadow is removed
    let mut shadow: Vec<Shapes> = vec![vec![]; n];
    if q.support_build_plate_only || tree {
        let mut acc: Shapes = vec![];
        for i in 0..n {
            acc = poly::union(&acc, &keep_out[i]);
            shadow[i] = acc.clone();
        }
    }
    let blocked_at = |j: usize| -> Shapes {
        // not inside the part on this layer or in the top gap above it, not in the bottom
        // gap over the part it stands on, nowhere over the part for bed-only support
        let mut b: Vec<&Shapes> = vec![];
        for k in j..=(j + gap_top).min(n - 1) {
            b.push(&keep_out[k]);
        }
        for k in 1..=gap_bottom {
            if j >= k {
                b.push(&keep_out[j - k]);
            }
        }
        if q.support_build_plate_only && j > 0 {
            b.push(&shadow[j - 1]);
        }
        poly::union_all(&b)
    };
    let min_area = if q.support_remove_small { (w * 2.5).powi(2) } else { w * w * 0.5 };
    // interface: right under overhangs (the next `interface_layers` layers above the gap)
    let near_of = |j: usize, layers: usize| -> Shapes {
        let mut near: Shapes = vec![];
        for k in 0..layers {
            let src = j + 1 + gap_top + k;
            if src < n && !overhang[src].is_empty() {
                near = poly::union(&near, &overhang[src]);
            }
        }
        near
    };
    if tree {
        let (b, i) = tree_support(slices, heights, q, w, &overhang, &keep_out, &shadow, gap_top, &mut report);
        for j in 0..n {
            if b[j].is_empty() && i[j].is_empty() {
                continue;
            }
            let blocked = blocked_at(j);
            let iface = poly::drop_small(poly::diff(&i[j], &blocked), w * w);
            let s = poly::diff(&poly::diff(&b[j], &blocked), &iface);
            base[j] = poly::drop_small(s, w * w * 0.75);
            interface[j] = iface;
        }
        return SupportLayers { base, interface, tree, report };
    }
    let mut carry: Shapes = vec![];
    for j in (0..n.saturating_sub(1)).rev() {
        // overhangs that start supporting at this layer (gap_top layers below them)
        let src = j + 1 + gap_top;
        if src < n && !overhang[src].is_empty() {
            carry = poly::union(&carry, &overhang[src]);
        }
        if carry.is_empty() {
            continue;
        }
        let blocked = blocked_at(j);
        let s = poly::diff(&carry, &blocked);
        // what hits the part stops there: below, the part is in the way
        carry = poly::diff(&carry, &slices[j]);
        // closing merges crumbs into printable islands; it must not refill the gaps
        let s = poly::drop_small(poly::diff(&poly::close(&s, w), &blocked), min_area);
        if s.is_empty() {
            continue;
        }
        let near = near_of(j, q.interface_layers as usize);
        let iface = poly::inter(&s, &poly::offset_join(&near, w, true));
        base[j] = poly::diff(&s, &iface);
        interface[j] = iface;
    }
    SupportLayers { base, interface, tree, report }
}

// ------------------------------------------------------------------ tree support

#[derive(Clone, Debug)]
struct Node {
    pos: Pt,
    /// height travelled down from the tip (mm), sets the branch radius
    run: f64,
    /// weight for merging (number of tips held)
    tips: f64,
}

/// Nearest point on the boundary of `s` and its distance from `p`.
fn nearest_edge(s: &Shapes, p: Pt) -> Option<(Pt, f64)> {
    let mut best: Option<(Pt, f64)> = None;
    for sh in s {
        for c in sh {
            let m = c.len();
            for k in 0..m {
                let (a, b) = (c[k], c[(k + 1) % m]);
                let d = [b[0] - a[0], b[1] - a[1]];
                let l2 = d[0] * d[0] + d[1] * d[1];
                let t = if l2 < 1e-18 { 0.0 } else { (((p[0] - a[0]) * d[0] + (p[1] - a[1]) * d[1]) / l2).clamp(0.0, 1.0) };
                let q = [a[0] + d[0] * t, a[1] + d[1] * t];
                let dd = poly::dist(p, q);
                if best.is_none_or(|(_, bd)| dd < bd) {
                    best = Some((q, dd));
                }
            }
        }
    }
    best
}

/// Signed distance from `p` to region `s`: negative inside.
fn signed_dist(s: &Shapes, p: Pt) -> (f64, Pt) {
    match nearest_edge(s, p) {
        None => (f64::INFINITY, p),
        Some((q, d)) => (if poly::in_shapes(s, p) { -d } else { d }, q),
    }
}

/// Moves `p` so that it is at least `clear` outside `s`, by no more than `budget`.
/// None when that is not possible.
fn push_out(s: &Shapes, p: Pt, clear: f64, budget: f64) -> Option<Pt> {
    let (d, q) = signed_dist(s, p);
    if d >= clear {
        return Some(p);
    }
    // direction away from the boundary
    let mut dir = [p[0] - q[0], p[1] - q[1]];
    let l = (dir[0] * dir[0] + dir[1] * dir[1]).sqrt();
    if l < 1e-9 {
        return None;
    }
    if d < 0.0 {
        dir = [-dir[0], -dir[1]];
    }
    let dir = [dir[0] / l, dir[1] / l];
    let need = clear - d;
    if need > budget + 1e-9 {
        return None;
    }
    let out = [p[0] + dir[0] * need, p[1] + dir[1] * need];
    // a concave corner can push into another wall; accept only a clear result
    (signed_dist(s, out).0 >= clear * 0.5).then_some(out)
}

fn circle(c: Pt, r: f64) -> Shape {
    let seg = ((r * 10.0) as usize).clamp(10, 40);
    vec![(0..seg)
        .map(|k| {
            let a = std::f64::consts::TAU * k as f64 / seg as f64;
            [c[0] + r * a.cos(), c[1] + r * a.sin()]
        })
        .collect()]
}

/// Tip positions on a grid inside `area` (at least one per piece).
fn tips_in(area: &Shapes, spacing: f64, inset: f64) -> Vec<Pt> {
    let mut out = vec![];
    for sh in area {
        let one: Shapes = vec![sh.clone()];
        let core = poly::offset(&one, -inset);
        let mut got = 0;
        if let Some((lo, hi)) = poly::bbox(&core) {
            // grid aligned to the world so tips of neighbouring layers line up
            let x0 = (lo[0] / spacing).floor() * spacing + spacing * 0.5;
            let y0 = (lo[1] / spacing).floor() * spacing + spacing * 0.5;
            let mut y = y0;
            while y <= hi[1] {
                let mut x = x0;
                while x <= hi[0] {
                    if poly::in_shapes(&core, [x, y]) {
                        out.push([x, y]);
                        got += 1;
                    }
                    x += spacing;
                }
                y += spacing;
            }
        }
        if got == 0 {
            // thin or small piece: one tip at its widest point
            let src = if core.is_empty() { one.clone() } else { core };
            let c = &src[0][0];
            let k = c.len().max(1) as f64;
            let m = [c.iter().map(|p| p[0]).sum::<f64>() / k, c.iter().map(|p| p[1]).sum::<f64>() / k];
            if poly::in_shapes(&src, m) {
                out.push(m);
            } else if let Some(p) = c.first() {
                out.push(*p);
            }
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn tree_support(
    slices: &[Shapes],
    heights: &[f64],
    q: &Process,
    w: f64,
    need: &[Shapes],
    keep_out: &[Shapes],
    shadow: &[Shapes],
    gap_top: usize,
    rep: &mut Report,
) -> (Vec<Shapes>, Vec<Shapes>) {
    let n = slices.len();
    let mut circles: Vec<Vec<Shape>> = vec![vec![]; n];
    let mut roof: Vec<Shapes> = vec![vec![]; n];
    let tip_r = (q.tree_tip_diameter / 2.0).max(w);
    let trunk_r = (q.tree_trunk_diameter / 2.0).max(tip_r);
    let lean = q.tree_angle.clamp(5.0, 75.0).to_radians().tan();
    // branches thicken by about 1 mm in diameter per 12 mm of height
    let grow = 0.04;
    let roof_layers = (q.interface_layers as usize).max(2);
    let spacing = (q.support_spacing * 1.6).clamp(2.5, 8.0);
    let radius = |run: f64| (tip_r + run * grow).min(trunk_r);
    let mut nodes: Vec<Node> = vec![];
    for j in (0..n).rev() {
        let h = heights[j].max(1e-3);
        // ---- roof (dense) right under the overhangs that start here
        for k in 0..roof_layers {
            let src = j + 1 + gap_top + k;
            if src < n && !need[src].is_empty() {
                roof[j] = poly::union(&roof[j], &need[src]);
            }
        }
        // ---- new tips below the lowest roof layer of an overhang
        let src = j + 1 + gap_top + roof_layers;
        if src < n && !need[src].is_empty() {
            let area = poly::diff(&poly::offset_join(&need[src], w * 0.5, true), &keep_out[j]);
            for p in tips_in(&area, spacing, tip_r * 0.5) {
                rep.tips += 1;
                nodes.push(Node { pos: p, run: 0.0, tips: 1.0 });
            }
            // tips stand under the roof: a thin roof carries them from here up
            if roof_layers > 0 && !area.is_empty() {
                roof[j] = poly::union(&roof[j], &area);
            }
        }
        if nodes.is_empty() {
            continue;
        }
        // ---- draw this layer
        for nd in &nodes {
            circles[j].push(circle(nd.pos, radius(nd.run)));
        }
        if j == 0 {
            rep.on_bed += nodes.len();
            break;
        }
        // ---- move every node one layer down
        let step = h * lean;
        let below = j - 1;
        let avoid = &keep_out[below];
        let mut next: Vec<Node> = Vec::with_capacity(nodes.len());
        let count = nodes.len();
        for a in 0..count {
            let nd = &nodes[a];
            let r = radius(nd.run + h);
            let mut want = nd.pos;
            // to the bed: leave the part's shadow (it would end on the part)
            let escaping = (q.support_build_plate_only || !shadow[below].is_empty()) && poly::in_shapes(&shadow[below], nd.pos);
            if escaping {
                if let Some((e, _)) = nearest_edge(&shadow[below], nd.pos) {
                    let d = poly::dist(e, nd.pos).max(1e-9);
                    let s = step.min(d + r);
                    want = [nd.pos[0] + (e[0] - nd.pos[0]) / d * s, nd.pos[1] + (e[1] - nd.pos[1]) / d * s];
                }
            } else {
                // lean towards the nearest neighbour so the branches meet and merge
                let mut best: Option<(usize, f64)> = None;
                for b in 0..count {
                    if b == a {
                        continue;
                    }
                    let d = poly::dist(nd.pos, nodes[b].pos);
                    if d < 12.0 && best.is_none_or(|(_, bd)| d < bd) {
                        best = Some((b, d));
                    }
                }
                if let Some((b, d)) = best {
                    let s = step.min(d * 0.5);
                    let o = nodes[b].pos;
                    want = [nd.pos[0] + (o[0] - nd.pos[0]) / d.max(1e-9) * s, nd.pos[1] + (o[1] - nd.pos[1]) / d.max(1e-9) * s];
                }
            }
            // keep clear of the part
            let clear = r.min(trunk_r) * 0.6 + w * 0.25;
            let moved = push_out(avoid, want, clear, step * 1.5).or_else(|| push_out(avoid, nd.pos, clear, step * 1.5));
            match moved {
                Some(p) => next.push(Node { pos: p, run: nd.run + h, tips: nd.tips }),
                None => {
                    // the part is in the way: rest on it unless only the bed may carry support
                    if poly::in_shapes(&slices[below], want) || poly::in_shapes(&slices[below], nd.pos) {
                        if q.support_build_plate_only {
                            rep.stranded += 1;
                        } else {
                            rep.on_part += 1;
                        }
                    } else if let Some(p) = push_out(avoid, nd.pos, w * 0.5, step * 4.0) {
                        // tight spot: squeeze past with a thin branch
                        next.push(Node { pos: p, run: nd.run + h, tips: nd.tips });
                    } else if q.support_build_plate_only {
                        rep.stranded += 1;
                    } else {
                        rep.on_part += 1;
                    }
                }
            }
        }
        // ---- merge branches that touch
        let mut merged: Vec<Node> = vec![];
        let mut used = vec![false; next.len()];
        for a in 0..next.len() {
            if used[a] {
                continue;
            }
            let mut m = next[a].clone();
            used[a] = true;
            loop {
                let mut hit = false;
                for b in 0..next.len() {
                    if used[b] {
                        continue;
                    }
                    let ra = radius(m.run);
                    let rb = radius(next[b].run);
                    if poly::dist(m.pos, next[b].pos) < (ra + rb) * 0.45 {
                        let (wa, wb) = (m.tips, next[b].tips);
                        let t = wa + wb;
                        let p = [(m.pos[0] * wa + next[b].pos[0] * wb) / t, (m.pos[1] * wa + next[b].pos[1] * wb) / t];
                        // the merged centre must stay clear too, else keep the heavier one
                        if signed_dist(avoid, p).0 > 0.0 {
                            m.pos = p;
                        } else if wb > wa {
                            m.pos = next[b].pos;
                        }
                        m.run = m.run.max(next[b].run) + (wa.min(wb)).sqrt() * 0.5;
                        m.tips = t;
                        used[b] = true;
                        hit = true;
                    }
                }
                if !hit {
                    break;
                }
            }
            merged.push(m);
        }
        nodes = merged;
    }
    let base: Vec<Shapes> = circles.iter().map(|c| if c.is_empty() { vec![] } else { poly::from_loops(&c.iter().map(|s| s[0].clone()).collect::<Vec<_>>()) }).collect();
    (base, roof)
}
