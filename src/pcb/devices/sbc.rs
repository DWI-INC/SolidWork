//! Single-board computers of the Assembler library: every Raspberry Pi (boards, Zeros,
//! Picos, Compute Modules and keyboard computers) and the Orange Pi range.
//!
//! The boards are described as data — outline, mounting holes, GPIO header, edge
//! connectors, chips, ribbon connectors, LEDs — and built by one generator, so each looks
//! like the real board at real scale. Raspberry Pi headers carry the J8 signal names; Orange
//! Pi headers carry the power pins at the standard positions and the physical pin number for
//! the signals (`PIN7`), since their GPIO banks differ per SoC. Orange Pi layouts follow the
//! manufacturer's port lists; exact connector offsets are approximations.

use super::*;
use crate::pcb::model3d::rounded_rect;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Sbc {
    // Raspberry Pi boards
    Pi1A,
    Pi1B,
    Pi1APlus,
    Pi1BPlus,
    Pi2B,
    Pi3B,
    Pi3BPlus,
    Pi3APlus,
    Pi5,
    // Zero
    PiZero,
    PiZeroW,
    PiZeroWH,
    PiZero2W,
    // Pico (the plain Pico is BoardKind::PiPico)
    PicoH,
    PicoW,
    PicoWH,
    Pico2,
    Pico2W,
    // Compute Modules
    Cm1,
    Cm3,
    Cm3Plus,
    Cm4S,
    Cm4,
    Cm5,
    // keyboard computers
    Pi400,
    Pi500,
    // Orange Pi
    OpiZero,
    OpiZeroLts,
    OpiZeroPlus,
    OpiZeroPlus2,
    OpiZero2,
    OpiZero2W,
    OpiZero3,
    OpiOne,
    OpiOnePlus,
    OpiLite,
    OpiLite2,
    OpiPc,
    OpiPcPlus,
    OpiPc2,
    OpiPlus,
    OpiPlus2,
    OpiPlus2E,
    OpiPrime,
    OpiWin,
    OpiWinPlus,
    Opi3,
    Opi3Lts,
    Opi3B,
    Opi4,
    Opi4Lts,
    Opi4A,
    Opi5,
    Opi5B,
    Opi5Plus,
    Opi5Pro,
    Opi5Max,
    Opi5Ultra,
    Opi800,
    OpiR1,
    OpiR1Plus,
    OpiR1PlusLts,
    OpiRv,
    OpiRv2,
    OpiAiPro,
    OpiCm4,
    OpiCm5,
    OpiI96,
    Opi2gIot,
    Opi4gIot,
}

use Sbc::*;

/// (board, name, description)
const TABLE: &[(Sbc, &str, &str)] = &[
    (Pi1A, "Raspberry Pi 1 Model A", "BCM2835, 256 MB, 26-polige Leiste, 1 × USB, HDMI, Composite, 85,6 × 56,5 mm"),
    (Pi1B, "Raspberry Pi 1 Model B", "BCM2835, 512 MB, 26-polige Leiste, 2 × USB, Ethernet, HDMI, Composite, 85,6 × 56,5 mm"),
    (Pi1APlus, "Raspberry Pi 1 Model A+", "BCM2835, 512 MB, 40-polige Leiste, 1 × USB, HDMI, 65 × 56 mm"),
    (Pi1BPlus, "Raspberry Pi 1 Model B+", "BCM2835, 512 MB, 40-polige Leiste, 4 × USB, Ethernet, HDMI, 85 × 56 mm"),
    (Pi2B, "Raspberry Pi 2 Model B", "BCM2836, 1 GB, 40-polige Leiste, 4 × USB, Ethernet, HDMI, 85 × 56 mm"),
    (Pi3B, "Raspberry Pi 3 Model B", "BCM2837, 1 GB, WLAN/Bluetooth, 4 × USB, Ethernet, HDMI, 85 × 56 mm"),
    (Pi3BPlus, "Raspberry Pi 3 Model B+", "BCM2837B0, 1 GB, Dualband-WLAN, Gigabit über USB, PoE-Header, 85 × 56 mm"),
    (Pi3APlus, "Raspberry Pi 3 Model A+", "BCM2837B0, 512 MB, Dualband-WLAN, 1 × USB, HDMI, 65 × 56 mm"),
    (Pi5, "Raspberry Pi 5", "BCM2712 + RP1, bis 16 GB, 2 × USB 3, 2 × USB 2, Gigabit, 2 × Micro-HDMI, PCIe, 85 × 56 mm"),
    (PiZero, "Raspberry Pi Zero", "BCM2835, 512 MB, Mini-HDMI, 2 × Micro-USB, Leiste unbestückt, 65 × 30 mm"),
    (PiZeroW, "Raspberry Pi Zero W", "BCM2835, 512 MB, WLAN/Bluetooth, Leiste unbestückt, 65 × 30 mm"),
    (PiZeroWH, "Raspberry Pi Zero WH", "Zero W mit aufgelöteter 40-poliger Stiftleiste, 65 × 30 mm"),
    (PiZero2W, "Raspberry Pi Zero 2 W", "RP3A0 (4 × Cortex-A53), 512 MB, WLAN/Bluetooth, 65 × 30 mm"),
    (PicoH, "Raspberry Pi Pico H", "RP2040, Stiftleisten bestückt, 3-poliger Debug-Stecker, LED an GP25"),
    (PicoW, "Raspberry Pi Pico W", "RP2040 mit CYW43439 (WLAN/Bluetooth), LED an WL_GPIO0"),
    (PicoWH, "Raspberry Pi Pico WH", "Pico W mit Stiftleisten und Debug-Stecker"),
    (Pico2, "Raspberry Pi Pico 2", "RP2350, 4 MB Flash, LED an GP25, 21 × 51 mm"),
    (Pico2W, "Raspberry Pi Pico 2 W", "RP2350 mit CYW43439 (WLAN/Bluetooth), LED an WL_GPIO0"),
    (Cm1, "Raspberry Pi Compute Module 1", "BCM2835, 512 MB, 4 GB eMMC, SO-DIMM-Steckkarte 67,6 × 30 mm"),
    (Cm3, "Raspberry Pi Compute Module 3", "BCM2837, 1 GB, 4 GB eMMC, SO-DIMM-Steckkarte 67,6 × 31 mm"),
    (Cm3Plus, "Raspberry Pi Compute Module 3+", "BCM2837B0, 1 GB, bis 32 GB eMMC, SO-DIMM 67,6 × 31 mm"),
    (Cm4S, "Raspberry Pi Compute Module 4S", "BCM2711, 1 GB, SO-DIMM-Format wie CM3, 67,6 × 31 mm"),
    (Cm4, "Raspberry Pi Compute Module 4", "BCM2711, bis 8 GB, WLAN, 2 × 100-polige Hirose-Stecker, 55 × 40 mm"),
    (Cm5, "Raspberry Pi Compute Module 5", "BCM2712, bis 16 GB, WLAN, 2 × 100-polige Stecker, 55 × 40 mm"),
    (Pi400, "Raspberry Pi 400", "Pi 4 in einer Tastatur, 40-polige Leiste hinten, 286 × 122 mm"),
    (Pi500, "Raspberry Pi 500", "Pi 5 in einer Tastatur, 40-polige Leiste hinten, 286 × 122 mm"),
    (OpiZero, "Orange Pi Zero", "Allwinner H2+, 256/512 MB, 26-polige Leiste, Ethernet, USB, WLAN, 48 × 46 mm"),
    (OpiZeroLts, "Orange Pi Zero LTS", "Allwinner H2+, 512 MB, 26-polige Leiste, Ethernet, USB, WLAN, 48 × 46 mm"),
    (OpiZeroPlus, "Orange Pi Zero Plus", "Allwinner H5, 512 MB, Gigabit-Ethernet, USB, WLAN, 48 × 46 mm"),
    (OpiZeroPlus2, "Orange Pi Zero Plus 2", "Allwinner H3/H5, 512 MB, 8 GB eMMC, Mini-HDMI, WLAN, 46 × 48 mm"),
    (OpiZero2, "Orange Pi Zero 2", "Allwinner H616, 512 MB/1 GB, Gigabit, USB, Micro-HDMI, USB-C, 60 × 53 mm"),
    (OpiZero2W, "Orange Pi Zero 2W", "Allwinner H618, bis 4 GB, Mini-HDMI, 2 × USB-C, 40-polige Leiste, 65 × 30 mm"),
    (OpiZero3, "Orange Pi Zero 3", "Allwinner H618, bis 4 GB, Gigabit, USB, Micro-HDMI, USB-C, 50 × 55 mm"),
    (OpiOne, "Orange Pi One", "Allwinner H3, 512 MB, Ethernet, 1 × USB, HDMI, 69 × 48 mm"),
    (OpiOnePlus, "Orange Pi One Plus", "Allwinner H6, 1 GB, Gigabit, 1 × USB, HDMI, 69 × 48 mm"),
    (OpiLite, "Orange Pi Lite", "Allwinner H3, 512 MB, WLAN, 2 × USB, HDMI, 69 × 48 mm"),
    (OpiLite2, "Orange Pi Lite 2", "Allwinner H6, 1 GB, WLAN, USB 3.0, HDMI, 69 × 48 mm"),
    (OpiPc, "Orange Pi PC", "Allwinner H3, 1 GB, Ethernet, 3 × USB, HDMI, 85 × 55 mm"),
    (OpiPcPlus, "Orange Pi PC Plus", "Allwinner H3, 1 GB, 8 GB eMMC, WLAN, Ethernet, 3 × USB, 85 × 55 mm"),
    (OpiPc2, "Orange Pi PC 2", "Allwinner H5, 1 GB, Gigabit, 3 × USB, HDMI, 85 × 55 mm"),
    (OpiPlus, "Orange Pi Plus", "Allwinner H3, 1 GB, Gigabit, 4 × USB, SATA, WLAN, 108 × 67 mm"),
    (OpiPlus2, "Orange Pi Plus 2", "Allwinner H3, 2 GB, 16 GB eMMC, Gigabit, 4 × USB, SATA, 108 × 67 mm"),
    (OpiPlus2E, "Orange Pi Plus 2E", "Allwinner H3, 2 GB, 16 GB eMMC, Gigabit, 3 × USB, WLAN, 85 × 55 mm"),
    (OpiPrime, "Orange Pi Prime", "Allwinner H5, 2 GB, Gigabit, 3 × USB, WLAN, 98 × 60 mm"),
    (OpiWin, "Orange Pi Win", "Allwinner A64, 1 GB, Gigabit, 4 × USB, WLAN, 93 × 60 mm"),
    (OpiWinPlus, "Orange Pi Win Plus", "Allwinner A64, 2 GB, Gigabit, 4 × USB, WLAN, 93 × 60 mm"),
    (Opi3, "Orange Pi 3", "Allwinner H6, 1/2 GB, Gigabit, 4 × USB 3.0, HDMI, 90 × 64 mm"),
    (Opi3Lts, "Orange Pi 3 LTS", "Allwinner H6, 2 GB, 8 GB eMMC, Gigabit, WLAN, 85 × 56 mm"),
    (Opi3B, "Orange Pi 3B", "Rockchip RK3566, bis 8 GB, Gigabit, WLAN, M.2, 85 × 56 mm"),
    (Opi4, "Orange Pi 4", "Rockchip RK3399, 4 GB, Gigabit, USB 3.0, HDMI, 91 × 56 mm"),
    (Opi4Lts, "Orange Pi 4 LTS", "Rockchip RK3399, 3/4 GB, Gigabit, WLAN, 91 × 56 mm"),
    (Opi4A, "Orange Pi 4A", "Allwinner T527, bis 4 GB, Gigabit, NPU, M.2, 85 × 56 mm"),
    (Opi5, "Orange Pi 5", "Rockchip RK3588S, bis 32 GB, Gigabit, 2 × USB-C, HDMI, 26-polige Leiste, 100 × 62 mm"),
    (Opi5B, "Orange Pi 5B", "Rockchip RK3588S, eMMC, WLAN 6, Gigabit, HDMI, 100 × 62 mm"),
    (Opi5Plus, "Orange Pi 5 Plus", "Rockchip RK3588, bis 32 GB, 2 × 2,5-Gbit, 2 × HDMI-Aus, HDMI-Ein, 100 × 75 mm"),
    (Opi5Pro, "Orange Pi 5 Pro", "Rockchip RK3588S, bis 16 GB, Gigabit, 2 × HDMI, WLAN 5, 89 × 57 mm"),
    (Opi5Max, "Orange Pi 5 Max", "Rockchip RK3588, bis 16 GB, 2,5-Gbit, 2 × HDMI, WLAN 6E, 89 × 57 mm"),
    (Opi5Ultra, "Orange Pi 5 Ultra", "Rockchip RK3588, bis 16 GB, 2,5-Gbit, HDMI-Ein und -Aus, 89 × 57 mm"),
    (Opi800, "Orange Pi 800", "Rockchip RK3399 in einer Tastatur, 26-polige Leiste hinten"),
    (OpiR1, "Orange Pi R1", "Allwinner H2+, 2 × Ethernet, WLAN, 13-polige Leiste, 60 × 46 mm"),
    (OpiR1Plus, "Orange Pi R1 Plus", "Rockchip RK3328, 2 × Gigabit, USB, 56 × 57 mm"),
    (OpiR1PlusLts, "Orange Pi R1 Plus LTS", "Rockchip RK3328, 2 × Gigabit, USB, 56 × 57 mm"),
    (OpiRv, "Orange Pi RV", "StarFive JH7110 (RISC-V), bis 8 GB, 2 × Gigabit, 4 × USB, 89 × 56 mm"),
    (OpiRv2, "Orange Pi RV2", "Ky X1 (RISC-V), bis 8 GB, 2 × Gigabit, M.2, 89 × 56 mm"),
    (OpiAiPro, "Orange Pi AIpro", "Ascend 310B (NPU), 8/16 GB, 2 × HDMI, Gigabit, M.2, 107 × 68 mm"),
    (OpiCm4, "Orange Pi CM4", "Rockchip RK3566, Compute-Module-Format, 55 × 40 mm"),
    (OpiCm5, "Orange Pi CM5", "Rockchip RK3588S, Compute-Module-Format, 55 × 40 mm"),
    (OpiI96, "Orange Pi i96", "RDA8810, 256 MB, 96Boards-IoT-Format, WLAN, 85 × 30 mm"),
    (Opi2gIot, "Orange Pi 2G-IoT", "RDA8810, 256 MB, GSM-Modem, SIM, WLAN, 67 × 42 mm"),
    (Opi4gIot, "Orange Pi 4G-IoT", "MediaTek MT6737, 1 GB, LTE-Modem, HDMI, 80 × 58 mm"),
];

impl Sbc {
    pub const ALL: [Sbc; 70] = [
        Pi1A, Pi1B, Pi1APlus, Pi1BPlus, Pi2B, Pi3B, Pi3BPlus, Pi3APlus, Pi5, PiZero, PiZeroW, PiZeroWH, PiZero2W, PicoH, PicoW, PicoWH, Pico2, Pico2W, Cm1, Cm3, Cm3Plus, Cm4S, Cm4, Cm5,
        Pi400, Pi500, OpiZero, OpiZeroLts, OpiZeroPlus, OpiZeroPlus2, OpiZero2, OpiZero2W, OpiZero3, OpiOne, OpiOnePlus, OpiLite, OpiLite2, OpiPc, OpiPcPlus, OpiPc2, OpiPlus, OpiPlus2,
        OpiPlus2E, OpiPrime, OpiWin, OpiWinPlus, Opi3, Opi3Lts, Opi3B, Opi4, Opi4Lts, Opi4A, Opi5, Opi5B, Opi5Plus, Opi5Pro, Opi5Max, Opi5Ultra, Opi800, OpiR1, OpiR1Plus, OpiR1PlusLts,
        OpiRv, OpiRv2, OpiAiPro, OpiCm4, OpiCm5, OpiI96, Opi2gIot, Opi4gIot,
    ];
    fn row(self) -> &'static (Sbc, &'static str, &'static str) {
        TABLE.iter().find(|r| r.0 == self).expect("table row")
    }
    pub fn name(self) -> &'static str {
        self.row().1
    }
    pub fn description(self) -> &'static str {
        self.row().2
    }
    pub fn orange(self) -> bool {
        self.name().starts_with("Orange")
    }
    pub fn pico(self) -> bool {
        matches!(self, PicoH | PicoW | PicoWH | Pico2 | Pico2W)
    }
    pub fn wireless_pico(self) -> bool {
        matches!(self, PicoW | PicoWH | Pico2W)
    }
    pub fn compute_module(self) -> bool {
        matches!(self, Cm1 | Cm3 | Cm3Plus | Cm4S | Cm4 | Cm5 | OpiCm4 | OpiCm5)
    }
    /// The lamp lit while the board runs.
    pub fn power_lamp(self) -> Option<&'static str> {
        if self.compute_module() {
            None
        } else if matches!(self, PiZero | PiZeroW | PiZeroWH | PiZero2W) {
            Some("ACT")
        } else {
            Some("PWR")
        }
    }
}

// ------------------------------------------------------------------ layout description

#[derive(Clone, Copy, PartialEq)]
enum Edge {
    Top,
    Bottom,
    Left,
    Right,
}
use Edge::{Bottom as B, Left as L, Right as R, Top as T};

#[derive(Clone, Copy, PartialEq)]
enum Port {
    UsbA(Mat),
    /// stacked pair, tongue colour
    UsbA2(Mat),
    MicroB,
    TypeC,
    Hdmi,
    MiniHdmi,
    MicroHdmi,
    Rj45,
    Audio,
    Rca,
    Dc,
}

const USB3: Mat = Mat::Rgb(30, 95, 210);
const USB2: Mat = Mat::Plastic;

#[derive(Clone, Copy)]
enum Style {
    /// moulded package with marking
    Plastic,
    /// BGA with a metal heat spreader
    Lid,
    /// shielding can
    Can,
}

#[derive(Clone, Copy)]
struct Chip {
    mark: &'static str,
    c: (f64, f64),
    size: (f64, f64),
    style: Style,
}

fn plastic(mark: &'static str, x: f64, y: f64, w: f64, h: f64) -> Chip {
    Chip { mark, c: (x, y), size: (w, h), style: Style::Plastic }
}
fn lid(mark: &'static str, x: f64, y: f64, w: f64, h: f64) -> Chip {
    Chip { mark, c: (x, y), size: (w, h), style: Style::Lid }
}
fn can(x: f64, y: f64, w: f64, h: f64) -> Chip {
    Chip { mark: "", c: (x, y), size: (w, h), style: Style::Can }
}

#[derive(Clone, Copy)]
struct Header {
    /// centre of the header
    c: (f64, f64),
    pins: usize,
    rows: usize,
    /// degrees; 0 = along x with pin 1 in the row of larger y
    rot: f64,
    populated: bool,
}

struct Layout {
    w: f64,
    h: f64,
    r: f64,
    t: f64,
    mask: [f32; 3],
    silk: Mat,
    mounts: Vec<(f64, f64)>,
    hole: f64,
    header: Option<Header>,
    ports: Vec<(Port, Edge, f64)>,
    chips: Vec<Chip>,
    /// ribbon connectors: centre, length, along x, tall (camera style) or low
    fpcs: Vec<((f64, f64), f64, bool, bool)>,
    leds: Vec<(&'static str, (f64, f64), LedColor)>,
    labels: Vec<(&'static str, (f64, f64), f64)>,
    buttons: Vec<(f64, f64)>,
    sd: Option<(Edge, f64)>,
    poe: Option<(f64, f64)>,
    /// small JST connectors: position, pins, along x
    jst: Vec<((f64, f64), usize, bool)>,
    /// SATA data connector
    sata: Option<(f64, f64)>,
}

const PI_GREEN: [f32; 3] = [0.09, 0.40, 0.20];
const OPI_BLUE: [f32; 3] = [0.06, 0.20, 0.50];
const OPI_BLACK: [f32; 3] = [0.07, 0.07, 0.08];
const OPI_GREEN: [f32; 3] = [0.07, 0.33, 0.18];
const PI_MOUNTS: [(f64, f64); 4] = [(3.5, 3.5), (61.5, 3.5), (3.5, 52.5), (61.5, 52.5)];

impl Layout {
    fn new(w: f64, h: f64, mask: [f32; 3]) -> Layout {
        Layout {
            w,
            h,
            r: 3.0,
            t: 1.4,
            mask,
            silk: SILK,
            mounts: vec![],
            hole: 2.75,
            header: None,
            ports: vec![],
            chips: vec![],
            fpcs: vec![],
            leds: vec![],
            labels: vec![],
            buttons: vec![],
            sd: None,
            poe: None,
            jst: vec![],
            sata: None,
        }
    }
    /// 2 × n header along the top edge centred at x.
    fn top_header(mut self, x: f64, pins: usize, populated: bool) -> Self {
        self.header = Some(Header { c: (x, 3.5), pins, rows: 2, rot: 0.0, populated });
        self
    }
    fn ports(mut self, p: &[(Port, Edge, f64)]) -> Self {
        self.ports.extend_from_slice(p);
        self
    }
    fn chips(mut self, c: &[Chip]) -> Self {
        self.chips.extend_from_slice(c);
        self
    }
    fn mounts(mut self, m: &[(f64, f64)]) -> Self {
        self.mounts = m.to_vec();
        self
    }
    fn corners(mut self, inset: f64) -> Self {
        let (w, h) = (self.w, self.h);
        self.mounts = vec![(inset, inset), (w - inset, inset), (inset, h - inset), (w - inset, h - inset)];
        self
    }
    fn leds(mut self, l: &[(&'static str, (f64, f64), LedColor)]) -> Self {
        self.leds.extend_from_slice(l);
        self
    }
    fn label(mut self, s: &'static str, at: (f64, f64), size: f64) -> Self {
        self.labels.push((s, at, size));
        self
    }
    fn fpc(mut self, c: (f64, f64), len: f64, along_x: bool, tall: bool) -> Self {
        self.fpcs.push((c, len, along_x, tall));
        self
    }
    fn sd(mut self, e: Edge, pos: f64) -> Self {
        self.sd = Some((e, pos));
        self
    }
    fn button(mut self, x: f64, y: f64) -> Self {
        self.buttons.push((x, y));
        self
    }
    fn jst(mut self, at: (f64, f64), pins: usize, along_x: bool) -> Self {
        self.jst.push((at, pins, along_x));
        self
    }
    fn pwr_act(self, x: f64, y: f64) -> Self {
        self.leds(&[("PWR", (x, y), LedColor::Red), ("ACT", (x, y + 2.6), LedColor::Green)])
    }
}

// ------------------------------------------------------------------ the boards

fn b_plus(name: &'static str, soc: Chip, lan: &'static str) -> Layout {
    Layout::new(85.0, 56.0, PI_GREEN)
        .mounts(&PI_MOUNTS)
        .top_header(32.5, 40, true)
        .ports(&[(Port::UsbA2(USB2), R, 9.0), (Port::UsbA2(USB2), R, 27.0), (Port::Rj45, R, 45.75), (Port::MicroB, B, 10.6), (Port::Hdmi, B, 32.0), (Port::Audio, B, 53.5)])
        .chips(&[soc, plastic(lan, 59.0, 30.5, 9.0, 9.0)])
        .fpc((45.0, 44.0), 22.0, false, true)
        .fpc((2.9, 21.5), 22.0, false, true)
        .sd(L, 28.0)
        .pwr_act(1.8, 43.0)
        .label(name, (25.0, 41.0), 1.6)
}

fn a_plus(name: &'static str, soc: Chip) -> Layout {
    Layout::new(65.0, 56.0, PI_GREEN)
        .mounts(&PI_MOUNTS)
        .top_header(32.5, 40, true)
        .ports(&[(Port::UsbA(USB2), R, 31.45), (Port::MicroB, B, 10.6), (Port::Hdmi, B, 32.0), (Port::Audio, B, 53.5)])
        .chips(&[soc])
        .fpc((45.0, 40.5), 22.0, false, true)
        .fpc((2.9, 21.5), 22.0, false, true)
        .sd(L, 28.0)
        .pwr_act(1.8, 43.0)
        .label(name, (24.0, 43.5), 1.4)
}

fn pi1(name: &'static str, ethernet: bool) -> Layout {
    let mut l = Layout::new(85.6, 56.5, PI_GREEN)
        .mounts(&[(25.5, 18.0), (70.0, 35.0)])
        .ports(&[(Port::Hdmi, B, 37.1), (Port::MicroB, L, 48.5), (Port::Rca, T, 46.0), (Port::Audio, T, 70.0)])
        .chips(&[plastic("BCM2835", 38.0, 28.5, 12.0, 12.0)])
        .fpc((54.0, 46.0), 18.0, false, true)
        .fpc((4.2, 22.0), 16.0, false, true)
        .sd(L, 28.0)
        .pwr_act(80.0, 34.0)
        .label(name, (36.0, 41.0), 1.5);
    l.r = 0.5;
    l.header = Some(Header { c: (18.5, 3.5), pins: 26, rows: 2, rot: 0.0, populated: true });
    l.ports.push((if ethernet { Port::UsbA2(USB2) } else { Port::UsbA(USB2) }, R, 24.5));
    if ethernet {
        l.ports.push((Port::Rj45, R, 46.3));
        l.chips.push(plastic("LAN9512", 63.0, 30.0, 9.0, 9.0));
    }
    l
}

fn zero(name: &'static str, soc: Chip, wifi: bool, populated: bool) -> Layout {
    let mut l = Layout::new(65.0, 30.0, PI_GREEN)
        .mounts(&[(3.5, 3.5), (61.5, 3.5), (3.5, 26.5), (61.5, 26.5)])
        .top_header(32.5, 40, populated)
        .ports(&[(Port::MiniHdmi, B, 12.4), (Port::MicroB, B, 41.4), (Port::MicroB, B, 54.0)])
        .chips(&[soc])
        .fpc((63.0, 15.0), 17.0, false, false)
        .sd(L, 16.9)
        .leds(&[("ACT", (58.5, 24.0), LedColor::Green)])
        .label(name, (32.5, 8.4), 1.3);
    if wifi {
        l.chips.push(plastic("", 48.0, 15.0, 5.0, 5.0));
    }
    l
}

fn cm_sodimm(name: &'static str, soc: Chip) -> Layout {
    let mut l = Layout::new(67.6, 31.0, PI_GREEN).chips(&[soc, plastic("EMMC", 50.0, 13.0, 11.5, 13.0)]).label(name, (33.8, 4.0), 1.3);
    l.r = 1.0;
    l.t = 1.0;
    l.hole = 0.0;
    l
}

fn cm_hirose(name: &'static str, soc: Chip, ram: Chip, mask: [f32; 3]) -> Layout {
    let mut l = Layout::new(55.0, 40.0, mask)
        .mounts(&[(3.5, 3.5), (51.5, 3.5), (3.5, 36.5), (51.5, 36.5)])
        .chips(&[soc, ram, plastic("EMMC", 40.5, 25.0, 11.5, 13.0), can(9.0, 30.0, 11.0, 10.0)])
        .leds(&[("PWR", (52.5, 22.0), LedColor::Red), ("ACT", (52.5, 25.0), LedColor::Green)])
        .label(name, (27.5, 37.6), 1.2);
    l.hole = 2.7;
    l.t = 1.2;
    l
}

/// Orange Pi H3/H5 class board, 85 × 55 mm.
fn opi_pc(name: &'static str, soc: &'static str, extras: &[Chip]) -> Layout {
    Layout::new(85.0, 55.0, OPI_BLUE)
        .mounts(&[(3.5, 3.5), (61.5, 3.5), (3.5, 51.5), (61.5, 51.5)])
        .top_header(32.5, 40, true)
        .ports(&[(Port::UsbA2(USB2), R, 22.0), (Port::Rj45, R, 44.5), (Port::UsbA(USB2), L, 14.0), (Port::Dc, B, 9.0), (Port::Hdmi, B, 30.0), (Port::Audio, B, 48.0), (Port::MicroB, L, 36.0)])
        .chips(&[plastic(soc, 34.0, 25.0, 14.0, 14.0), plastic("DDR3", 49.0, 21.0, 8.0, 12.0), plastic("DDR3", 49.0, 34.0, 8.0, 12.0)])
        .chips(extras)
        .fpc((58.0, 42.0), 16.0, false, true)
        .sd(L, 26.0)
        .button(76.0, 3.0)
        .leds(&[("PWR", (11.0, 46.0), LedColor::Red), ("ACT", (14.0, 46.0), LedColor::Green)])
        .label(name, (28.0, 42.0), 1.6)
}

/// Orange Pi One / Lite class board, 69 × 48 mm.
fn opi_one(name: &'static str, soc: &'static str, ethernet: bool, usb3: bool, wifi: bool) -> Layout {
    let mut l = Layout::new(69.0, 48.0, OPI_BLUE)
        .corners(3.0)
        .top_header(34.5, 40, true)
        .ports(&[(Port::UsbA(if usb3 { USB3 } else { USB2 }), R, 15.0), (Port::Hdmi, B, 31.0), (Port::Dc, B, 10.0), (Port::MicroB, L, 30.0)])
        .chips(&[plastic(soc, 30.0, 22.0, 14.0, 14.0), plastic("DDR", 44.0, 31.0, 10.0, 8.0)])
        .sd(L, 14.0)
        .leds(&[("PWR", (48.0, 44.0), LedColor::Red), ("ACT", (51.0, 44.0), LedColor::Green)])
        .label(name, (25.0, 9.0), 1.5);
    l.ports.push(if ethernet { (Port::Rj45, R, 36.0) } else { (Port::UsbA(USB2), R, 34.0) });
    if wifi {
        l.chips.push(can(15.0, 22.0, 8.0, 8.0));
    }
    l
}

/// Orange Pi Zero class board with a 26-pin header along the top edge.
fn opi_zero(name: &'static str, soc: &'static str, w: f64, h: f64, ports: &[(Port, Edge, f64)], wifi: bool) -> Layout {
    let mut l = Layout::new(w, h, OPI_BLUE)
        .corners(2.6)
        .top_header(w / 2.0 - 3.0, 26, true)
        .ports(ports)
        .chips(&[plastic(soc, w / 2.0, h * 0.38, 12.0, 12.0)])
        .sd(R, h * 0.35)
        .leds(&[("PWR", (4.5, h * 0.45), LedColor::Red), ("ACT", (4.5, h * 0.45 + 2.6), LedColor::Green)])
        .label(name, (w / 2.0, h * 0.62), 1.2);
    if wifi {
        l.chips.push(plastic("", w * 0.82, h * 0.25, 5.0, 5.0));
    }
    l
}

/// Raspberry-Pi-sized Orange Pi (85–91 × 56 mm).
fn opi_b(name: &'static str, w: f64, soc: Chip, mask: [f32; 3], usb: [Mat; 4], audio: bool, wifi: bool) -> Layout {
    let mut l = Layout::new(w, 56.0, mask)
        .mounts(&PI_MOUNTS)
        .top_header(32.5, 40, true)
        .chips(&[soc, plastic("LPDDR4", 45.0, 22.0, 10.0, 12.0)])
        .fpc((47.0, 44.0), 18.0, false, false)
        .sd(L, 28.0)
        .pwr_act(1.8, 43.0)
        .label(name, (24.0, 41.0), 1.5);
    let top = [(Port::UsbA(usb[0]), R, 9.0), (Port::UsbA(usb[2]), R, 27.0)];
    for (p, e, y) in top {
        l.ports.push((p, e, y));
    }
    // stacked pairs: the upper port of each stack has its own tongue colour
    l.ports.push((Port::UsbA2(usb[1]), R, 9.0));
    l.ports.push((Port::UsbA2(usb[3]), R, 27.0));
    l.ports.extend_from_slice(&[(Port::Rj45, R, 45.75), (Port::TypeC, B, 10.6), (Port::Hdmi, B, 30.0)]);
    if audio {
        l.ports.push((Port::Audio, B, 50.0));
    }
    if wifi {
        l.chips.push(can(14.0, 16.0, 11.0, 10.0));
    }
    l
}

/// Orange Pi 5 family (RK3588/RK3588S).
fn opi5(name: &'static str, w: f64, h: f64, soc: &'static str, header: usize) -> Layout {
    Layout::new(w, h, OPI_GREEN)
        .corners(3.5)
        .top_header(if header == 26 { 45.0 } else { 40.0 }, header, true)
        .chips(&[lid(soc, 40.0, 30.0, 17.0, 17.0), plastic("LPDDR4X", 58.0, 22.0, 10.0, 12.0), plastic("LPDDR4X", 58.0, 37.0, 10.0, 12.0)])
        .fpc((22.0, 28.0), 18.0, false, false)
        .jst((72.0, 6.0), 2, true)
        .sd(L, 32.0)
        .pwr_act(2.0, 46.0)
        .label(name, (36.0, h - 13.0), 1.6)
}

fn layout(s: Sbc) -> Layout {
    match s {
        Pi1A => pi1("RASPBERRY PI MODEL A", false),
        Pi1B => pi1("RASPBERRY PI MODEL B", true),
        Pi1APlus => a_plus("RASPBERRY PI A+", plastic("BCM2835", 29.0, 32.0, 12.0, 12.0)),
        Pi1BPlus => b_plus("RASPBERRY PI MODEL B+", plastic("BCM2835", 29.0, 32.5, 12.0, 12.0), "LAN9514"),
        Pi2B => b_plus("RASPBERRY PI 2 MODEL B", plastic("BCM2836", 29.0, 32.5, 14.0, 14.0), "LAN9514"),
        Pi3B => b_plus("RASPBERRY PI 3 MODEL B", plastic("BCM2837", 29.0, 32.5, 14.0, 14.0), "LAN9514").chips(&[plastic("", 11.0, 30.0, 6.0, 6.0)]),
        Pi3BPlus => {
            let mut l = b_plus("RASPBERRY PI 3 MODEL B+", lid("BCM2837B0", 29.0, 32.5, 14.0, 14.0), "LAN7515").chips(&[can(12.5, 15.5, 12.0, 11.0)]);
            l.poe = Some((62.0, 9.5));
            l
        }
        Pi3APlus => a_plus("RASPBERRY PI 3 A+", lid("BCM2837B0", 29.0, 32.0, 14.0, 14.0)).chips(&[can(12.5, 15.5, 12.0, 11.0)]),
        Pi5 => {
            let mut l = Layout::new(85.0, 56.0, PI_GREEN)
                .mounts(&PI_MOUNTS)
                .top_header(32.5, 40, true)
                .ports(&[(Port::UsbA2(USB2), R, 9.0), (Port::UsbA2(USB3), R, 27.0), (Port::Rj45, R, 45.75), (Port::TypeC, B, 11.2), (Port::MicroHdmi, B, 26.0), (Port::MicroHdmi, B, 39.5)])
                .chips(&[lid("BCM2712", 29.0, 26.0, 15.0, 15.0), plastic("RP1", 60.0, 30.0, 7.0, 7.0), plastic("LPDDR4X", 44.0, 22.0, 9.0, 12.0), can(12.5, 15.5, 11.0, 10.0)])
                .fpc((51.0, 47.0), 13.0, false, false)
                .fpc((57.5, 47.0), 13.0, false, false)
                .fpc((2.6, 26.0), 14.0, false, false)
                .jst((69.0, 4.2), 4, true)
                .jst((62.0, 40.0), 2, true)
                .jst((47.0, 54.0), 3, true)
                .button(1.6, 44.0)
                .pwr_act(1.8, 48.0)
                .label("RASPBERRY PI 5", (22.0, 41.5), 1.7);
            l.poe = Some((62.0, 9.5));
            l
        }
        PiZero => zero("RASPBERRY PI ZERO", plastic("BCM2835", 30.0, 17.0, 12.0, 12.0), false, false),
        PiZeroW => zero("RASPBERRY PI ZERO W", plastic("BCM2835", 30.0, 17.0, 12.0, 12.0), true, false),
        PiZeroWH => zero("RASPBERRY PI ZERO WH", plastic("BCM2835", 30.0, 17.0, 12.0, 12.0), true, true),
        PiZero2W => zero("RASPBERRY PI ZERO 2 W", Chip { mark: "", c: (30.0, 17.0), size: (13.0, 13.0), style: Style::Can }, false, false),
        Cm1 => cm_sodimm("COMPUTE MODULE", plastic("BCM2835", 22.0, 14.0, 12.0, 12.0)),
        Cm3 => cm_sodimm("COMPUTE MODULE 3", plastic("BCM2837", 22.0, 14.0, 14.0, 14.0)),
        Cm3Plus => cm_sodimm("COMPUTE MODULE 3+", lid("BCM2837B0", 22.0, 14.0, 14.0, 14.0)),
        Cm4S => cm_sodimm("COMPUTE MODULE 4S", lid("BCM2711", 22.0, 14.0, 14.0, 14.0)),
        Cm4 => cm_hirose("RASPBERRY PI CM4", lid("BCM2711", 22.0, 14.0, 14.0, 14.0), plastic("LPDDR4", 40.0, 10.0, 11.0, 12.0), PI_GREEN),
        Cm5 => cm_hirose("RASPBERRY PI CM5", lid("BCM2712", 22.0, 14.0, 15.0, 15.0), plastic("LPDDR4X", 40.0, 10.0, 11.0, 12.0), PI_GREEN),
        OpiCm4 => cm_hirose("ORANGE PI CM4", plastic("RK3566", 22.0, 14.0, 13.0, 13.0), plastic("LPDDR4", 40.0, 10.0, 11.0, 12.0), OPI_BLACK),
        OpiCm5 => cm_hirose("ORANGE PI CM5", lid("RK3588S", 22.0, 14.0, 16.0, 16.0), plastic("LPDDR4X", 40.0, 10.0, 11.0, 12.0), OPI_BLACK),
        OpiZero => opi_zero("ORANGE PI ZERO", "H2+", 48.0, 46.0, &[(Port::Rj45, B, 13.0), (Port::UsbA(USB2), B, 33.0), (Port::MicroB, L, 30.0)], true),
        OpiZeroLts => opi_zero("ORANGE PI ZERO LTS", "H2+", 48.0, 46.0, &[(Port::Rj45, B, 13.0), (Port::UsbA(USB2), B, 33.0), (Port::MicroB, L, 30.0)], true),
        OpiZeroPlus => opi_zero("ORANGE PI ZERO PLUS", "H5", 48.0, 46.0, &[(Port::Rj45, B, 13.0), (Port::UsbA(USB2), B, 33.0), (Port::MicroB, L, 30.0)], true),
        OpiZeroPlus2 => opi_zero("ZERO PLUS 2", "H5", 46.0, 48.0, &[(Port::MiniHdmi, B, 13.0), (Port::MicroB, B, 28.0), (Port::MicroB, B, 38.0)], true).chips(&[plastic("EMMC", 36.0, 20.0, 8.0, 10.0)]),
        OpiZero2 => opi_zero("ORANGE PI ZERO 2", "H616", 60.0, 53.0, &[(Port::Rj45, B, 13.0), (Port::UsbA(USB2), B, 33.0), (Port::TypeC, L, 40.0), (Port::MicroHdmi, L, 26.0)], true),
        OpiZero3 => opi_zero("ORANGE PI ZERO 3", "H618", 50.0, 55.0, &[(Port::Rj45, B, 12.5), (Port::UsbA(USB2), B, 33.0), (Port::TypeC, L, 42.0), (Port::MicroHdmi, L, 27.0)], true),
        OpiZero2W => {
            let mut l = zero("ORANGE PI ZERO 2W", plastic("H618", 30.0, 17.0, 12.0, 12.0), true, false);
            l.mask = OPI_BLACK;
            l.ports = vec![(Port::MiniHdmi, B, 12.4), (Port::TypeC, B, 41.4), (Port::TypeC, B, 54.0)];
            l.leds = vec![("PWR", (58.5, 23.0), LedColor::Red), ("ACT", (55.5, 23.0), LedColor::Green)];
            l
        }
        OpiOne => opi_one("ORANGE PI ONE", "H3", true, false, false),
        OpiOnePlus => opi_one("ORANGE PI ONE PLUS", "H6", true, false, false),
        OpiLite => opi_one("ORANGE PI LITE", "H3", false, false, true),
        OpiLite2 => opi_one("ORANGE PI LITE 2", "H6", false, true, true),
        OpiPc => opi_pc("ORANGE PI PC", "H3", &[]),
        OpiPcPlus => opi_pc("ORANGE PI PC PLUS", "H3", &[plastic("EMMC", 18.0, 22.0, 9.0, 11.0), can(66.0, 8.5, 8.0, 7.0)]),
        OpiPc2 => opi_pc("ORANGE PI PC 2", "H5", &[]),
        OpiPlus2E => opi_pc("ORANGE PI PLUS 2E", "H3", &[plastic("EMMC", 18.0, 22.0, 9.0, 11.0), can(66.0, 8.5, 8.0, 7.0)]),
        OpiPlus | OpiPlus2 => {
            let mut l = Layout::new(108.0, 67.0, OPI_BLUE)
                .corners(3.5)
                .top_header(34.0, 40, true)
                .ports(&[(Port::UsbA2(USB2), R, 12.0), (Port::UsbA2(USB2), R, 30.0), (Port::Rj45, R, 51.0), (Port::Dc, B, 10.0), (Port::Hdmi, B, 32.0), (Port::Audio, B, 52.0), (Port::MicroB, L, 40.0)])
                .chips(&[plastic("H3", 40.0, 30.0, 14.0, 14.0), plastic("DDR3", 56.0, 24.0, 8.0, 12.0), plastic("DDR3", 56.0, 38.0, 8.0, 12.0), plastic("EMMC", 22.0, 26.0, 9.0, 11.0), can(78.0, 47.0, 8.0, 7.0)])
                .sd(L, 22.0)
                .button(90.0, 3.0)
                .pwr_act(64.0, 58.0)
                .label(if s == OpiPlus { "ORANGE PI PLUS" } else { "ORANGE PI PLUS 2" }, (36.0, 52.0), 1.8);
            l.sata = Some((68.0, 62.0));
            l
        }
        OpiPrime => {
            let mut l = opi_pc("ORANGE PI PRIME", "H5", &[can(66.0, 8.5, 8.0, 7.0)]);
            l.w = 98.0;
            l.h = 60.0;
            l.mounts = vec![(3.5, 3.5), (94.5, 3.5), (3.5, 56.5), (61.5, 56.5)];
            l.ports = vec![(Port::UsbA2(USB2), R, 20.0), (Port::Rj45, R, 48.0), (Port::UsbA(USB2), L, 14.0), (Port::Dc, B, 9.0), (Port::Hdmi, B, 30.0), (Port::Audio, B, 48.0), (Port::MicroB, L, 38.0)];
            l
        }
        OpiWin | OpiWinPlus => Layout::new(93.0, 60.0, OPI_BLUE)
            .corners(3.5)
            .top_header(32.5, 40, true)
            .ports(&[(Port::UsbA2(USB2), R, 13.0), (Port::UsbA2(USB2), R, 31.0), (Port::Rj45, B, 75.0), (Port::Hdmi, B, 45.0), (Port::Audio, B, 60.0), (Port::Dc, B, 12.0), (Port::MicroB, B, 25.0)])
            .chips(&[plastic("A64", 36.0, 28.0, 14.0, 14.0), plastic("DDR3", 52.0, 22.0, 8.0, 12.0), plastic("DDR3", 52.0, 36.0, 8.0, 12.0), can(16.0, 22.0, 9.0, 8.0)])
            .sd(L, 34.0)
            .pwr_act(2.0, 44.0)
            .label(if s == OpiWin { "ORANGE PI WIN" } else { "ORANGE PI WIN PLUS" }, (36.0, 44.0), 1.6),
        Opi3 => Layout::new(90.0, 64.0, OPI_BLUE)
            .corners(3.5)
            .top_header(30.0, 26, true)
            .ports(&[(Port::UsbA2(USB3), R, 15.0), (Port::UsbA2(USB3), R, 33.0), (Port::Rj45, R, 52.0), (Port::Hdmi, B, 35.0), (Port::Audio, B, 52.0), (Port::Dc, B, 12.0)])
            .chips(&[plastic("H6", 36.0, 30.0, 14.0, 14.0), plastic("LPDDR3", 54.0, 24.0, 10.0, 12.0), plastic("EMMC", 18.0, 30.0, 9.0, 11.0), can(16.0, 14.0, 9.0, 8.0)])
            .sd(L, 44.0)
            .pwr_act(2.0, 50.0)
            .label("ORANGE PI 3", (34.0, 46.0), 1.8),
        Opi3Lts => opi_b("ORANGE PI 3 LTS", 85.0, plastic("H6", 29.0, 30.0, 14.0, 14.0), OPI_BLUE, [USB3, USB2, USB2, USB2], true, true),
        Opi3B => opi_b("ORANGE PI 3B", 85.0, plastic("RK3566", 29.0, 30.0, 13.0, 13.0), OPI_GREEN, [USB3, USB2, USB2, USB2], false, true),
        Opi4 => opi_b("ORANGE PI 4", 91.0, lid("RK3399", 29.0, 30.0, 15.0, 15.0), OPI_BLACK, [USB3, USB2, USB3, USB2], true, false),
        Opi4Lts => opi_b("ORANGE PI 4 LTS", 91.0, lid("RK3399", 29.0, 30.0, 15.0, 15.0), OPI_BLACK, [USB3, USB2, USB3, USB2], true, true),
        Opi4A => opi_b("ORANGE PI 4A", 85.0, plastic("T527", 29.0, 30.0, 14.0, 14.0), OPI_GREEN, [USB3, USB2, USB2, USB2], true, true),
        OpiRv => opi_b("ORANGE PI RV", 89.0, plastic("JH7110", 29.0, 30.0, 14.0, 14.0), OPI_GREEN, [USB3, USB3, USB3, USB3], false, true).ports(&[(Port::Rj45, L, 46.0)]),
        OpiRv2 => opi_b("ORANGE PI RV2", 89.0, plastic("KY X1", 29.0, 30.0, 14.0, 14.0), OPI_BLACK, [USB3, USB3, USB2, USB2], false, true).ports(&[(Port::Rj45, L, 46.0)]),
        Opi5 => opi5("ORANGE PI 5", 100.0, 62.0, "RK3588S", 26).ports(&[(Port::UsbA(USB3), R, 11.0), (Port::UsbA2(USB2), R, 30.0), (Port::Rj45, R, 50.0), (Port::TypeC, B, 12.0), (Port::TypeC, B, 26.0), (Port::Hdmi, B, 45.0), (Port::Audio, B, 62.0)]),
        Opi5B => opi5("ORANGE PI 5B", 100.0, 62.0, "RK3588S", 26)
            .ports(&[(Port::UsbA(USB3), R, 11.0), (Port::UsbA2(USB2), R, 30.0), (Port::Rj45, R, 50.0), (Port::TypeC, B, 12.0), (Port::TypeC, B, 26.0), (Port::Hdmi, B, 45.0), (Port::Audio, B, 62.0)])
            .chips(&[can(78.0, 10.0, 10.0, 8.0)]),
        Opi5Plus => opi5("ORANGE PI 5 PLUS", 100.0, 75.0, "RK3588", 40).ports(&[
            (Port::UsbA2(USB3), R, 12.0),
            (Port::UsbA2(USB2), R, 30.0),
            (Port::Rj45, R, 48.5),
            (Port::Rj45, R, 65.5),
            (Port::TypeC, B, 10.0),
            (Port::Hdmi, B, 27.0),
            (Port::Hdmi, B, 45.0),
            (Port::Hdmi, B, 63.0),
            (Port::Audio, B, 79.0),
        ]),
        Opi5Pro | Opi5Max | Opi5Ultra => {
            let (name, soc) = match s {
                Opi5Pro => ("ORANGE PI 5 PRO", "RK3588S"),
                Opi5Max => ("ORANGE PI 5 MAX", "RK3588"),
                _ => ("ORANGE PI 5 ULTRA", "RK3588"),
            };
            let mut l = opi5(name, 89.0, 57.0, soc, 40).ports(&[(Port::UsbA2(USB3), R, 9.0), (Port::UsbA2(USB2), R, 27.0), (Port::Rj45, R, 45.75), (Port::TypeC, B, 10.0), (Port::Hdmi, B, 27.0), (Port::Hdmi, B, 45.0)]);
            l.mounts = PI_MOUNTS.to_vec();
            l.header = Some(Header { c: (32.5, 3.5), pins: 40, rows: 2, rot: 0.0, populated: true });
            l.jst = vec![((63.0, 52.0), 2, true)];
            l.chips[1].c = (55.0, 20.0);
            l.chips[2].c = (55.0, 35.0);
            l.chips.push(can(12.0, 16.0, 10.0, 9.0));
            l
        }
        OpiR1 => opi_zero("ORANGE PI R1", "H2+", 60.0, 46.0, &[(Port::Rj45, B, 13.0), (Port::Rj45, B, 32.0), (Port::UsbA(USB2), R, 34.0), (Port::MicroB, L, 30.0)], true),
        OpiR1Plus | OpiR1PlusLts => {
            let mut l = opi_zero(if s == OpiR1Plus { "ORANGE PI R1 PLUS" } else { "R1 PLUS LTS" }, "RK3328", 56.0, 57.0, &[(Port::Rj45, B, 12.0), (Port::Rj45, B, 31.0), (Port::UsbA(USB2), R, 22.0), (Port::TypeC, L, 40.0)], false);
            l.mask = OPI_BLACK;
            l.header = Some(Header { c: (26.0, 3.0), pins: 13, rows: 1, rot: 0.0, populated: true });
            l
        }
        OpiAiPro => Layout::new(107.0, 68.0, OPI_BLACK)
            .corners(3.5)
            .top_header(40.0, 40, true)
            .ports(&[(Port::UsbA2(USB3), R, 15.0), (Port::Rj45, R, 38.0), (Port::Hdmi, B, 20.0), (Port::Hdmi, B, 38.0), (Port::TypeC, B, 58.0), (Port::Audio, B, 74.0)])
            .chips(&[lid("ASCEND 310B", 45.0, 32.0, 18.0, 18.0), plastic("LPDDR4X", 66.0, 26.0, 10.0, 12.0), plastic("LPDDR4X", 66.0, 41.0, 10.0, 12.0), can(20.0, 20.0, 12.0, 10.0)])
            .jst((86.0, 6.0), 4, true)
            .sd(L, 40.0)
            .pwr_act(2.0, 52.0)
            .label("ORANGE PI AIPRO", (40.0, 56.0), 2.0),
        OpiI96 => {
            let mut l = Layout::new(85.0, 30.0, OPI_BLUE)
                .corners(3.0)
                .top_header(42.5, 40, true)
                .ports(&[(Port::MicroB, B, 10.0), (Port::MicroB, B, 25.0)])
                .chips(&[plastic("RDA8810", 42.0, 17.0, 11.0, 11.0), can(62.0, 17.0, 9.0, 8.0)])
                .sd(R, 15.0)
                .pwr_act(75.0, 17.0)
                .label("ORANGE PI I96", (22.0, 18.0), 1.4);
            l.header = Some(Header { c: (42.5, 3.4), pins: 40, rows: 2, rot: 0.0, populated: false });
            l
        }
        Opi2gIot => Layout::new(67.0, 42.0, OPI_BLUE)
            .corners(3.0)
            .top_header(33.5, 40, true)
            .ports(&[(Port::MicroB, B, 12.0), (Port::UsbA(USB2), R, 28.0)])
            .chips(&[plastic("RDA8810", 26.0, 22.0, 11.0, 11.0), can(44.0, 22.0, 10.0, 9.0)])
            .sd(L, 24.0)
            .pwr_act(3.0, 34.0)
            .label("ORANGE PI 2G-IOT", (26.0, 36.0), 1.3),
        Opi4gIot => Layout::new(80.0, 58.0, OPI_BLUE)
            .corners(3.5)
            .top_header(32.5, 40, true)
            .ports(&[(Port::MicroB, B, 12.0), (Port::Hdmi, B, 32.0), (Port::UsbA(USB2), R, 40.0), (Port::Audio, B, 52.0)])
            .chips(&[plastic("MT6737", 30.0, 28.0, 13.0, 13.0), can(52.0, 24.0, 14.0, 12.0)])
            .sd(L, 30.0)
            .pwr_act(2.0, 44.0)
            .label("ORANGE PI 4G-IOT", (30.0, 44.0), 1.5),
        Pi400 | Pi500 | Opi800 | PicoH | PicoW | PicoWH | Pico2 | Pico2W => unreachable!("built separately"),
    }
}

// ------------------------------------------------------------------ header pin names

const PI_J8: [&str; 40] = [
    "3V3", "5V", "GPIO2", "5V", "GPIO3", "GND", "GPIO4", "GPIO14", "GND", "GPIO15", "GPIO17", "GPIO18", "GPIO27", "GND", "GPIO22", "GPIO23", "3V3", "GPIO24", "GPIO10", "GND",
    "GPIO9", "GPIO25", "GPIO11", "GPIO8", "GND", "GPIO7", "ID_SD", "ID_SC", "GPIO5", "GND", "GPIO6", "GPIO12", "GPIO13", "GND", "GPIO19", "GPIO16", "GPIO26", "GPIO20", "GND",
    "GPIO21",
];

fn pin_name(s: Sbc, i: usize, rows: usize) -> String {
    if rows == 1 {
        // 13-pin auxiliary header of the router boards
        return match i {
            0 => "5V".into(),
            1 => "3V3".into(),
            2 => "GND".into(),
            _ => format!("PIN{}", i + 1),
        };
    }
    if !s.orange() {
        return PI_J8[i].into();
    }
    match i + 1 {
        1 | 17 => "3V3".into(),
        2 | 4 => "5V".into(),
        6 | 9 | 14 | 20 | 25 | 30 | 34 | 39 => "GND".into(),
        n => format!("PIN{n}"),
    }
}

fn header_pins(s: Sbc, h: &Header) -> Vec<(String, P)> {
    let per_row = h.pins / h.rows;
    let c = p(h.c.0, h.c.1);
    (0..h.pins)
        .map(|i| {
            let (col, row) = if h.rows == 1 { (i, 0) } else { (i / 2, i % 2) };
            let a = (col as f64 - (per_row as f64 - 1.0) / 2.0) * 2.54;
            let b = if h.rows == 1 { 0.0 } else { 1.27 - row as f64 * 2.54 };
            (pin_name(s, i, h.rows), c + crate::pcb::geom::rot(p(a, b), h.rot))
        })
        .collect()
}

// ------------------------------------------------------------------ builder

fn edge_point(l: &Layout, e: Edge, pos: f64, over: f64) -> (P, P) {
    match e {
        T => (p(pos, -over), p(0.0, -1.0)),
        B => (p(pos, l.h + over), p(0.0, 1.0)),
        L => (p(-over, pos), p(-1.0, 0.0)),
        R => (p(l.w + over, pos), p(1.0, 0.0)),
    }
}

fn add_port(m: &mut Model, l: &Layout, port: Port, e: Edge, pos: f64, z: f64) {
    let (kind, over) = match port {
        Port::UsbA(c) => (Receptacle::TypeA(c), 2.0),
        Port::UsbA2(c) => {
            // the upper port of a stack; the lower one is listed separately or added here
            let (at, d) = edge_point(l, e, pos, 2.0);
            if !l.ports.iter().any(|&(q, e2, p2)| matches!(q, Port::UsbA(_)) && e2 == e && (p2 - pos).abs() < 1e-6) {
                receptacle(m, Receptacle::TypeA(c), at, d, z);
            }
            receptacle(m, Receptacle::TypeA(c), at, d, z + 7.8);
            return;
        }
        Port::MicroB => (Receptacle::MicroB, 0.8),
        Port::TypeC => (Receptacle::TypeC, 1.2),
        Port::Hdmi => (Receptacle::Hdmi, 1.2),
        Port::MiniHdmi => (Receptacle::MiniHdmi, 0.6),
        Port::MicroHdmi => (Receptacle::MicroHdmi, 1.0),
        Port::Rj45 => (Receptacle::Rj45, 2.1),
        Port::Audio => (Receptacle::Audio, 0.0),
        Port::Rca => (Receptacle::Rca, 0.0),
        Port::Dc => (Receptacle::DcSmall, 0.5),
    };
    let (at, d) = edge_point(l, e, pos, over);
    receptacle(m, kind, at, d, z);
}

fn add_chip(m: &mut Model, c: &Chip, z: f64) {
    let (cx, cy) = c.c;
    let (w, h) = c.size;
    let (lo, hi) = ([cx - w / 2.0, cy - h / 2.0], [cx + w / 2.0, cy + h / 2.0]);
    let top = match c.style {
        Style::Plastic => {
            m.body(BLACK, [lo[0], lo[1], z], [hi[0], hi[1], z + 1.1], 0.15);
            m.cyl(Mat::Rgb(70, 70, 72), [lo[0] + 1.0, lo[1] + 1.0, z + 1.1], [lo[0] + 1.0, lo[1] + 1.0, z + 1.11], 0.35, 12);
            z + 1.1
        }
        Style::Lid => {
            m.boxx(Mat::Rgb(25, 70, 40), [lo[0], lo[1], z], [hi[0], hi[1], z + 0.9]);
            m.rbox(Mat::Metal, [lo[0] + 0.6, lo[1] + 0.6, z + 0.9], [hi[0] - 0.6, hi[1] - 0.6, z + 2.2], 1.0);
            z + 2.2
        }
        Style::Can => {
            m.rbox(Mat::Rgb(200, 202, 205), [lo[0], lo[1], z], [hi[0], hi[1], z + 1.6], 0.5);
            m.boxx(Mat::Rgb(185, 187, 191), [cx - w * 0.3, cy - h * 0.3, z + 1.6], [cx + w * 0.3, cy + h * 0.3, z + 1.62]);
            z + 1.6
        }
    };
    if !c.mark.is_empty() {
        let size = (w * 0.8 / c.mark.len().max(4) as f64 * 1.3).min(1.6);
        text(m, MARK, c.mark, p(cx, cy), size, 0.0, top);
    }
}

/// Ribbon (FPC) connector: white housing with a dark latch.
fn add_fpc(m: &mut Model, (c, len, along_x, tall): ((f64, f64), f64, bool, bool), z: f64) {
    let (h, depth) = if tall { (5.5, 3.4) } else { (2.2, 4.5) };
    let (hx, hy) = if along_x { (len / 2.0, depth / 2.0) } else { (depth / 2.0, len / 2.0) };
    m.boxx(Mat::Rgb(236, 236, 230), [c.0 - hx, c.1 - hy, z], [c.0 + hx, c.1 + hy, z + h]);
    let latch = Mat::Rgb(40, 40, 42);
    if along_x {
        m.boxx(latch, [c.0 - hx + 0.3, c.1 - hy + 0.3, z + h], [c.0 + hx - 0.3, c.1, z + h + 0.3]);
    } else {
        m.boxx(latch, [c.0 - hx + 0.3, c.1 - hy + 0.3, z + h], [c.0, c.1 + hy - 0.3, z + h + 0.3]);
    }
}

fn add_jst(m: &mut Model, ((x, y), pins, along_x): ((f64, f64), usize, bool), z: f64) {
    let len = pins as f64 * 1.0 + 2.0;
    let (hx, hy) = if along_x { (len / 2.0, 1.6) } else { (1.6, len / 2.0) };
    m.boxx(Mat::Rgb(236, 236, 230), [x - hx, y - hy, z], [x + hx, y + hy, z + 4.2]);
    m.boxx(Mat::Rgb(60, 60, 60), [x - hx + 0.4, y - hy + 0.4, z + 4.2], [x + hx - 0.4, y + hy - 0.4, z + 4.22]);
}

fn finish(b: &mut Build, s: Sbc, l: &Layout, m: Model, pin_z: f64, pins: &[(String, P)]) {
    b.model(&m);
    for &(name, (x, y), c) in &l.leds {
        let (off, on) = c.rgb();
        b.lamp(name, off, on, &smd_led(p(x, y), l.t));
    }
    for (n, q) in pins {
        b.pin(n, [q.x, q.y, pin_z]);
    }
    let _ = s;
}

/// A board described by its layout.
fn board(s: Sbc) -> DevicePart {
    let l = layout(s);
    let t = l.t;
    let outline = if l.r > 0.0 { rounded_rect(p(l.w / 2.0, l.h / 2.0), l.w, l.h, l.r, 6) } else { rect_pts(P::ZERO, p(l.w, l.h)) };
    let holes: Vec<(P, f64)> = if l.hole > 0.0 { l.mounts.iter().map(|&(x, y)| (p(x, y), l.hole)).collect() } else { vec![] };
    let mut b = Build::board(s.name(), outline, &holes, t, l.mask);
    let mut m = Model::default();
    for &(port, e, pos) in &l.ports {
        add_port(&mut m, &l, port, e, pos, t);
    }
    for c in &l.chips {
        add_chip(&mut m, c, t);
    }
    for &f in &l.fpcs {
        add_fpc(&mut m, f, t);
    }
    for &j in &l.jst {
        add_jst(&mut m, j, t);
    }
    for &(x, y) in &l.buttons {
        smd_button(&mut m, p(x, y), 3.0, 2.4, t, BLACK);
    }
    if let Some((e, pos)) = l.sd {
        let (at, d) = edge_point(&l, e, pos, 0.0);
        m.obox(Mat::Metal, at, d, (-15.0, -0.2), (-7.0, 7.0), (-1.5, 0.0));
        m.obox(Mat::Rgb(25, 25, 28), at, d, (-13.0, 2.2), (-5.5, 5.5), (-1.25, -0.5));
    }
    if let Some((x, y)) = l.poe {
        let at: Vec<P> = (0..4).map(|i| p(x + (i % 2) as f64 * 2.54, y + (i / 2) as f64 * 2.54)).collect();
        male_pins(&mut m, &at, (t, t + 2.5), t - 0.5, t + 8.5);
    }
    if let Some((x, y)) = l.sata {
        m.boxx(BLACK, [x - 8.5, y - 2.8, t], [x + 8.5, y + 2.8, t + 5.5]);
        m.boxx(Mat::Rgb(20, 20, 20), [x - 7.5, y - 1.0, t + 5.5], [x + 7.5, y + 1.0, t + 5.52]);
    }
    let mut pins = vec![];
    let mut pin_z = t + 0.05;
    if let Some(h) = &l.header {
        pins = header_pins(s, h);
        let at: Vec<P> = pins.iter().map(|x| x.1).collect();
        if h.populated {
            male_pins(&mut m, &at, (t, t + 2.5), t - 0.5, t + 8.5);
            pin_z = t + 8.5;
        } else {
            for &q in &at {
                m.annulus(Mat::Gold, q, 0.5, 0.9, t + 0.02, 16, false);
                m.cyl(Mat::Rgb(20, 20, 20), [q.x, q.y, t + 0.01], [q.x, q.y, t + 0.02], 0.5, 12);
            }
        }
        if let Some(&(_, q)) = pins.first() {
            text(&mut m, l.silk, "1", q + crate::pcb::geom::rot(p(-2.4, 0.0), h.rot), 0.9, 0.0, t);
        }
    }
    if s.compute_module() {
        cm_connectors(&mut m, &l, s, &mut pins);
        pin_z = if l.w > 60.0 { t / 2.0 } else { -3.0 };
    }
    for &(txt, (x, y), size) in &l.labels {
        text(&mut m, l.silk, txt, p(x, y), size, 0.0, t);
    }
    finish(&mut b, s, &l, m, pin_z, &pins);
    b.done(Kind::Board(BoardKind::Sbc(s)))
}

/// Compute modules: gold fingers (SO-DIMM) or the two board-to-board connectors underneath,
/// with the GPIO bank as pins along them.
fn cm_connectors(m: &mut Model, l: &Layout, _s: Sbc, pins: &mut Vec<(String, P)>) {
    let names: Vec<String> = ["5V", "3V3", "GND"].iter().map(|s| s.to_string()).chain((0..28).map(|i| format!("GPIO{i}"))).collect();
    if l.w > 60.0 {
        // SO-DIMM edge: fingers on both faces along the lower edge, notch near the middle
        let n = 100;
        for i in 0..n {
            let x = 3.0 + i as f64 * 0.6;
            if (x - 40.0).abs() < 0.9 {
                continue;
            }
            m.boxx(Mat::Gold, [x, l.h - 3.0, l.t], [x + 0.4, l.h - 0.3, l.t + 0.02]);
            m.boxx(Mat::Gold, [x, l.h - 3.0, -0.02], [x + 0.4, l.h - 0.3, 0.0]);
        }
        for (i, n) in names.iter().enumerate() {
            pins.push((n.clone(), p(4.0 + i as f64 * 1.8, l.h - 1.0)));
        }
    } else {
        for x0 in [6.0, 31.0] {
            m.boxx(BLACK, [x0, 30.5, -3.0], [x0 + 19.5, 35.0, 0.0]);
            m.boxx(Mat::Gold, [x0 + 0.5, 32.4, -3.02], [x0 + 19.0, 33.1, -3.0]);
        }
        for (i, n) in names.iter().enumerate() {
            let x = if i < 16 { 7.0 + i as f64 * 1.15 } else { 32.0 + (i - 16) as f64 * 1.15 };
            pins.push((n.clone(), p(x, 32.75)));
        }
    }
}

/// Keyboard computers: the case with keys, ports and the GPIO header at the back.
fn keyboard(s: Sbc) -> DevicePart {
    let (w, d) = if s == Opi800 { (300.0, 125.0) } else { (286.0, 122.0) };
    let (case, key, accent) = match s {
        Opi800 => ([0.10, 0.10, 0.11], Mat::Rgb(30, 30, 32), Mat::Rgb(240, 120, 30)),
        _ => ([0.94, 0.93, 0.91], Mat::Rgb(245, 244, 242), Mat::Rgb(205, 40, 70)),
    };
    let mut b = Build::new(s.name(), case, 1.2e-6);
    let base = 12.0;
    b.extrude_rect("Gehäuse", P::ZERO, p(w, d), base, case);
    let mut m = Model::default();
    let case_mat = Mat::Rgb((case[0] * 255.0) as u8, (case[1] * 255.0) as u8, (case[2] * 255.0) as u8);
    // sloped top: higher at the back (y = 0)
    let (zb, zf) = (21.0, 13.0);
    let zat = |y: f64| zb + (zf - zb) * y / d;
    m.quad(case_mat, [0.0, 0.0, base], [w, 0.0, base], [w, 0.0, zb], [0.0, 0.0, zb]);
    m.poly(case_mat, &[[0.0, 0.0, zb], [w, 0.0, zb], [w, d, zf], [0.0, d, zf]]);
    m.poly(case_mat, &[[0.0, d, base], [0.0, d, zf], [w, d, zf], [w, d, base]]);
    m.poly(case_mat, &[[0.0, 0.0, base], [0.0, 0.0, zb], [0.0, d, zf], [0.0, d, base]]);
    m.poly(case_mat, &[[w, 0.0, base], [w, d, base], [w, d, zf], [w, 0.0, zb]]);
    // keys: 6 rows, the space bar in the front row
    let (kx0, ky0, pitch) = (14.0, 24.0, 19.0);
    for row in 0..6 {
        let y = ky0 + row as f64 * pitch * 0.85;
        let n = if row == 5 { 0 } else { 14 };
        for k in 0..n {
            let x = kx0 + k as f64 * pitch;
            let kw = if (row == 1 && k == 0) || (row == 3 && k == 0) { 1.0 } else { 1.0 };
            let z = zat(y) + 0.0;
            let mat = if row == 0 && k == 0 { accent } else { key };
            m.body(mat, [x, y, z], [x + pitch * kw - 2.0, y + pitch * 0.85 - 2.0, z + 4.0], 0.8);
        }
        if row == 5 {
            for (x0, x1) in [(kx0, kx0 + 55.0), (kx0 + 60.0, kx0 + 175.0), (kx0 + 180.0, kx0 + 262.0)] {
                let z = zat(y);
                m.body(key, [x0, y, z], [x1 - 2.0, y + pitch * 0.85 - 2.0, z + 4.0], 0.8);
            }
        }
    }
    // ports along the back (they open towards -y)
    let back = |x: f64| (p(x, -0.3), p(0.0, -1.0));
    let z = 3.0;
    let ports: Vec<(Receptacle, f64)> = match s {
        Opi800 => vec![(Receptacle::Hdmi, 120.0), (Receptacle::TypeA(USB3), 145.0), (Receptacle::TypeA(USB2), 165.0), (Receptacle::TypeC, 185.0), (Receptacle::Audio, 200.0)],
        Pi500 => vec![(Receptacle::MicroHdmi, 112.0), (Receptacle::MicroHdmi, 124.0), (Receptacle::TypeC, 138.0), (Receptacle::TypeA(USB2), 158.0), (Receptacle::TypeA(USB3), 176.0), (Receptacle::TypeA(USB3), 194.0), (Receptacle::Rj45, 216.0)],
        _ => vec![(Receptacle::MicroHdmi, 112.0), (Receptacle::MicroHdmi, 124.0), (Receptacle::TypeC, 138.0), (Receptacle::TypeA(USB2), 158.0), (Receptacle::TypeA(USB3), 176.0), (Receptacle::TypeA(USB3), 194.0), (Receptacle::Rj45, 216.0)],
    };
    for (r, x) in ports {
        let (at, d) = back(x);
        receptacle(&mut m, r, at, d, z);
    }
    // microSD slot and the GPIO header: horizontal pins in a recess at the back
    m.boxx(Mat::Rgb(20, 20, 22), [92.0, -0.05, 5.0], [104.0, 0.0, 6.5]);
    let pins_n = if s == Opi800 { 26 } else { 40 };
    let x0 = 30.0;
    let mut pins = vec![];
    m.boxx(Mat::Rgb(25, 25, 27), [x0 - 2.0, -0.05, 3.5], [x0 + (pins_n / 2) as f64 * 2.54 + 0.5, 0.0, 10.0]);
    for i in 0..pins_n {
        let x = x0 + (i / 2) as f64 * 2.54;
        let zz = if i % 2 == 0 { 5.5 } else { 8.04 };
        m.boxx(Mat::Gold, [x - 0.32, -6.0, zz - 0.32], [x + 0.32, 2.0, zz + 0.32]);
        let name = if s == Opi800 {
            match i + 1 {
                1 | 17 => "3V3".to_string(),
                2 | 4 => "5V".into(),
                6 | 9 | 14 | 20 | 25 => "GND".into(),
                n => format!("PIN{n}"),
            }
        } else {
            PI_J8[i].to_string()
        };
        pins.push((name, [x, -6.0, zz]));
    }
    let silk = if s == Opi800 { Mat::Rgb(240, 120, 30) } else { Mat::Rgb(205, 40, 70) };
    text(&mut m, silk, s.name().to_uppercase().as_str(), p(w - 60.0, 12.0), 3.0, 0.0, zat(12.0));
    b.model(&m);
    let (off, on) = LedColor::Green.rgb();
    let mut led = Model::default();
    led.boxx(Mat::White, [w - 20.0, 8.0, zat(8.0)], [w - 17.0, 10.0, zat(8.0) + 0.4]);
    b.lamp("PWR", off, on, &led);
    for (n, q) in pins {
        b.pin(&n, q);
    }
    b.done(Kind::Board(BoardKind::Sbc(s)))
}

/// Geometry and pins of a single-board computer.
pub fn build(s: Sbc) -> DevicePart {
    match s {
        Pi400 | Pi500 | Opi800 => keyboard(s),
        _ if s.pico() => super::pico_variant(Some(s)),
        _ => board(s),
    }
}
