//! Mathcad-style worksheet embedded in the part. One statement per line:
//!
//! * `name := expression`      defines a variable (`name := expression =` also shows it)
//! * `expression =`            evaluates and shows the result
//! * `// text`, `" text`, `# text` comment lines; an empty line is a gap
//!
//! A unit in square brackets after a definition (`F := 1200 [N]`) is kept as a label.
//! Model parameters and the part's mass properties are available as variables, and
//! worksheet results can be handed back to the model as parameters.

use crate::model::expr::{self, Vars};

#[derive(Clone, Debug, PartialEq)]
pub enum LineKind {
    Text,
    Define { name: String, show: bool },
    Eval,
    Empty,
}

#[derive(Clone, Debug)]
pub struct Line {
    pub kind: LineKind,
    pub value: Option<f64>,
    pub unit: String,
    pub error: Option<String>,
}

fn valid_name(n: &str) -> bool {
    let mut c = n.chars();
    matches!(c.next(), Some(ch) if ch.is_alphabetic() || ch == '_') && n.chars().all(|ch| ch.is_alphanumeric() || ch == '_')
}

/// Splits off a trailing `[unit]`.
fn unit_of(s: &str) -> (&str, String) {
    let t = s.trim_end();
    if t.ends_with(']') {
        if let Some(i) = t.rfind('[') {
            return (&t[..i], t[i + 1..t.len() - 1].trim().to_string());
        }
    }
    (t, String::new())
}

/// Evaluates the worksheet top to bottom; `model` holds the model's variables (upper case).
pub fn evaluate(text: &str, model: &Vars) -> (Vec<Line>, Vars) {
    let mut vars: Vars = model.clone();
    let mut defined = Vars::new();
    let mut out = vec![];
    for raw in text.lines() {
        let s = raw.trim();
        if s.is_empty() {
            out.push(Line { kind: LineKind::Empty, value: None, unit: String::new(), error: None });
            continue;
        }
        if s.starts_with("//") || s.starts_with('#') || s.starts_with('"') {
            out.push(Line { kind: LineKind::Text, value: None, unit: String::new(), error: None });
            continue;
        }
        let (body, unit) = unit_of(s);
        let body = body.trim();
        let (body, show) = match body.strip_suffix('=') {
            Some(b) if !b.ends_with(':') && !b.ends_with('<') && !b.ends_with('>') && !b.ends_with('!') && !b.ends_with('=') => (b.trim(), true),
            _ => (body, false),
        };
        if let Some(i) = body.find(":=") {
            let name = body[..i].trim();
            let ex = body[i + 2..].trim();
            if !valid_name(name) {
                out.push(Line { kind: LineKind::Define { name: name.into(), show }, value: None, unit, error: Some(format!("„{}“ ist kein gültiger Name", name)) });
                continue;
            }
            match expr::eval(ex, &vars) {
                Ok(v) => {
                    vars.insert(name.to_uppercase(), v);
                    defined.insert(name.to_string(), v);
                    out.push(Line { kind: LineKind::Define { name: name.into(), show }, value: Some(v), unit, error: None });
                }
                Err(e) => out.push(Line { kind: LineKind::Define { name: name.into(), show }, value: None, unit, error: Some(e) }),
            }
        } else if show {
            match expr::eval(body, &vars) {
                Ok(v) => out.push(Line { kind: LineKind::Eval, value: Some(v), unit, error: None }),
                Err(e) => out.push(Line { kind: LineKind::Eval, value: None, unit, error: Some(e) }),
            }
        } else {
            out.push(Line { kind: LineKind::Eval, value: None, unit, error: Some("Zuweisung mit := oder Auswertung mit = am Ende schreiben (Text mit // beginnen)".into()) });
        }
    }
    (out, defined)
}

pub const TEMPLATES: [(&str, &str); 4] = [
    (
        "Biegebalken (Kragträger)",
        "// Kragträger mit Einzellast am Ende, Rechteckquerschnitt\nF := 500 [N]\nL := 120 [mm]\nb := 20 [mm]\nh := 8 [mm]\nE := 210000 [MPa]\n\nI := b*h^3/12 = [mm^4]\nW := b*h^2/6 = [mm^3]\nM := F*L = [N·mm]\nsigma := M/W = [MPa]\nf := F*L^3/(3*E*I) = [mm]\n// Sicherheit gegen Fließen (S235)\nS := 235/sigma =\n",
    ),
    (
        "Schraubenverbindung (vereinfacht nach VDI 2230)",
        "// Zylinderschraube M8, Festigkeitsklasse 8.8\nd := 8 [mm]\nP := 1.25 [mm]\nRp02 := 640 [MPa]\nmu := 0.12\n\nd2 := d - 0.6495*P = [mm]\nd3 := d - 1.2269*P = [mm]\nAs := pi/4*((d2+d3)/2)^2 = [mm^2]\n// Montagevorspannkraft bei 90 % Ausnutzung (grobe Näherung)\nFM := 0.9*Rp02*As/sqrt(1+3*(1.5*d2/d3*(P/(pi*d2)+1.155*mu))^2) = [N]\nMA := FM*(0.16*P+0.58*d2*mu+mu*0.65*d) = [N·mm]\n",
    ),
    (
        "Druckfeder",
        "// Zylindrische Schraubendruckfeder aus Federstahldraht (Namen ohne Groß-/Kleinschreibung)\nDraht := 1.5 [mm]\nDm := 12 [mm]\nn := 8\nG := 81500 [MPa]\n\nR := G*Draht^4/(8*Dm^3*n) = [N/mm]\ns := 10 [mm]\nF := R*s = [N]\nw := Dm/Draht\nk := (w+0.5)/(w-0.75)\ntau := k*8*Dm*F/(pi*Draht^3) = [MPa]\n",
    ),
    (
        "Masse und Kosten aus dem Modell",
        "// MASSE, VOLUMEN und OBERFLAECHE kommen aus dem Modell\nm := MASSE = [kg]\nV := VOLUMEN/1000 = [cm^3]\nPreis_kg := 4.8 [€/kg]\nMaterialkosten := m*Preis_kg*1.15 = [€]\nLack := OBERFLAECHE/1e6*12 = [€]\n",
    ),
];

/// Number as shown in the worksheet.
pub fn show(v: f64) -> String {
    super::num(v)
}
