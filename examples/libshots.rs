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
            let img = freo::softrender::render_part(&lp.data, &r, 1000, 750, view);
            std::fs::write(format!("{out}/{name}_{k}.png"), freo::softrender::encode_png(&img)).unwrap();
        }
        println!("{name}: {} polys", r.topo.polys.len());
    }
}

