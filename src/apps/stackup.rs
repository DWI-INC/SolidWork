//! EZ Tolerance Analysis: one-dimensional tolerance stack-ups. Worst case, statistical (RSS)
//! and Monte Carlo results of the closing dimension, each contributor's share of the
//! variation, process capability against the requirement and the expected reject rate.

use super::Rng;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Dist {
    /// normal, the tolerance band covers ±3σ
    Normal,
    Uniform,
    Triangular,
}

impl Dist {
    pub const ALL: [Dist; 3] = [Dist::Normal, Dist::Uniform, Dist::Triangular];
    pub fn name(self) -> &'static str {
        match self {
            Dist::Normal => "Normal (±3σ)",
            Dist::Uniform => "Gleichverteilt",
            Dist::Triangular => "Dreieck",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Contributor {
    pub name: String,
    pub nominal: f64,
    /// signed deviations: upper ≥ lower
    pub upper: f64,
    pub lower: f64,
    /// +1 increases the closing dimension, −1 decreases it
    pub dir: i8,
    pub dist: Dist,
}

impl Contributor {
    pub fn mean(&self) -> f64 {
        self.nominal + (self.upper + self.lower) * 0.5
    }
    pub fn half(&self) -> f64 {
        (self.upper - self.lower).abs() * 0.5
    }
    pub fn sigma(&self) -> f64 {
        match self.dist {
            Dist::Normal => self.half() / 3.0,
            Dist::Uniform => self.half() / 3f64.sqrt(),
            Dist::Triangular => self.half() / 6f64.sqrt(),
        }
    }
    fn sample(&self, r: &mut Rng) -> f64 {
        let h = self.half();
        let m = self.mean();
        m + match self.dist {
            Dist::Normal => r.normal() * h / 3.0,
            Dist::Uniform => (r.f() * 2.0 - 1.0) * h,
            Dist::Triangular => (r.f() + r.f() - 1.0) * h,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Stackup {
    pub name: String,
    pub items: Vec<Contributor>,
    /// requirement on the closing dimension
    pub lsl: Option<f64>,
    pub usl: Option<f64>,
}

impl Default for Stackup {
    fn default() -> Self {
        Stackup { name: "Maßkette 1".into(), items: vec![], lsl: None, usl: None }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Result {
    pub nominal: f64,
    pub wc: (f64, f64),
    pub mean: f64,
    pub sigma: f64,
    pub rss: (f64, f64),
    pub mc_mean: f64,
    pub mc_sigma: f64,
    pub mc_range: (f64, f64),
    pub hist: Vec<u32>,
    pub hist_range: (f64, f64),
    /// share of the variance per contributor (sums to 1)
    pub share: Vec<f64>,
    /// sensitivity to the worst case: share of the total tolerance width
    pub wc_share: Vec<f64>,
    pub cp: Option<f64>,
    pub cpk: Option<f64>,
    /// expected parts outside the limits per million: normal estimate and Monte Carlo
    pub ppm: Option<f64>,
    pub mc_ppm: Option<f64>,
    pub samples: usize,
}

fn phi(z: f64) -> f64 {
    // standard normal CDF (Abramowitz–Stegun 7.1.26 via erf)
    let x = z / 2f64.sqrt();
    let t = 1.0 / (1.0 + 0.3275911 * x.abs());
    let y = 1.0 - (((((1.061405429 * t - 1.453152027) * t) + 1.421413741) * t - 0.284496736) * t + 0.254829592) * t * (-x * x).exp();
    0.5 * (1.0 + if x >= 0.0 { y } else { -y })
}

pub fn analyze(s: &Stackup, samples: usize, seed: u64) -> Result {
    let mut r = Result { samples, ..Default::default() };
    if s.items.is_empty() {
        return r;
    }
    let sg = |c: &Contributor| c.dir.signum() as f64;
    r.nominal = s.items.iter().map(|c| sg(c) * c.nominal).sum();
    let (mut lo, mut hi) = (0.0, 0.0);
    for c in &s.items {
        let (a, b) = (c.nominal + c.lower, c.nominal + c.upper);
        if sg(c) > 0.0 {
            lo += a;
            hi += b;
        } else {
            lo -= b;
            hi -= a;
        }
    }
    r.wc = (lo, hi);
    r.mean = s.items.iter().map(|c| sg(c) * c.mean()).sum();
    let var: f64 = s.items.iter().map(|c| c.sigma().powi(2)).sum();
    r.sigma = var.sqrt();
    r.rss = (r.mean - 3.0 * r.sigma, r.mean + 3.0 * r.sigma);
    r.share = s.items.iter().map(|c| if var > 0.0 { c.sigma().powi(2) / var } else { 0.0 }).collect();
    let wsum: f64 = s.items.iter().map(|c| c.half()).sum();
    r.wc_share = s.items.iter().map(|c| if wsum > 0.0 { c.half() / wsum } else { 0.0 }).collect();
    // Monte Carlo
    let mut rng = Rng::new(seed);
    let mut vals = Vec::with_capacity(samples);
    for _ in 0..samples {
        vals.push(s.items.iter().map(|c| sg(c) * c.sample(&mut rng)).sum::<f64>());
    }
    let n = vals.len().max(1) as f64;
    r.mc_mean = vals.iter().sum::<f64>() / n;
    r.mc_sigma = (vals.iter().map(|v| (v - r.mc_mean).powi(2)).sum::<f64>() / (n - 1.0).max(1.0)).sqrt();
    r.mc_range = (vals.iter().cloned().fold(f64::INFINITY, f64::min), vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max));
    let (h0, h1) = (r.wc.0.min(r.mc_range.0), r.wc.1.max(r.mc_range.1));
    let span = (h1 - h0).max(1e-12);
    let bins = 60;
    r.hist = vec![0; bins];
    for v in &vals {
        let k = (((v - h0) / span) * bins as f64).floor().clamp(0.0, bins as f64 - 1.0) as usize;
        r.hist[k] += 1;
    }
    r.hist_range = (h0, h1);
    if s.lsl.is_some() || s.usl.is_some() {
        let sd = r.sigma.max(1e-15);
        let mut out = 0.0;
        if let (Some(l), Some(u)) = (s.lsl, s.usl) {
            r.cp = Some((u - l) / (6.0 * sd));
        }
        let mut cpk = f64::INFINITY;
        if let Some(l) = s.lsl {
            cpk = cpk.min((r.mean - l) / (3.0 * sd));
            out += phi((l - r.mean) / sd);
        }
        if let Some(u) = s.usl {
            cpk = cpk.min((u - r.mean) / (3.0 * sd));
            out += 1.0 - phi((u - r.mean) / sd);
        }
        r.cpk = Some(cpk);
        r.ppm = Some(out * 1e6);
        let bad = vals.iter().filter(|v| s.lsl.is_some_and(|l| **v < l) || s.usl.is_some_and(|u| **v > u)).count();
        r.mc_ppm = Some(bad as f64 / n * 1e6);
    }
    r
}

/// ISO 2768-1 general tolerances for linear sizes: ± deviation for class f, m, c, v.
pub fn iso2768(size: f64, class: usize) -> f64 {
    const LIM: [f64; 8] = [3.0, 6.0, 30.0, 120.0, 400.0, 1000.0, 2000.0, 4000.0];
    const T: [[f64; 8]; 4] =
        [[0.05, 0.05, 0.1, 0.15, 0.2, 0.3, 0.5, 0.5], [0.1, 0.1, 0.2, 0.3, 0.5, 0.8, 1.2, 2.0], [0.2, 0.3, 0.5, 0.8, 1.2, 2.0, 3.0, 4.0], [0.5, 0.5, 1.0, 1.5, 2.5, 4.0, 6.0, 8.0]];
    let i = LIM.iter().position(|l| size.abs() <= *l).unwrap_or(7);
    T[class.min(3)][i]
}
