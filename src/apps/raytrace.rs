//! Render Studio: a progressive path tracer for the part. Physically based materials
//! (diffuse, glossy dielectric, metal, clear-coated paint), studio or outdoor lighting with a
//! soft key light sampled directly, a ground plane that catches shadows (optionally a mirror
//! finish), filmic tone mapping. Samples accumulate on worker threads while the window shows
//! the picture getting cleaner.

use super::Rng;
use crate::app::camera::Camera;
use crate::kernel::math::*;
use crate::kernel::topology::Topology;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Material {
    pub name: &'static str,
    pub metal: bool,
    pub roughness: f64,
    /// clear coat over a diffuse base (car paint)
    pub coat: bool,
    /// fixed colour (None: the part's colour)
    pub color: Option<[f64; 3]>,
}

pub const MATERIALS: [Material; 9] = [
    Material { name: "Kunststoff glänzend", metal: false, roughness: 0.18, coat: false, color: None },
    Material { name: "Kunststoff matt", metal: false, roughness: 0.55, coat: false, color: None },
    Material { name: "Autolack", metal: false, roughness: 0.35, coat: true, color: None },
    Material { name: "Gummi", metal: false, roughness: 0.85, coat: false, color: Some([0.05, 0.05, 0.055]) },
    Material { name: "Aluminium gebürstet", metal: true, roughness: 0.28, coat: false, color: Some([0.91, 0.92, 0.92]) },
    Material { name: "Edelstahl", metal: true, roughness: 0.16, coat: false, color: Some([0.78, 0.77, 0.74]) },
    Material { name: "Chrom", metal: true, roughness: 0.03, coat: false, color: Some([0.95, 0.95, 0.96]) },
    Material { name: "Messing", metal: true, roughness: 0.12, coat: false, color: Some([0.91, 0.78, 0.42]) },
    Material { name: "Kupfer", metal: true, roughness: 0.14, coat: false, color: Some([0.96, 0.64, 0.54]) },
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scene {
    StudioLight,
    StudioDark,
    Outdoor,
}

impl Scene {
    pub const ALL: [Scene; 3] = [Scene::StudioLight, Scene::StudioDark, Scene::Outdoor];
    pub fn name(self) -> &'static str {
        match self {
            Scene::StudioLight => "Studio hell",
            Scene::StudioDark => "Studio dunkel",
            Scene::Outdoor => "Außen (Himmel)",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    pub width: usize,
    pub height: usize,
    pub scene: Scene,
    pub material: Material,
    pub color: [f64; 3],
    pub ground: bool,
    pub ground_mirror: bool,
    pub exposure: f64,
    pub max_samples: u32,
    pub bounces: u32,
    /// key light: azimuth and elevation in degrees
    pub light_az: f64,
    pub light_el: f64,
}

pub struct Render {
    pub accum: Mutex<Vec<[f32; 3]>>,
    pub samples: AtomicU32,
    pub cancel: AtomicBool,
    pub settings: Settings,
    pub started: std::time::Instant,
}

impl Render {
    /// Tone-mapped sRGB pixels (RGBA) of what has accumulated so far.
    pub fn image(&self) -> Vec<u8> {
        let n = self.samples.load(Ordering::Relaxed).max(1) as f32;
        let acc = self.accum.lock().unwrap();
        let ex = self.settings.exposure as f32;
        let mut out = Vec::with_capacity(acc.len() * 4);
        for p in acc.iter() {
            for c in 0..3 {
                let v = aces(p[c] / n * ex);
                out.push((v.powf(1.0 / 2.2) * 255.0 + 0.5).clamp(0.0, 255.0) as u8);
            }
            out.push(255);
        }
        out
    }
}

fn aces(x: f32) -> f32 {
    let (a, b, c, d, e) = (2.51, 0.03, 2.43, 0.59, 0.14);
    ((x * (a * x + b)) / (x * (c * x + d) + e)).clamp(0.0, 1.0)
}

struct Ctx<'a> {
    topo: &'a Topology,
    s: Settings,
    ground_y: f64,
    ground_r: f64,
    center: V3,
    key_dir: V3,
    key_cos: f64,
    key_l: V3,
    diag: f64,
}

/// Starts rendering `topo` as seen by `cam` on worker threads.
pub fn start(topo: Arc<Topology>, cam: Camera, s: Settings) -> Arc<Render> {
    let r = Arc::new(Render {
        accum: Mutex::new(vec![[0.0; 3]; s.width * s.height]),
        samples: AtomicU32::new(0),
        cancel: AtomicBool::new(false),
        settings: s,
        started: std::time::Instant::now(),
    });
    let r2 = r.clone();
    std::thread::spawn(move || {
        let b = topo.bbox;
        let diag = b.diagonal().max(1e-6);
        let (az, el) = (s.light_az.to_radians(), s.light_el.to_radians());
        let key_dir = V3::new(az.sin() * el.cos(), el.sin(), az.cos() * el.cos()).normalize();
        let key_half = if s.scene == Scene::Outdoor { 0.6f64 } else { 14.0f64 }.to_radians();
        let key_l = match s.scene {
            Scene::StudioLight => V3::splat(8.0),
            Scene::StudioDark => V3::splat(9.0),
            Scene::Outdoor => V3::new(1.0, 0.95, 0.85) * 300.0,
        };
        let ctx = Ctx { topo: &topo, s, ground_y: b.min.y - diag * 1e-4, ground_r: diag * 6.0, center: b.center(), key_dir, key_cos: key_half.cos(), key_l, diag };
        let threads = crate::sim::mesh::threads();
        let (w, h) = (s.width, s.height);
        for pass in 0..s.max_samples {
            if r2.cancel.load(Ordering::Relaxed) {
                return;
            }
            let rows_per = h.div_ceil(threads);
            let mut parts: Vec<Vec<[f32; 3]>> = vec![];
            std::thread::scope(|sc| {
                let hs: Vec<_> = (0..threads)
                    .map(|t| {
                        let ctx = &ctx;
                        let r2 = &r2;
                        sc.spawn(move || {
                            let mut buf = vec![];
                            for y in t * rows_per..((t + 1) * rows_per).min(h) {
                                if r2.cancel.load(Ordering::Relaxed) {
                                    break;
                                }
                                for x in 0..w {
                                    let mut rng = Rng::new(((pass as u64) << 40) ^ ((y * w + x) as u64 * 2654435761) ^ 0x5bd1e995);
                                    let px = (x as f32 + rng.f() as f32, y as f32 + rng.f() as f32);
                                    let (o, d) = cam.ray(px, (w as f32, h as f32));
                                    // a parallel view starts its rays well in front of the part
                                    let o = if cam.perspective { o } else { o - d * (ctx.diag * 2.0) };
                                    let c = trace(ctx, o, d, &mut rng);
                                    buf.push([c.x as f32, c.y as f32, c.z as f32]);
                                }
                            }
                            buf
                        })
                    })
                    .collect();
                for hd in hs {
                    parts.push(hd.join().unwrap_or_default());
                }
            });
            if r2.cancel.load(Ordering::Relaxed) {
                return;
            }
            let mut acc = r2.accum.lock().unwrap();
            let mut i = 0;
            for p in parts {
                for c in p {
                    if i < acc.len() {
                        let v = &mut acc[i];
                        // clamp fireflies
                        let m = c[0].max(c[1]).max(c[2]);
                        let k = if m > 40.0 { 40.0 / m } else { 1.0 };
                        v[0] += c[0] * k;
                        v[1] += c[1] * k;
                        v[2] += c[2] * k;
                    }
                    i += 1;
                }
            }
            drop(acc);
            r2.samples.fetch_add(1, Ordering::Relaxed);
        }
    });
    r
}

/// Light from the surroundings in direction `d` (without the key light).
fn sky(ctx: &Ctx, d: V3) -> V3 {
    let y = d.y;
    match ctx.s.scene {
        Scene::StudioLight => {
            // white cyclorama: bright above, soft fill boxes left and right
            let base = if y > 0.0 { V3::splat(0.2 + 0.16 * y) } else { V3::splat(0.08 + 0.12 * (1.0 + y)) };
            let fill = V3::new(-0.8, 0.35, 0.45).normalize();
            let f = (d.dot(fill) - 0.82).max(0.0) * 5.0;
            let rim = V3::new(0.2, 0.5, -0.85).normalize();
            let rr = (d.dot(rim) - 0.9).max(0.0) * 14.0;
            base + V3::splat(f + rr)
        }
        Scene::StudioDark => {
            let base = V3::splat(0.02 + 0.04 * y.max(0.0));
            let strip1 = V3::new(-0.9, 0.3, 0.3).normalize();
            let strip2 = V3::new(0.6, 0.45, -0.66).normalize();
            let s1 = if d.dot(strip1) > 0.93 { 6.0 } else { 0.0 };
            let s2 = if d.dot(strip2) > 0.95 { 4.0 } else { 0.0 };
            base + V3::splat(s1 + s2)
        }
        Scene::Outdoor => {
            if y >= 0.0 {
                let zen = V3::new(0.25, 0.45, 0.95);
                let hor = V3::new(0.85, 0.9, 1.0);
                (hor.lerp(zen, y.powf(0.5))) * 0.55
            } else {
                V3::new(0.35, 0.32, 0.28) * 0.5
            }
        }
    }
}

/// What a camera ray that hits nothing shows: a backdrop, not the lighting environment.
fn backdrop(ctx: &Ctx, d: V3) -> V3 {
    match ctx.s.scene {
        Scene::StudioLight => V3::splat(0.92 + 0.08 * d.y.max(0.0)),
        Scene::StudioDark => V3::splat(0.03 + 0.03 * d.y.max(-0.5)),
        Scene::Outdoor => sky(ctx, d),
    }
}

fn fresnel(f0: V3, cos: f64) -> V3 {
    f0 + (V3::ONE - f0) * (1.0 - cos.clamp(0.0, 1.0)).powi(5)
}

fn onb(n: V3) -> (V3, V3) {
    let a = if n.x.abs() > 0.9 { V3::Y } else { V3::X };
    let t = n.cross(a).normalize();
    (t, n.cross(t))
}

fn cosine_dir(n: V3, rng: &mut Rng) -> V3 {
    let (t, b) = onb(n);
    let (u1, u2) = (rng.f(), rng.f());
    let r = u1.sqrt();
    let phi = std::f64::consts::TAU * u2;
    (t * (r * phi.cos()) + b * (r * phi.sin()) + n * (1.0 - u1).max(0.0).sqrt()).normalize()
}

/// Phong lobe exponent for a roughness.
fn shininess(r: f64) -> f64 {
    (2.0 / (r * r).max(1e-4) - 2.0).max(1.0)
}

fn lobe_dir(axis: V3, e: f64, rng: &mut Rng) -> V3 {
    let (t, b) = onb(axis);
    let (u1, u2) = (rng.f(), rng.f());
    let ct = u1.powf(1.0 / (e + 1.0));
    let st = (1.0 - ct * ct).max(0.0).sqrt();
    let phi = std::f64::consts::TAU * u2;
    (t * (st * phi.cos()) + b * (st * phi.sin()) + axis * ct).normalize()
}

struct Hit {
    t: f64,
    n: V3,
    ground: bool,
}

fn intersect(ctx: &Ctx, o: V3, d: V3) -> Option<Hit> {
    let mut best: Option<Hit> = None;
    if let Some((t, pi, k, u, v)) = ctx.topo.ray_hit(o, d, ctx.diag * 1e-7) {
        let vn = &ctx.topo.vnormals[pi];
        let n = if vn.len() > k + 1 { safe_norm(vn[0] * (1.0 - u - v) + vn[k] * u + vn[k + 1] * v, ctx.topo.planes[pi].n) } else { ctx.topo.planes[pi].n };
        best = Some(Hit { t, n, ground: false });
    }
    if ctx.s.ground && d.y < -1e-9 {
        let t = (ctx.ground_y - o.y) / d.y;
        if t > ctx.diag * 1e-7 && best.as_ref().map_or(true, |b| t < b.t) {
            let p = o + d * t;
            if (V3::new(p.x, 0.0, p.z) - V3::new(ctx.center.x, 0.0, ctx.center.z)).length() < ctx.ground_r {
                best = Some(Hit { t, n: V3::Y, ground: true });
            }
        }
    }
    best
}

fn occluded(ctx: &Ctx, o: V3, d: V3) -> bool {
    ctx.topo.ray_hit(o, d, ctx.diag * 1e-6).is_some()
}

/// Direction toward the key light, sampled in its cone.
fn key_sample(ctx: &Ctx, rng: &mut Rng) -> V3 {
    let (t, b) = onb(ctx.key_dir);
    let ct = 1.0 - rng.f() * (1.0 - ctx.key_cos);
    let st = (1.0 - ct * ct).max(0.0).sqrt();
    let phi = std::f64::consts::TAU * rng.f();
    (t * (st * phi.cos()) + b * (st * phi.sin()) + ctx.key_dir * ct).normalize()
}

fn trace(ctx: &Ctx, mut o: V3, mut d: V3, rng: &mut Rng) -> V3 {
    let mut col = V3::ZERO;
    let mut thr = V3::ONE;
    let key_solid = std::f64::consts::TAU * (1.0 - ctx.key_cos);
    let mat = ctx.s.material;
    let base = V3::from_array(mat.color.unwrap_or(ctx.s.color));
    let mut specular_bounce = true;
    for depth in 0..ctx.s.bounces.max(1) {
        let Some(hit) = intersect(ctx, o, d) else {
            let env = if depth == 0 { backdrop(ctx, d) } else { sky(ctx, d) };
            col += thr * env;
            // the key light seen directly or in a mirror-like reflection
            if depth > 0 && specular_bounce && d.dot(ctx.key_dir) > ctx.key_cos {
                col += thr * ctx.key_l;
            }
            break;
        };
        let p = o + d * hit.t;
        let mut n = hit.n;
        if n.dot(d) > 0.0 {
            n = -n;
        }
        let eps = ctx.diag * 2e-5;
        let po = p + n * eps;
        if hit.ground {
            // shadow catcher: light grey floor that fades into the backdrop with distance
            let fade = ((p - ctx.center).length() / ctx.ground_r).clamp(0.0, 1.0);
            let albedo = match ctx.s.scene {
                Scene::StudioDark => V3::splat(0.04),
                Scene::Outdoor => V3::new(0.42, 0.40, 0.36),
                Scene::StudioLight => V3::splat(0.62),
            };
            if ctx.s.ground_mirror && rng.f() < 0.35 {
                let r = d - n * 2.0 * d.dot(n);
                thr *= fresnel(V3::splat(0.04), (-d).dot(n)).max(V3::splat(0.25)) * (1.0 - fade * 0.6) / 0.35 * 0.35;
                o = po;
                d = lobe_dir(r, shininess(0.02), rng);
                specular_bounce = true;
                continue;
            }
            // direct key light
            let l = key_sample(ctx, rng);
            if !occluded(ctx, po, l) {
                col += thr * albedo * ctx.key_l * (l.dot(n).max(0.0) * key_solid / std::f64::consts::PI);
            }
            if fade > 0.995 {
                col += thr * backdrop(ctx, d);
                break;
            }
            thr *= albedo;
            o = po;
            d = cosine_dir(n, rng);
            specular_bounce = false;
        } else {
            let wo = -d;
            let cos_o = wo.dot(n).max(1e-4);
            let rough = mat.roughness;
            let e = shininess(rough);
            let (f0, diffuse) = if mat.metal { (base, V3::ZERO) } else { (V3::splat(0.04), base) };
            let f = fresnel(f0, cos_o);
            let coat_f = if mat.coat { fresnel(V3::splat(0.05), cos_o).x } else { 0.0 };
            // direct key light (diffuse + glossy lobe)
            let l = key_sample(ctx, rng);
            let ndl = l.dot(n);
            if ndl > 0.0 && !occluded(ctx, po, l) {
                let r = n * 2.0 * wo.dot(n) - wo;
                let spec_lobe = (e + 2.0) / std::f64::consts::TAU * r.dot(l).max(0.0).powf(e);
                let coat_lobe = if mat.coat { (shininess(0.03) + 2.0) / std::f64::consts::TAU * r.dot(l).max(0.0).powf(shininess(0.03)) * coat_f } else { 0.0 };
                let brdf = diffuse * (V3::ONE - f) / std::f64::consts::PI + f * spec_lobe.min(200.0) + V3::splat(coat_lobe.min(400.0));
                col += thr * brdf * ctx.key_l * (ndl * key_solid);
            }
            // continue the path: coat, glossy or diffuse
            let p_coat = coat_f;
            let p_spec = if mat.metal { 1.0 } else { (f.x + f.y + f.z) / 3.0 };
            let u = rng.f();
            if u < p_coat {
                let r = d - n * 2.0 * d.dot(n);
                d = lobe_dir(r, shininess(0.03), rng);
                specular_bounce = true;
            } else if rng.f() < p_spec {
                let r = d - n * 2.0 * d.dot(n);
                d = lobe_dir(r, e, rng);
                if mat.metal {
                    thr *= f;
                } else {
                    thr *= f / p_spec.max(1e-3);
                }
                specular_bounce = rough < 0.25;
            } else {
                d = cosine_dir(n, rng);
                thr *= diffuse * (V3::ONE - f) / (1.0 - p_spec).max(1e-3);
                specular_bounce = false;
            }
            if d.dot(n) <= 0.0 {
                break;
            }
            o = po;
        }
        // Russian roulette
        if depth >= 2 {
            let q = thr.max_element().clamp(0.05, 0.95);
            if rng.f() > q {
                break;
            }
            thr /= q;
        }
    }
    col
}
