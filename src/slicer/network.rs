//! Sending G-code to a printer on the network: Klipper (Moonraker, as used by Mainsail and
//! Fluidd), OctoPrint and Duet/RepRapFirmware. Plain HTTP/1.1 over the standard library –
//! printers on the local network do not use TLS, so no extra dependencies are needed.

use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum HostKind {
    Moonraker,
    OctoPrint,
    Duet,
}

impl HostKind {
    pub const ALL: [HostKind; 3] = [HostKind::Moonraker, HostKind::OctoPrint, HostKind::Duet];
    pub fn name(&self) -> &'static str {
        match self {
            HostKind::Moonraker => "Klipper (Moonraker / Mainsail / Fluidd)",
            HostKind::OctoPrint => "OctoPrint",
            HostKind::Duet => "Duet / RepRapFirmware",
        }
    }
    /// What the key field means for this host.
    pub fn key_label(&self) -> &'static str {
        match self {
            HostKind::Moonraker => "API-Schlüssel (optional)",
            HostKind::OctoPrint => "API-Schlüssel",
            HostKind::Duet => "Passwort (optional)",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Connection {
    pub kind: HostKind,
    /// "192.168.1.50", "m1pro.local:7125" or "http://octopi.local"
    pub host: String,
    /// OctoPrint/Moonraker API key, or the Duet password
    pub key: String,
    /// start printing right after the upload
    pub start: bool,
}

impl Default for Connection {
    fn default() -> Self {
        Connection { kind: HostKind::Moonraker, host: String::new(), key: String::new(), start: true }
    }
}

struct Target {
    host: String,
    port: u16,
    /// path prefix without a trailing slash ("" or "/octoprint")
    base: String,
}

fn parse_host(s: &str) -> Result<Target, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("Keine Adresse des Druckers angegeben.".into());
    }
    if s.starts_with("https://") {
        return Err("HTTPS wird nicht unterstützt – im lokalen Netz die http://-Adresse des Druckers verwenden.".into());
    }
    let s = s.strip_prefix("http://").unwrap_or(s);
    let (hp, base) = match s.find('/') {
        Some(i) => (&s[..i], s[i..].trim_end_matches('/').to_string()),
        None => (s, String::new()),
    };
    let (host, port) = if let Some(rest) = hp.strip_prefix('[') {
        // [IPv6]:port
        let end = rest.find(']').ok_or("Ungültige IPv6-Adresse.")?;
        let port = rest[end + 1..].strip_prefix(':').map(|p| p.parse::<u16>().map_err(|_| "Ungültiger Port.".to_string())).transpose()?;
        (rest[..end].to_string(), port.unwrap_or(80))
    } else {
        match hp.rsplit_once(':') {
            Some((h, p)) => (h.to_string(), p.parse::<u16>().map_err(|_| format!("Ungültiger Port „{}“.", p))?),
            None => (hp.to_string(), 80),
        }
    };
    if host.is_empty() {
        return Err("Keine Adresse des Druckers angegeben.".into());
    }
    Ok(Target { host, port, base })
}

struct Response {
    status: u16,
    body: Vec<u8>,
}

impl Response {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
    fn ok(&self) -> bool {
        (200..300).contains(&self.status)
    }
}

fn dechunk(mut b: &[u8]) -> Vec<u8> {
    let mut out = vec![];
    while let Some(nl) = b.windows(2).position(|w| w == b"\r\n") {
        let line = String::from_utf8_lossy(&b[..nl]);
        let size = usize::from_str_radix(line.split(';').next().unwrap_or("").trim(), 16).unwrap_or(0);
        b = &b[nl + 2..];
        if size == 0 || size > b.len() {
            break;
        }
        out.extend_from_slice(&b[..size]);
        b = &b[(size + 2).min(b.len())..];
    }
    out
}

fn request(t: &Target, method: &str, path: &str, headers: &[(&str, String)], body: &[u8], timeout: Duration) -> Result<Response, String> {
    let addr = (t.host.as_str(), t.port)
        .to_socket_addrs()
        .map_err(|e| format!("Drucker „{}“ nicht gefunden: {}", t.host, e))?
        .next()
        .ok_or_else(|| format!("Drucker „{}“ nicht gefunden.", t.host))?;
    let mut sock = TcpStream::connect_timeout(&addr, Duration::from_secs(5)).map_err(|e| format!("Keine Verbindung zu {}:{}: {}", t.host, t.port, e))?;
    sock.set_read_timeout(Some(timeout)).ok();
    sock.set_write_timeout(Some(timeout)).ok();
    let host_hdr = if t.port == 80 { t.host.clone() } else { format!("{}:{}", t.host, t.port) };
    let mut head = format!("{} {}{} HTTP/1.1\r\nHost: {}\r\nUser-Agent: SolidWork\r\nAccept: */*\r\nConnection: close\r\nContent-Length: {}\r\n", method, t.base, path, host_hdr, body.len());
    for (k, v) in headers {
        head.push_str(&format!("{}: {}\r\n", k, v));
    }
    head.push_str("\r\n");
    sock.write_all(head.as_bytes()).map_err(|e| format!("Senden fehlgeschlagen: {}", e))?;
    for chunk in body.chunks(64 * 1024) {
        sock.write_all(chunk).map_err(|e| format!("Senden fehlgeschlagen: {}", e))?;
    }
    sock.flush().ok();
    let mut raw = vec![];
    sock.read_to_end(&mut raw).map_err(|e| format!("Keine Antwort vom Drucker: {}", e))?;
    let split = raw.windows(4).position(|w| w == b"\r\n\r\n").ok_or("Ungültige Antwort vom Drucker.")?;
    let head = String::from_utf8_lossy(&raw[..split]).into_owned();
    let mut lines = head.lines();
    let status = lines.next().and_then(|l| l.split_whitespace().nth(1)).and_then(|c| c.parse::<u16>().ok()).ok_or("Ungültige Antwort vom Drucker.")?;
    let chunked = lines.any(|l| {
        let l = l.to_ascii_lowercase();
        l.starts_with("transfer-encoding:") && l.contains("chunked")
    });
    let rest = &raw[split + 4..];
    let body = if chunked { dechunk(rest) } else { rest.to_vec() };
    Ok(Response { status, body })
}

fn urlenc(s: &str) -> String {
    let mut o = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            o.push(b as char);
        } else {
            o.push_str(&format!("%{:02X}", b));
        }
    }
    o
}

/// A file name every host accepts.
pub fn safe_name(name: &str) -> String {
    let mut n: String = name.chars().map(|c| if c.is_ascii_alphanumeric() || "._-".contains(c) { c } else { '_' }).collect();
    if !n.to_ascii_lowercase().ends_with(".gcode") {
        n.push_str(".gcode");
    }
    n
}

fn multipart(fields: &[(&str, &str)], file_name: &str, data: &[u8]) -> (String, Vec<u8>) {
    let boundary = format!("----SolidWork{:016x}", data.len() as u64 ^ 0x5D3A_91C7_0B4E_22F1);
    let mut b = vec![];
    b.extend_from_slice(format!("--{}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{}\"\r\nContent-Type: application/octet-stream\r\n\r\n", boundary, file_name).as_bytes());
    b.extend_from_slice(data);
    b.extend_from_slice(b"\r\n");
    for (k, v) in fields {
        b.extend_from_slice(format!("--{}\r\nContent-Disposition: form-data; name=\"{}\"\r\n\r\n{}\r\n", boundary, k, v).as_bytes());
    }
    b.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());
    (format!("multipart/form-data; boundary={}", boundary), b)
}

fn short(s: &str) -> String {
    let s = s.trim();
    let mut o: String = s.chars().take(160).collect();
    if s.chars().count() > 160 {
        o.push('…');
    }
    o
}

fn fail(what: &str, r: &Response) -> String {
    let hint = match r.status {
        401 | 403 => " – API-Schlüssel oder Passwort prüfen.",
        404 => " – stimmt die Art des Druckers (Klipper/OctoPrint/Duet) und die Adresse?",
        409 => " – der Drucker druckt gerade oder ist nicht bereit.",
        413 => " – die Datei ist für den Drucker zu groß (Upload-Grenze erhöhen).",
        _ => "",
    };
    format!("{}: HTTP {}{} {}", what, r.status, hint, short(&r.text()))
}

/// Asks the printer who it is. Ok carries a short description.
pub fn test(c: &Connection) -> Result<String, String> {
    let t = parse_host(&c.host)?;
    let to = Duration::from_secs(8);
    match c.kind {
        HostKind::Moonraker => {
            let mut h = vec![];
            if !c.key.is_empty() {
                h.push(("X-Api-Key", c.key.clone()));
            }
            let r = request(&t, "GET", "/printer/info", &h, &[], to)?;
            if !r.ok() {
                return Err(fail("Moonraker antwortet nicht wie erwartet", &r));
            }
            let v: serde_json::Value = serde_json::from_slice(&r.body).unwrap_or_default();
            let res = &v["result"];
            let state = res["state"].as_str().unwrap_or("?");
            let host = res["hostname"].as_str().unwrap_or(&t.host);
            let ver = res["software_version"].as_str().unwrap_or("");
            Ok(format!("Klipper auf „{}“ – Zustand: {}{}", host, state, if ver.is_empty() { String::new() } else { format!(" ({})", ver) }))
        }
        HostKind::OctoPrint => {
            if c.key.is_empty() {
                return Err("OctoPrint braucht einen API-Schlüssel (OctoPrint ▸ Einstellungen ▸ Application Keys).".into());
            }
            let r = request(&t, "GET", "/api/version", &[("X-Api-Key", c.key.clone())], &[], to)?;
            if !r.ok() {
                return Err(fail("OctoPrint antwortet nicht wie erwartet", &r));
            }
            let v: serde_json::Value = serde_json::from_slice(&r.body).unwrap_or_default();
            Ok(format!("OctoPrint {} erreichbar", v["text"].as_str().or(v["server"].as_str()).unwrap_or("")).trim().to_string())
        }
        HostKind::Duet => {
            let r = request(&t, "GET", &format!("/rr_connect?password={}", urlenc(&c.key)), &[], &[], to)?;
            if r.ok() {
                let v: serde_json::Value = serde_json::from_slice(&r.body).unwrap_or_default();
                if v["err"].as_i64().unwrap_or(0) != 0 {
                    return Err("Duet: falsches Passwort.".into());
                }
                let _ = request(&t, "GET", "/rr_disconnect", &session_hdr(&v), &[], to);
                return Ok(format!("Duet (RepRapFirmware{}) erreichbar", v["boardType"].as_str().map(|b| format!(", {}", b)).unwrap_or_default()));
            }
            // Duet 3 with a single-board computer (DSF)
            let r = request(&t, "GET", &format!("/machine/connect?password={}", urlenc(&c.key)), &[], &[], to)?;
            if r.ok() {
                return Ok("Duet 3 (DSF) erreichbar".into());
            }
            Err(fail("Duet antwortet nicht wie erwartet", &r))
        }
    }
}

fn session_hdr(v: &serde_json::Value) -> Vec<(&'static str, String)> {
    match v["sessionKey"].as_i64() {
        Some(k) => vec![("X-Session-Key", k.to_string())],
        None => vec![],
    }
}

/// Uploads G-code under `name` and, if `c.start`, starts the print. Ok carries a message.
pub fn send(c: &Connection, name: &str, gcode: &[u8]) -> Result<String, String> {
    let t = parse_host(&c.host)?;
    let name = safe_name(name);
    // large files over Wi-Fi: allow a few minutes
    let to = Duration::from_secs(60 + (gcode.len() / (100 * 1024)) as u64);
    let started = if c.start { " und Druck gestartet" } else { "" };
    match c.kind {
        HostKind::Moonraker => {
            let start = if c.start { "true" } else { "false" };
            let (ct, body) = multipart(&[("root", "gcodes"), ("print", start)], &name, gcode);
            let mut h = vec![("Content-Type", ct)];
            if !c.key.is_empty() {
                h.push(("X-Api-Key", c.key.clone()));
            }
            let r = request(&t, "POST", "/server/files/upload", &h, &body, to)?;
            if !r.ok() {
                return Err(fail("Upload zu Klipper fehlgeschlagen", &r));
            }
            let v: serde_json::Value = serde_json::from_slice(&r.body).unwrap_or_default();
            if c.start && v["print_started"].as_bool() == Some(false) {
                return Ok(format!("„{}“ hochgeladen – der Druck konnte nicht starten (Drucker beschäftigt oder nicht bereit).", name));
            }
            Ok(format!("„{}“ an Klipper gesendet{}.", name, started))
        }
        HostKind::OctoPrint => {
            if c.key.is_empty() {
                return Err("OctoPrint braucht einen API-Schlüssel.".into());
            }
            let flag = if c.start { "true" } else { "false" };
            let (ct, body) = multipart(&[("select", flag), ("print", flag)], &name, gcode);
            let r = request(&t, "POST", "/api/files/local", &[("Content-Type", ct), ("X-Api-Key", c.key.clone())], &body, to)?;
            if !r.ok() {
                return Err(fail("Upload zu OctoPrint fehlgeschlagen", &r));
            }
            Ok(format!("„{}“ an OctoPrint gesendet{}.", name, started))
        }
        HostKind::Duet => {
            let path = format!("0:/gcodes/{}", name);
            let short_to = Duration::from_secs(8);
            let r = request(&t, "GET", &format!("/rr_connect?password={}", urlenc(&c.key)), &[], &[], short_to)?;
            if r.ok() {
                let v: serde_json::Value = serde_json::from_slice(&r.body).unwrap_or_default();
                if v["err"].as_i64().unwrap_or(0) != 0 {
                    return Err("Duet: falsches Passwort.".into());
                }
                let mut h = session_hdr(&v);
                h.push(("Content-Type", "application/octet-stream".into()));
                let r = request(&t, "POST", &format!("/rr_upload?name={}", urlenc(&path)), &h, gcode, to)?;
                let res: serde_json::Value = serde_json::from_slice(&r.body).unwrap_or_default();
                if !r.ok() || res["err"].as_i64().unwrap_or(0) != 0 {
                    return Err(fail("Upload zur Duet fehlgeschlagen", &r));
                }
                if c.start {
                    let g = format!("M32 \"{}\"", path);
                    let r = request(&t, "GET", &format!("/rr_gcode?gcode={}", urlenc(&g)), &session_hdr(&v), &[], short_to)?;
                    if !r.ok() {
                        return Err(fail("Hochgeladen, aber der Druck startete nicht", &r));
                    }
                }
                let _ = request(&t, "GET", "/rr_disconnect", &session_hdr(&v), &[], short_to);
                return Ok(format!("„{}“ an die Duet gesendet{}.", name, started));
            }
            // DSF (Duet 3 + single-board computer)
            let r = request(&t, "GET", &format!("/machine/connect?password={}", urlenc(&c.key)), &[], &[], short_to)?;
            if !r.ok() {
                return Err(fail("Duet antwortet nicht", &r));
            }
            let v: serde_json::Value = serde_json::from_slice(&r.body).unwrap_or_default();
            let mut h = vec![];
            if let Some(k) = v["sessionKey"].as_str() {
                h.push(("X-Session-Key", k.to_string()));
            }
            let mut hu = h.clone();
            hu.push(("Content-Type", "application/octet-stream".into()));
            let r = request(&t, "PUT", &format!("/machine/file/{}", urlenc(&path)), &hu, gcode, to)?;
            if !r.ok() {
                return Err(fail("Upload zur Duet fehlgeschlagen", &r));
            }
            if c.start {
                let mut hc = h.clone();
                hc.push(("Content-Type", "text/plain".into()));
                let r = request(&t, "POST", "/machine/code", &hc, format!("M32 \"{}\"", path).as_bytes(), short_to)?;
                if !r.ok() {
                    return Err(fail("Hochgeladen, aber der Druck startete nicht", &r));
                }
            }
            let _ = request(&t, "GET", "/machine/disconnect", &h, &[], short_to);
            Ok(format!("„{}“ an die Duet gesendet{}.", name, started))
        }
    }
}

/// Runs one G-code command on the printer right away (e.g. baby-stepping the Z offset
/// during the first layer).
pub fn run_gcode(c: &Connection, cmd: &str) -> Result<String, String> {
    let t = parse_host(&c.host)?;
    let to = Duration::from_secs(10);
    let r = match c.kind {
        HostKind::Moonraker => {
            let mut h = vec![];
            if !c.key.is_empty() {
                h.push(("X-Api-Key", c.key.clone()));
            }
            request(&t, "POST", &format!("/printer/gcode/script?script={}", urlenc(cmd)), &h, &[], to)?
        }
        HostKind::OctoPrint => {
            if c.key.is_empty() {
                return Err("OctoPrint braucht einen API-Schlüssel.".into());
            }
            let body = serde_json::json!({ "commands": [cmd] }).to_string();
            request(&t, "POST", "/api/printer/command", &[("X-Api-Key", c.key.clone()), ("Content-Type", "application/json".into())], body.as_bytes(), to)?
        }
        HostKind::Duet => {
            let r = request(&t, "GET", &format!("/rr_connect?password={}", urlenc(&c.key)), &[], &[], to)?;
            let v: serde_json::Value = serde_json::from_slice(&r.body).unwrap_or_default();
            let r = request(&t, "GET", &format!("/rr_gcode?gcode={}", urlenc(cmd)), &session_hdr(&v), &[], to)?;
            let _ = request(&t, "GET", "/rr_disconnect", &session_hdr(&v), &[], to);
            r
        }
    };
    if r.ok() {
        Ok(format!("„{}“ gesendet.", cmd))
    } else {
        Err(fail(&format!("„{}“ abgelehnt", cmd), &r))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;
    use std::sync::mpsc;

    /// One-shot HTTP server: answers every request with `reply` and hands the raw requests back.
    fn server(replies: Vec<(u16, &'static str, bool)>) -> (String, mpsc::Receiver<Vec<u8>>) {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = l.local_addr().unwrap();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            for (status, body, chunked) in replies {
                let (mut s, _) = l.accept().unwrap();
                s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
                let mut req = vec![];
                let mut buf = [0u8; 65536];
                // read headers, then Content-Length bytes
                loop {
                    let n = s.read(&mut buf).unwrap();
                    req.extend_from_slice(&buf[..n]);
                    if let Some(p) = req.windows(4).position(|w| w == b"\r\n\r\n") {
                        let head = String::from_utf8_lossy(&req[..p]).to_ascii_lowercase();
                        let len = head.lines().find_map(|l| l.strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap())).unwrap_or(0);
                        while req.len() < p + 4 + len {
                            let n = s.read(&mut buf).unwrap();
                            req.extend_from_slice(&buf[..n]);
                        }
                        break;
                    }
                }
                tx.send(req).unwrap();
                let resp = if chunked {
                    format!("HTTP/1.1 {} X\r\nTransfer-Encoding: chunked\r\n\r\n{:x}\r\n{}\r\n0\r\n\r\n", status, body.len(), body)
                } else {
                    format!("HTTP/1.1 {} X\r\nContent-Length: {}\r\n\r\n{}", status, body.len(), body)
                };
                s.write_all(resp.as_bytes()).unwrap();
            }
        });
        (format!("http://127.0.0.1:{}", addr.port()), rx)
    }

    #[test]
    fn live_gcode_on_moonraker() {
        let (url, rx) = server(vec![(200, r#"{"result":"ok"}"#, false)]);
        let c = Connection { kind: HostKind::Moonraker, host: url, key: "k".into(), start: false };
        run_gcode(&c, "SET_GCODE_OFFSET Z_ADJUST=-0.020 MOVE=1").unwrap();
        let req = String::from_utf8(rx.recv().unwrap()).unwrap();
        assert!(req.starts_with("POST /printer/gcode/script?script=SET_GCODE_OFFSET%20Z_ADJUST%3D-0.020%20MOVE%3D1 "), "{}", req);
        assert!(req.contains("X-Api-Key: k"));
    }

    #[test]
    fn hosts_parse() {
        let t = parse_host("192.168.1.5").unwrap();
        assert_eq!((t.host.as_str(), t.port, t.base.as_str()), ("192.168.1.5", 80, ""));
        let t = parse_host("http://m1.local:7125/").unwrap();
        assert_eq!((t.host.as_str(), t.port, t.base.as_str()), ("m1.local", 7125, ""));
        let t = parse_host("pi.local/octoprint").unwrap();
        assert_eq!((t.port, t.base.as_str()), (80, "/octoprint"));
        let t = parse_host("[fe80::1]:8080").unwrap();
        assert_eq!((t.host.as_str(), t.port), ("fe80::1", 8080));
        assert!(parse_host("https://x").is_err());
        assert!(parse_host("  ").is_err());
        assert_eq!(safe_name("Teil 1 (PLA)"), "Teil_1__PLA_.gcode");
    }

    #[test]
    fn moonraker_upload_starts_print() {
        let (url, rx) = server(vec![(201, r#"{"item":{"path":"a.gcode"},"print_started":true}"#, false)]);
        let c = Connection { kind: HostKind::Moonraker, host: url, key: String::new(), start: true };
        let msg = send(&c, "Teil A", b"G28\nG1 X1\n").unwrap();
        assert!(msg.contains("Druck gestartet"), "{}", msg);
        let req = String::from_utf8(rx.recv().unwrap()).unwrap();
        assert!(req.starts_with("POST /server/files/upload HTTP/1.1"));
        assert!(req.contains("filename=\"Teil_A.gcode\""));
        assert!(req.contains("name=\"print\"\r\n\r\ntrue"));
        assert!(req.contains("G28\nG1 X1\n"));
    }

    #[test]
    fn octoprint_needs_key_and_reports_errors() {
        let c = Connection { kind: HostKind::OctoPrint, host: "127.0.0.1:1".into(), key: String::new(), start: false };
        assert!(send(&c, "a", b"G28").is_err());
        let (url, rx) = server(vec![(403, "Invalid API key", true)]);
        let c = Connection { kind: HostKind::OctoPrint, host: url, key: "ABC".into(), start: false };
        let e = send(&c, "a", b"G28").unwrap_err();
        assert!(e.contains("403") && e.contains("API-Schlüssel"), "{}", e);
        let req = String::from_utf8(rx.recv().unwrap()).unwrap();
        assert!(req.starts_with("POST /api/files/local") && req.contains("X-Api-Key: ABC"));
    }

    #[test]
    fn duet_uploads_then_starts() {
        let (url, rx) = server(vec![
            (200, r#"{"err":0,"sessionKey":42,"boardType":"duetwifi102"}"#, false),
            (200, r#"{"err":0}"#, false),
            (200, "{}", false),
            (200, "{}", false),
        ]);
        let c = Connection { kind: HostKind::Duet, host: url, key: String::new(), start: true };
        send(&c, "x.gcode", b"G28").unwrap();
        let reqs: Vec<String> = (0..4).map(|_| String::from_utf8(rx.recv().unwrap()).unwrap()).collect();
        assert!(reqs[0].starts_with("GET /rr_connect?password="));
        assert!(reqs[1].starts_with("POST /rr_upload?name=0%3A%2Fgcodes%2Fx.gcode") && reqs[1].contains("X-Session-Key: 42"));
        assert!(reqs[2].starts_with("GET /rr_gcode?gcode=M32%20%220%3A%2Fgcodes%2Fx.gcode%22"));
        assert!(reqs[3].starts_with("GET /rr_disconnect"));
    }
}
