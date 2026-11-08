use base64::{engine::general_purpose::STANDARD, Engine};
use serde_json::{json, Value};
use std::{
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::{Duration, Instant},
};
use xphost_core::{Backend, Request};

#[derive(Clone, Debug)]
pub struct RefSpec {
    pub name: String,
    pub index: Option<usize>,
}
impl RefSpec {
    pub fn parse(text: &str) -> Result<Self, String> {
        if text.is_empty() {
            return Err("missing dataref".into());
        }
        if let Some((name, index)) = text.strip_suffix(']').and_then(|s| s.split_once('[')) {
            Ok(Self {
                name: name.into(),
                index: Some(index.parse().map_err(|_| "invalid array index")?),
            })
        } else if text.contains(['[', ']']) {
            Err("invalid dataref index".into())
        } else {
            Ok(Self {
                name: text.into(),
                index: None,
            })
        }
    }
    pub fn number(&self, api: &impl Backend) -> Result<f64, String> {
        let value = api.read(&self.name)?;
        if let Some(arr) = value.as_array() {
            arr.get(self.index.unwrap_or(0))
                .and_then(Value::as_f64)
                .ok_or_else(|| "bad array index".into())
        } else {
            value.as_f64().ok_or_else(|| "not numeric".into())
        }
    }
    pub fn describe(&self, api: &impl Backend) -> String {
        match api.read(&self.name) {
            Ok(v) => {
                if let Some(index) = self.index {
                    v.get(index)
                        .map(ToString::to_string)
                        .unwrap_or_else(|| "(bad index)".into())
                } else if let Some(text) = v.as_str() {
                    let bytes = STANDARD.decode(text).unwrap_or_default();
                    let n = bytes.iter().position(|&v| v == 0).unwrap_or(bytes.len());
                    format!("{:?}", String::from_utf8_lossy(&bytes[..n]))
                } else {
                    v.to_string()
                }
            }
            Err(_) => "(absent)".into(),
        }
    }
}
struct Trace {
    file: File,
    refs: Vec<RefSpec>,
    every: u64,
    left: u64,
}
pub struct Runner<'a, B: Backend> {
    pub api: &'a B,
    pub dt: f64,
    pub fast: bool,
    pub failed: bool,
    pub faulted: bool,
    pub out: PathBuf,
    pub debug_request: String,
    pub debug_result: String,
    pub stop: Arc<AtomicBool>,
    next: Instant,
    start: Instant,
    trace: Option<Trace>,
}
impl<'a, B: Backend> Runner<'a, B> {
    fn require_bytes(&self, name: &str) -> Result<(), String> {
        let refs = self.api.request(Request::Refs)?;
        if refs.as_array().is_some_and(|refs| {
            refs.iter()
                .any(|r| r["name"] == name && r["value_type"] == "data")
        }) {
            Ok(())
        } else {
            Err(format!("expected byte dataref: {name}"))
        }
    }
    pub fn new(api: &'a B, dt: f64, fast: bool, out: PathBuf, stop: Arc<AtomicBool>) -> Self {
        let now = Instant::now();
        Self {
            api,
            dt,
            fast,
            failed: false,
            faulted: false,
            out,
            debug_request: String::new(),
            debug_result: String::new(),
            stop,
            next: now,
            start: now,
            trace: None,
        }
    }
    pub fn step(&mut self) {
        if let Err(error) = self.api.step(self.dt) {
            self.faulted = true;
            println!("xphost: {error}");
            return;
        }
        if let Some(trace) = &mut self.trace {
            trace.left = trace.left.saturating_sub(1);
            if trace.left == 0 {
                trace.left = trace.every;
                self.sample();
            }
        }
        if !self.fast {
            self.next += Duration::from_secs_f64(self.dt);
            let now = Instant::now();
            if self.next > now {
                thread::sleep(self.next - now);
            } else if now.duration_since(self.next) > Duration::from_millis(250) {
                self.next = now;
            }
        }
    }
    pub fn advance(&mut self, seconds: f64) -> Result<(), String> {
        if !seconds.is_finite() || seconds < 0. {
            return Err("invalid wait duration".into());
        }
        for _ in 0..(seconds / self.dt).round() as u64 {
            if self.stop.load(Ordering::Relaxed) || self.faulted {
                break;
            }
            self.step();
        }
        Ok(())
    }
    fn sample(&mut self) {
        if let Some(trace) = &mut self.trace {
            let mut line = format!("{:.4}", self.api.time());
            for r in &trace.refs {
                line.push('\t');
                if let Ok(v) = r.number(self.api) {
                    line.push_str(&v.to_string());
                }
            }
            let _ = writeln!(trace.file, "{line}");
            let _ = trace.file.flush();
        }
    }
    fn instruction(&mut self, line: &str) -> Result<(), String> {
        let words: Vec<_> = line.split_whitespace().collect();
        if words.is_empty() {
            return Ok(());
        }
        let at = |i: usize| {
            words
                .get(i)
                .copied()
                .ok_or_else(|| "missing script argument".to_string())
        };
        let number = |i| {
            at(i)?
                .parse::<f64>()
                .ok()
                .filter(|v| v.is_finite())
                .ok_or_else(|| "invalid script number".to_string())
        };
        let rest = |n| {
            let mut text = line;
            for _ in 0..n {
                text = text.trim_start();
                text = text
                    .find(char::is_whitespace)
                    .map(|i| &text[i..])
                    .unwrap_or("");
            }
            text.trim_start()
        };
        match words[0] {
            "echo" => println!("{}", rest(1)),
            "mark" => println!(
                "mark {}: sim={:.3} wall={:.3}",
                rest(1),
                self.api.time(),
                self.start.elapsed().as_secs_f64()
            ),
            "wait" => self.advance(number(1)?)?,
            "wait_until" => {
                let r = RefSpec::parse(at(1)?)?;
                let op = at(2)?;
                let want = number(3)?;
                let timeout = if words.len() > 4 { number(4)? } else { 60. };
                if timeout < 0. {
                    return Err("negative timeout".into());
                }
                if !matches!(op, "==" | "!=" | ">=" | "<=" | ">" | "<") {
                    return Err("invalid comparison".into());
                }
                let mut matched = false;
                let limit = (timeout / self.dt).round() as u64;
                for i in 0..=limit {
                    if self.stop.load(Ordering::Relaxed) || self.faulted {
                        break;
                    }
                    if let Ok(v) = r.number(self.api) {
                        matched = match op {
                            "==" => v == want,
                            "!=" => v != want,
                            ">=" => v >= want,
                            "<=" => v <= want,
                            ">" => v > want,
                            _ => v < want,
                        };
                        if matched {
                            break;
                        }
                    }
                    if i < limit {
                        self.step();
                    }
                }
                if !matched {
                    return Err(format!("wait_until timed out: {line}"));
                }
            }
            "set" => {
                let r = RefSpec::parse(at(1)?)?;
                let v = number(2)?;
                self.api.write(&r.name, json!(v), r.index.unwrap_or(0))?;
            }
            "sets" => {
                let r = RefSpec::parse(at(1)?)?;
                self.require_bytes(&r.name)?;
                self.api.write(&r.name, json!(rest(2).as_bytes()), 0)?;
            }
            "get" => println!(
                "{} = {}",
                at(1)?,
                RefSpec::parse(at(1)?)?.describe(self.api)
            ),
            "cmd" | "cmd_begin" | "cmd_end" => self.api.command(
                at(1)?,
                match words[0] {
                    "cmd_begin" => 1,
                    "cmd_end" => 2,
                    _ => 0,
                },
            )?,
            "dbg" => {
                if self.debug_request.is_empty() || self.debug_result.is_empty() {
                    return Err("dbg requires --debug-request and --debug-result".into());
                }
                self.require_bytes(&self.debug_request)?;
                self.require_bytes(&self.debug_result)?;
                self.api
                    .write(&self.debug_request, json!(rest(1).as_bytes()), 0)?;
                self.advance(2. * self.dt)?;
                let encoded = self.api.read(&self.debug_result)?;
                let encoded = encoded
                    .as_str()
                    .ok_or("debug result must be a byte dataref")?;
                let bytes = STANDARD.decode(encoded).map_err(|e| e.to_string())?;
                let n = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
                println!(
                    "dbg {} -> {}",
                    rest(1),
                    String::from_utf8_lossy(&bytes[..n])
                );
            }
            "expect" => {
                let r = RefSpec::parse(at(1)?)?;
                let lo = number(2)?;
                let hi = number(3)?;
                let v = r.number(self.api)?;
                if v < lo || v > hi {
                    return Err(format!("expect {} = {v} outside [{lo}, {hi}]", at(1)?));
                }
                println!("expect {} = {v}: PASS", at(1)?);
            }
            "trace_off" => self.trace = None,
            "trace" => {
                let path = self.out.join(at(1)?);
                let every = ((number(2)? / self.dt).round() as u64).max(1);
                let refs = words[3..]
                    .iter()
                    .map(|s| RefSpec::parse(s))
                    .collect::<Result<Vec<_>, _>>()?;
                let mut file = File::create(&path).map_err(|e| e.to_string())?;
                writeln!(file, "t\t{}", words[3..].join("\t")).map_err(|e| e.to_string())?;
                self.trace = Some(Trace {
                    file,
                    refs,
                    every,
                    left: every,
                });
                self.sample();
                println!("trace -> {} every {every} frame(s)", path.display());
            }
            "dump" => {
                let path = self.out.join(at(1)?);
                let mut file = File::create(&path).map_err(|e| e.to_string())?;
                writeln!(file, "dataref\tvalue").map_err(|e| e.to_string())?;
                for name in &words[2..] {
                    writeln!(file, "{name}\t{}", RefSpec::parse(name)?.describe(self.api))
                        .map_err(|e| e.to_string())?;
                }
                println!("dump -> {}", path.display());
            }
            _ => return Err(format!("unknown verb: {}", words[0])),
        }
        Ok(())
    }
    pub fn script(&mut self, path: &Path) {
        match fs::read_to_string(path) {
            Ok(text) => {
                for (i, line) in text.lines().enumerate() {
                    if self.stop.load(Ordering::Relaxed) || self.faulted {
                        break;
                    }
                    if let Err(error) = self.instruction(line.split('#').next().unwrap_or("")) {
                        self.failed = true;
                        println!("script FAIL line {}: {error}", i + 1);
                    }
                }
            }
            Err(error) => {
                self.failed = true;
                println!("script FAIL: {error}");
            }
        }
    }
    pub fn stats(&self) -> Result<Value, String> {
        self.api.request(Request::Stats)
    }
}
