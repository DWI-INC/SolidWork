//! Engines behind the Anwendungen tab: weld seams, mould and die-casting design, mould bases,
//! filling simulation, generative design (topology optimisation), convergence studies,
//! worksheets, GD&T proposals, tolerance stack-ups and the photo-realistic renderer.
//! Everything here is free of UI code so tests and the MCP server can use it too.

pub mod gdt;
pub mod mold;
pub mod moldflow;
pub mod raytrace;
pub mod stackup;
pub mod topopt;
pub mod weld;
pub mod worksheet;

use serde::{Deserialize, Serialize};

/// What the applications store with the part.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, Default)]
pub struct AppsData {
    #[serde(default)]
    pub welds: Vec<weld::WeldSeam>,
    /// Mathcad-style worksheet embedded in the part
    #[serde(default)]
    pub worksheet: String,
    #[serde(default)]
    pub stackups: Vec<stackup::Stackup>,
}

/// Numbers for reports: German decimal comma, sensible digits.
pub fn num(v: f64) -> String {
    if !v.is_finite() {
        return "–".into();
    }
    let a = v.abs();
    let s = if a == 0.0 {
        "0".to_string()
    } else if a >= 1e6 || a < 1e-4 {
        format!("{:.3e}", v)
    } else if a >= 1000.0 {
        format!("{:.0}", v)
    } else if a >= 100.0 {
        format!("{:.1}", v)
    } else if a >= 10.0 {
        format!("{:.2}", v)
    } else if a >= 1.0 {
        format!("{:.3}", v)
    } else {
        format!("{:.4}", v)
    };
    s.replace('.', ",")
}

/// Small fast random numbers (xorshift64*), reproducible from a seed.
#[derive(Clone)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /// uniform in [0, 1)
    pub fn f(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
    /// standard normal (Box–Muller)
    pub fn normal(&mut self) -> f64 {
        let u = self.f().max(1e-300);
        let v = self.f();
        (-2.0 * u.ln()).sqrt() * (std::f64::consts::TAU * v).cos()
    }
}
