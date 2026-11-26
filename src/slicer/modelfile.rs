//! Model files as they come from model sites: STL and OBJ as they are, 3MF (a ZIP with XML
//! meshes, also with the Bambu/Prusa split into several model files) turned into binary STL,
//! and ZIP archives ("Alle Dateien herunterladen") unpacked to the model files inside.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// The file types that can be printed.
pub fn is_model(name: &str) -> bool {
    let l = name.to_lowercase();
    l.ends_with(".stl") || l.ends_with(".3mf") || l.ends_with(".obj")
}

/// Entries of a ZIP archive: name → contents (stored or deflated entries).
pub fn unzip(data: &[u8]) -> Result<Vec<(String, Vec<u8>)>, String> {
    let u16le = |o: usize| -> Result<usize, String> { data.get(o..o + 2).map(|b| u16::from_le_bytes([b[0], b[1]]) as usize).ok_or_else(|| "ZIP-Datei ist beschädigt.".to_string()) };
    let u32le = |o: usize| -> Result<usize, String> { data.get(o..o + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]) as usize).ok_or_else(|| "ZIP-Datei ist beschädigt.".to_string()) };
    // end of central directory: last "PK\5\6" within the final 64 KiB
    let from = data.len().saturating_sub(65_557);
    let eocd = (from..data.len().saturating_sub(21)).rev().find(|&i| data[i..i + 4] == [0x50, 0x4b, 0x05, 0x06]).ok_or("Keine ZIP-Datei.")?;
    let count = u16le(eocd + 10)?;
    let mut at = u32le(eocd + 16)?;
    let mut out = vec![];
    for _ in 0..count {
        if u32le(at)? != 0x0201_4b50 {
            return Err("ZIP-Verzeichnis ist beschädigt.".into());
        }
        let method = u16le(at + 10)?;
        let csize = u32le(at + 20)?;
        let usize_ = u32le(at + 24)?;
        let (nlen, xlen, clen) = (u16le(at + 28)?, u16le(at + 30)?, u16le(at + 32)?);
        let local = u32le(at + 42)?;
        let name = String::from_utf8_lossy(data.get(at + 46..at + 46 + nlen).ok_or("ZIP-Datei ist beschädigt.")?).to_string();
        at += 46 + nlen + xlen + clen;
        if name.ends_with('/') {
            continue;
        }
        if u32le(local)? != 0x0403_4b50 {
            return Err("ZIP-Eintrag ist beschädigt.".into());
        }
        let start = local + 30 + u16le(local + 26)? + u16le(local + 28)?;
        let raw = data.get(start..start + csize).ok_or("ZIP-Eintrag ist abgeschnitten.")?;
        let bytes = match method {
            0 => raw.to_vec(),
            8 => miniz_oxide::inflate::decompress_to_vec_with_limit(raw, usize_.max(1) * 2 + 1024).map_err(|_| format!("{}: lässt sich nicht entpacken.", name))?,
            m => return Err(format!("{}: Packverfahren {} wird nicht unterstützt.", name, m)),
        };
        out.push((name, bytes));
    }
    Ok(out)
}

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let mut rest = tag;
    while let Some(i) = rest.find(name) {
        let before = rest[..i].chars().last();
        let after = &rest[i + name.len()..];
        if before.is_some_and(|c| c.is_whitespace()) {
            let after = after.trim_start();
            if let Some(after) = after.strip_prefix('=') {
                let after = after.trim_start();
                let q = after.chars().next()?;
                if q == '"' || q == '\'' {
                    let end = after[1..].find(q)?;
                    return Some(&after[1..1 + end]);
                }
            }
        }
        rest = &rest[i + name.len()..];
    }
    None
}

type M = [f64; 12];
const ID: M = [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0];

/// 3MF transform "m00 m01 m02 m10 m11 m12 m20 m21 m22 m30 m31 m32" (row vectors).
fn transform(t: Option<&str>) -> M {
    let v: Vec<f64> = t.unwrap_or("").split_whitespace().filter_map(|x| x.parse().ok()).collect();
    if v.len() == 12 {
        let mut m = ID;
        m.copy_from_slice(&v);
        m
    } else {
        ID
    }
}

/// a then b
fn mul(a: &M, b: &M) -> M {
    let mut r = [0.0; 12];
    for i in 0..3 {
        for j in 0..3 {
            r[i * 3 + j] = (0..3).map(|k| a[i * 3 + k] * b[k * 3 + j]).sum();
        }
    }
    for j in 0..3 {
        r[9 + j] = (0..3).map(|k| a[9 + k] * b[k * 3 + j]).sum::<f64>() + b[9 + j];
    }
    r
}

fn apply(m: &M, p: [f64; 3]) -> [f64; 3] {
    [0, 1, 2].map(|j| p[0] * m[j] + p[1] * m[3 + j] + p[2] * m[6 + j] + m[9 + j])
}

#[derive(Default)]
struct Object {
    verts: Vec<[f64; 3]>,
    tris: Vec<[usize; 3]>,
    /// (file, object id, transform)
    comps: Vec<(String, String, M)>,
}

/// Objects of one .model file, and its build items (object id, transform) and unit scale.
fn parse_model(xml: &str, file: &str) -> (HashMap<String, Object>, Vec<(String, M)>, f64) {
    let mut objs: HashMap<String, Object> = HashMap::new();
    let mut items = vec![];
    let mut cur: Option<String> = None;
    let mut scale = 1.0;
    for chunk in xml.split('<').skip(1) {
        let tag = chunk.split('>').next().unwrap_or("");
        let name = tag.split(|c: char| c.is_whitespace() || c == '/').next().unwrap_or("");
        let local = name.rsplit(':').next().unwrap_or(name);
        match local {
            "model" if !name.starts_with('/') => {
                scale = match attr(tag, "unit").unwrap_or("millimeter") {
                    "micron" => 0.001,
                    "centimeter" => 10.0,
                    "inch" => 25.4,
                    "foot" => 304.8,
                    "meter" => 1000.0,
                    _ => 1.0,
                };
            }
            "object" => {
                if let Some(id) = attr(tag, "id") {
                    objs.entry(id.to_string()).or_default();
                    cur = Some(id.to_string());
                }
            }
            "vertex" => {
                if let Some(o) = cur.as_ref().and_then(|c| objs.get_mut(c)) {
                    let g = |k| attr(tag, k).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
                    o.verts.push([g("x"), g("y"), g("z")]);
                }
            }
            "triangle" => {
                if let Some(o) = cur.as_ref().and_then(|c| objs.get_mut(c)) {
                    let g = |k| attr(tag, k).and_then(|v| v.parse::<usize>().ok());
                    if let (Some(a), Some(b), Some(c)) = (g("v1"), g("v2"), g("v3")) {
                        o.tris.push([a, b, c]);
                    }
                }
            }
            "component" => {
                if let Some(o) = cur.as_ref().and_then(|c| objs.get_mut(c)) {
                    let path = attr(tag, "p:path").or_else(|| attr(tag, "path")).map(|p| p.trim_start_matches('/').to_string()).unwrap_or_else(|| file.to_string());
                    if let Some(id) = attr(tag, "objectid") {
                        o.comps.push((path, id.to_string(), transform(attr(tag, "transform"))));
                    }
                }
            }
            "item" => {
                if let Some(id) = attr(tag, "objectid") {
                    items.push((id.to_string(), transform(attr(tag, "transform"))));
                }
            }
            _ => {}
        }
    }
    (objs, items, scale)
}

fn push_stl(out: &mut Vec<u8>, a: [f64; 3], b: [f64; 3], c: [f64; 3]) {
    let u = [b[0] - a[0], b[1] - a[1], b[2] - a[2]];
    let v = [c[0] - a[0], c[1] - a[1], c[2] - a[2]];
    let n = [u[1] * v[2] - u[2] * v[1], u[2] * v[0] - u[0] * v[2], u[0] * v[1] - u[1] * v[0]];
    let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt().max(1e-30);
    for x in [n[0] / l, n[1] / l, n[2] / l].iter().chain(a.iter()).chain(b.iter()).chain(c.iter()) {
        out.extend_from_slice(&(*x as f32).to_le_bytes());
    }
    out.extend_from_slice(&[0, 0]);
}

/// The printable mesh of a 3MF as binary STL (every build item, placed as on the plate).
pub fn threemf_to_stl(data: &[u8]) -> Result<Vec<u8>, String> {
    let files: HashMap<String, Vec<u8>> = unzip(data)?.into_iter().map(|(n, b)| (n.trim_start_matches('/').to_string(), b)).collect();
    let main = files.keys().find(|k| k.to_lowercase() == "3d/3dmodel.model").or_else(|| files.keys().find(|k| k.to_lowercase().ends_with(".model"))).cloned().ok_or("Im 3MF ist kein Modell.")?;
    let mut models: HashMap<String, (HashMap<String, Object>, f64)> = HashMap::new();
    let load = |f: &str, models: &mut HashMap<String, (HashMap<String, Object>, f64)>| -> Vec<(String, M)> {
        if models.contains_key(f) {
            return vec![];
        }
        let Some(b) = files.get(f).or_else(|| files.iter().find(|(k, _)| k.eq_ignore_ascii_case(f)).map(|(_, v)| v)) else { return vec![] };
        let (o, items, s) = parse_model(&String::from_utf8_lossy(b), f);
        models.insert(f.to_string(), (o, s));
        items
    };
    let items = load(&main, &mut models);
    let mut body: Vec<u8> = vec![];
    let mut n = 0u32;
    // (file, object, transform, depth)
    let mut stack: Vec<(String, String, M, u32)> = items.into_iter().rev().map(|(id, m)| (main.clone(), id, m, 0)).collect();
    if stack.is_empty() {
        // no build section: every object as it is
        let ids: Vec<String> = models[&main].0.keys().cloned().collect();
        stack = ids.into_iter().map(|id| (main.clone(), id, ID, 0)).collect();
    }
    while let Some((file, id, m, depth)) = stack.pop() {
        if depth > 16 {
            continue;
        }
        load(&file, &mut models);
        let Some((objs, scale)) = models.get(&file) else { continue };
        let Some(o) = objs.get(&id) else { continue };
        // transforms are in model units too: place first, then scale to millimetres
        let sm = mul(&m, &[*scale, 0.0, 0.0, 0.0, *scale, 0.0, 0.0, 0.0, *scale, 0.0, 0.0, 0.0]);
        for t in &o.tris {
            if let (Some(a), Some(b), Some(c)) = (o.verts.get(t[0]), o.verts.get(t[1]), o.verts.get(t[2])) {
                push_stl(&mut body, apply(&sm, *a), apply(&sm, *b), apply(&sm, *c));
                n += 1;
            }
        }
        for (f, cid, cm) in &o.comps {
            stack.push((f.clone(), cid.clone(), mul(cm, &m), depth + 1));
        }
    }
    if n == 0 {
        return Err("Das 3MF enthält keine Dreiecke.".into());
    }
    let mut out = vec![0u8; 80];
    out[..21].copy_from_slice(b"SolidWork 3MF import ");
    out.extend_from_slice(&n.to_le_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

/// Prepares a downloaded file for import: STL/OBJ as they are, 3MF converted to an STL beside
/// it, ZIP unpacked into a folder beside it. Returns the files that can be opened.
pub fn prepare(path: &Path) -> Result<Vec<PathBuf>, String> {
    let name = path.file_name().map(|s| s.to_string_lossy().to_string()).unwrap_or_default();
    let lower = name.to_lowercase();
    let dir = path.parent().unwrap_or(Path::new("."));
    if lower.ends_with(".stl") || lower.ends_with(".obj") {
        return Ok(vec![path.to_path_buf()]);
    }
    let data = std::fs::read(path).map_err(|e| format!("{}: {}", name, e))?;
    if lower.ends_with(".3mf") {
        let stl = threemf_to_stl(&data)?;
        let out = path.with_extension("stl");
        std::fs::write(&out, stl).map_err(|e| e.to_string())?;
        return Ok(vec![out]);
    }
    if lower.ends_with(".zip") {
        let stem = path.file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| "modell".into());
        let folder = dir.join(&stem);
        std::fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        let mut out = vec![];
        for (n, b) in unzip(&data)? {
            let base = n.rsplit('/').next().unwrap_or(&n).to_string();
            if !is_model(&base) || base.starts_with("._") {
                continue;
            }
            let target = folder.join(&base);
            std::fs::write(&target, &b).map_err(|e| e.to_string())?;
            out.extend(prepare(&target)?);
        }
        if out.is_empty() {
            return Err(format!("{}: keine STL-, 3MF- oder OBJ-Datei im Archiv.", name));
        }
        return Ok(out);
    }
    Err(format!("{}: kein druckbares Modell (STL, 3MF, OBJ oder ZIP).", name))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A ZIP with stored entries, as the tests need it.
    fn zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut out = vec![];
        let mut cd = vec![];
        for (name, data) in entries {
            let off = out.len() as u32;
            let h = |v: &mut Vec<u8>, central: bool| {
                v.extend_from_slice(if central { &[0x50, 0x4b, 1, 2, 20, 0] } else { &[0x50, 0x4b, 3, 4] });
                v.extend_from_slice(&[20, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
                v.extend_from_slice(&(data.len() as u32).to_le_bytes());
                v.extend_from_slice(&(data.len() as u32).to_le_bytes());
                v.extend_from_slice(&(name.len() as u16).to_le_bytes());
                v.extend_from_slice(&[0, 0]);
                if central {
                    v.extend_from_slice(&[0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
                    v.extend_from_slice(&off.to_le_bytes());
                }
                v.extend_from_slice(name.as_bytes());
            };
            h(&mut out, false);
            out.extend_from_slice(data);
            h(&mut cd, true);
        }
        let cd_off = out.len() as u32;
        out.extend_from_slice(&cd);
        out.extend_from_slice(&[0x50, 0x4b, 5, 6, 0, 0, 0, 0]);
        out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        out.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        out.extend_from_slice(&(cd.len() as u32).to_le_bytes());
        out.extend_from_slice(&cd_off.to_le_bytes());
        out.extend_from_slice(&[0, 0]);
        out
    }

    const CUBE: &str = r#"<object id="1" type="model"><mesh><vertices>
        <vertex x="0" y="0" z="0"/><vertex x="10" y="0" z="0"/><vertex x="0" y="10" z="0"/><vertex x="0" y="0" z="10"/>
        </vertices><triangles><triangle v1="0" v2="2" v3="1"/><triangle v1="0" v2="1" v3="3"/><triangle v1="0" v2="3" v3="2"/><triangle v1="1" v2="2" v3="3"/></triangles></mesh></object>"#;

    fn tri_count(stl: &[u8]) -> u32 {
        u32::from_le_bytes([stl[80], stl[81], stl[82], stl[83]])
    }

    #[test]
    fn plain_3mf_with_build_transform() {
        let model = format!(r#"<?xml version="1.0"?><model unit="centimeter" xmlns="http://schemas.microsoft.com/3dmanufacturing/core/2015/02"><resources>{}</resources><build><item objectid="1" transform="1 0 0 0 1 0 0 0 1 5 0 0"/></build></model>"#, CUBE);
        let z = zip(&[("[Content_Types].xml", b"<Types/>"), ("3D/3dmodel.model", model.as_bytes())]);
        let stl = threemf_to_stl(&z).unwrap();
        assert_eq!(tri_count(&stl), 4);
        assert_eq!(stl.len(), 84 + 4 * 50);
        // first triangle, second vertex (x=0,y=10 cm → 100 mm), moved 5 cm in x after scaling
        let f = |o: usize| f32::from_le_bytes([stl[o], stl[o + 1], stl[o + 2], stl[o + 3]]);
        let v1 = 84 + 12 + 12;
        assert_eq!((f(v1), f(v1 + 4)), (50.0, 100.0));
    }

    #[test]
    fn split_3mf_with_components() {
        let main = r#"<model unit="millimeter" xmlns:p="http://schemas.microsoft.com/3dmanufacturing/production/1015/06"><resources><object id="2" type="model"><components><component p:path="/3D/Objects/part.model" objectid="1" transform="1 0 0 0 1 0 0 0 1 0 0 2"/><component p:path="/3D/Objects/part.model" objectid="1"/></components></object></resources><build><item objectid="2"/></build></model>"#;
        let part = format!("<model unit=\"millimeter\"><resources>{}</resources></model>", CUBE);
        let z = zip(&[("3D/3dmodel.model", main.as_bytes()), ("3D/Objects/part.model", part.as_bytes())]);
        assert_eq!(tri_count(&threemf_to_stl(&z).unwrap()), 8);
    }

    #[test]
    fn zip_of_models_is_unpacked() {
        let dir = tempfile::tempdir().unwrap();
        let stl = {
            let mut v = vec![0u8; 80];
            v.extend_from_slice(&0u32.to_le_bytes());
            v
        };
        let z = zip(&[("files/a.stl", &stl), ("files/readme.txt", b"hi"), ("images/b.png", b"x")]);
        let p = dir.path().join("Benchy.zip");
        std::fs::write(&p, z).unwrap();
        let got = prepare(&p).unwrap();
        assert_eq!(got, vec![dir.path().join("Benchy").join("a.stl")]);
        assert!(prepare(&dir.path().join("x.png")).is_err());
    }
}
