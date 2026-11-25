//! Renders every Assembler library part with its surface colours to PNG (for checking the
//! built-in models).  `cargo run --release --example libshots -- <out dir> [filter]`

use freo::kernel::math::*;
use freo::model::{library, regen};

fn main() {
    let mut args = std::env::args().skip(1);
    let out = args.next().unwrap_or_else(|| "libshots".into());
    let filter = args.next().unwrap_or_default().to_lowercase();
    std::fs::create_dir_all(&out).unwrap();
    for item in library::catalog() {
        if item == library::Item::Screw || !item.name().to_lowercase().contains(&filter) {
            continue;
        }
        let lp = item.build().unwrap();
        let r = regen::run(&lp.data);
        let name: String = item.name().chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect();
        for (k, view) in [V3::new(0.55, 1.0, 1.05), V3::new(-0.9, 0.55, -0.8)].into_iter().enumerate() {
            let img = shot(&lp.data, &r, 1000, 750, view);
            std::fs::write(format!("{out}/{name}_{k}.png"), freo::softrender::encode_png(&img)).unwrap();
        }
        println!("{name}: {} polys", r.topo.polys.len());
    }
}

fn shot(data: &freo::model::features::ModelData, r: &regen::RegenResult, w: usize, h: usize, view: V3) -> freo::softrender::Image {
    let ss = 2;
    let (sw, sh) = (w * ss, h * ss);
    let t = &r.topo;
    let view = safe_norm(view, V3::Z);
    let right = safe_norm(V3::Y.cross(view), V3::X);
    let up = view.cross(right);
    let b = t.bbox;
    let c = b.center();
    let scale = 0.92 * (sw.min(sh) as f64) / b.diagonal().max(1e-6);
    let proj = |p: V3| -> (f64, f64, f64) {
        let d = p - c;
        (sw as f64 / 2.0 + d.dot(right) * scale, sh as f64 / 2.0 - d.dot(up) * scale, d.dot(view))
    };
    let mut rgb = vec![[0f32; 3]; sw * sh];
    for y in 0..sh {
        let k = y as f32 / sh as f32;
        for x in 0..sw {
            rgb[y * sw + x] = [0.93 - 0.2 * k, 0.94 - 0.18 * k, 0.96 - 0.14 * k];
        }
    }
    let mut z = vec![f64::NEG_INFINITY; sw * sh];
    let key = safe_norm(view + up * 0.9 + right * 0.4, view);
    let fill = safe_norm(view - right * 0.8, view);
    for (pi, poly) in t.polys.iter().enumerate() {
        let col = r.tag_owner.get(&t.tags[pi]).and_then(|id| data.feature_colors.get(id)).copied().unwrap_or(data.appearance.color);
        let pts: Vec<(f64, f64, f64)> = poly.iter().map(|&i| proj(t.positions[i])).collect();
        let ns = &t.vnormals[pi];
        for k in 1..pts.len() - 1 {
            let tri = [(pts[0], ns[0]), (pts[k], ns[k]), (pts[k + 1], ns[k + 1])];
            raster(&mut rgb, &mut z, sw, sh, tri, |n: V3| {
                let n = if n.dot(view) < 0.0 { -n } else { n };
                let diff = 0.62 * n.dot(key).max(0.0) + 0.22 * n.dot(fill).max(0.0) + 0.24;
                let hv = safe_norm(key + view, view);
                let spec = 0.35 * n.dot(hv).max(0.0).powi(40) as f32;
                [(col[0] * diff as f32 + spec).min(1.0), (col[1] * diff as f32 + spec).min(1.0), (col[2] * diff as f32 + spec).min(1.0)]
            });
        }
    }
    let mut img = freo::softrender::Image { w, h, rgb: vec![[0; 3]; w * h] };
    for y in 0..h {
        for x in 0..w {
            let mut a = [0f32; 3];
            for dy in 0..ss {
                for dx in 0..ss {
                    let p = rgb[(y * ss + dy) * sw + x * ss + dx];
                    for i in 0..3 {
                        a[i] += p[i];
                    }
                }
            }
            let n = (ss * ss) as f32;
            img.rgb[y * w + x] = [(a[0] / n * 255.0) as u8, (a[1] / n * 255.0) as u8, (a[2] / n * 255.0) as u8];
        }
    }
    img
}

fn raster(img: &mut [[f32; 3]], z: &mut [f64], w: usize, h: usize, t: [((f64, f64, f64), V3); 3], shade: impl Fn(V3) -> [f32; 3]) {
    let (a, b, c) = (t[0].0, t[1].0, t[2].0);
    let minx = a.0.min(b.0).min(c.0).floor().max(0.0) as usize;
    let maxx = (a.0.max(b.0).max(c.0).ceil().max(0.0) as usize).min(w - 1);
    let miny = a.1.min(b.1).min(c.1).floor().max(0.0) as usize;
    let maxy = (a.1.max(b.1).max(c.1).ceil().max(0.0) as usize).min(h - 1);
    let area = (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
    if area.abs() < 1e-12 {
        return;
    }
    for y in miny..=maxy {
        for x in minx..=maxx {
            let (px, py) = (x as f64 + 0.5, y as f64 + 0.5);
            let w0 = ((b.0 - px) * (c.1 - py) - (b.1 - py) * (c.0 - px)) / area;
            let w1 = ((c.0 - px) * (a.1 - py) - (c.1 - py) * (a.0 - px)) / area;
            let w2 = 1.0 - w0 - w1;
            if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                continue;
            }
            let d = w0 * a.2 + w1 * b.2 + w2 * c.2;
            let i = y * w + x;
            if d > z[i] {
                z[i] = d;
                img[i] = shade(safe_norm(t[0].1 * w0 + t[1].1 * w1 + t[2].1 * w2, V3::Y));
            }
        }
    }
}
