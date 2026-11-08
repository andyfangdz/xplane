use crate::{access, state::*};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    ffi::{CString, c_char},
    fs::{self, File},
    io::Write,
    path::Path,
};
use xphost_core::*;

thread_local! {static RESPONSE:RefCell<CString>=RefCell::new(CString::default());}
#[unsafe(no_mangle)]
pub extern "C" fn XpHost_ProtocolVersion() -> u32 {
    PROTOCOL_VERSION
}
/// Private versioned control protocol, owned by the Rust host and fake SDK.
/// # Safety
/// Input is a live NUL-terminated UTF-8 request. Returned bytes live until the
/// next call on this thread; the caller copies them before making another call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XpHost_Call(input: *const c_char) -> *const c_char {
    // SAFETY: the native loader supplies a CString for this synchronous call.
    let input = unsafe { string(input) };
    let result = std::panic::catch_unwind(|| {
        serde_json::from_str::<Request>(&input)
            .map_err(|e| e.to_string())
            .and_then(request)
    })
    .unwrap_or_else(|_| Err("host internal panic".into()));
    let text = match result {
        Ok(value) => json!({"ok":value}),
        Err(error) => json!({"error":error}),
    }
    .to_string();
    RESPONSE.with(|response| {
        let mut response = response.borrow_mut();
        *response = CString::new(text).expect("JSON escapes NUL");
        response.as_ptr()
    })
}
/// # Safety
/// The attached message function stays loaded until detached with None.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XpHost_Attach(callback: Option<xphost_native::guard::Message>) {
    state().message = callback;
}

fn request(request: Request) -> Result<Value, String> {
    match request {
        Request::Init(c) => {
            let c = *c;
            c.profile.validate()?;
            let mut s = state();
            if s.config.is_some() {
                return Err("host is already initialized".into());
            }
            s.log = Some(
                File::create(Path::new(&c.out_dir).join("Log.txt")).map_err(|e| e.to_string())?,
            );
            let mut log = File::create(Path::new(&c.out_dir).join("commands.tsv"))
                .map_err(|e| e.to_string())?;
            writeln!(log, "t\tcommand\tphase").map_err(|e| e.to_string())?;
            s.command_log = Some(log);
            if let Some(path) = &c.datarefs {
                s.store
                    .load_catalog(&fs::read_to_string(path).map_err(|e| e.to_string())?)?;
            }
            if let Some(path) = &c.commands {
                for line in fs::read_to_string(path).map_err(|e| e.to_string())?.lines() {
                    if let Some((name, desc)) = line.split_once(char::is_whitespace) {
                        if name.starts_with("sim/") {
                            s.command(name, desc.trim());
                        }
                    }
                }
            }
            s.store.seed_parked(&c)?;
            fs::write(
                Path::new(&c.out_dir).join("aircraft-profile.json"),
                serde_json::to_vec_pretty(&c.profile).map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
            s.log_raw(&format!("xphost: aircraft profile {}\n", c.profile.name));
            s.config = Some(c);
            s.log_raw("xphost headless host (Rust fake XPLM_64.dll; no X-Plane flight model or rendering)\n");
            Ok(Value::Null)
        }
        Request::Info {
            name,
            signature,
            description,
        } => {
            let mut s = state();
            s.name = name;
            s.signature = signature;
            s.description = description;
            Ok(Value::Null)
        }
        Request::Enabled(v) => {
            state().enabled = v;
            Ok(Value::Null)
        }
        Request::FlightStart(start) => {
            let mut s = state();
            let values = s
                .config
                .as_ref()
                .ok_or("host is not initialized")?
                .profile
                .start_values(start)
                .clone();
            for (name, value) in &values {
                s.store.seed_profile_value(name, value)?;
            }
            Ok(Value::Null)
        }
        Request::Time => Ok(json!(state().now)),
        Request::Step(dt) => {
            if !dt.is_finite() || !(0. ..=1.).contains(&dt) || dt == 0. {
                return Err("invalid frame interval".into());
            }
            access::step(dt);
            if state().faulted {
                Err("plugin fault".into())
            } else {
                Ok(Value::Null)
            }
        }
        Request::Refs => Ok(json!(
            state()
                .store
                .entries
                .iter()
                .enumerate()
                .filter(|(_, e)| e.alive)
                .map(|(id, e)| e.metadata(id))
                .collect::<Vec<_>>()
        )),
        Request::Ref(id) => Ok(state()
            .store
            .entries
            .get(id)
            .filter(|e| e.alive)
            .map(|e| json!(e.metadata(id)))
            .unwrap_or(Value::Null)),
        Request::CommandInfo(id) => Ok(state()
            .commands
            .get(id)
            .map(|c| json!({"id":id,"name":c.name,"description":c.description}))
            .unwrap_or(Value::Null)),
        Request::Commands => Ok(json!(
            state()
                .commands
                .iter()
                .enumerate()
                .map(|(id, c)| json!({"id":id,"name":c.name,"description":c.description}))
                .collect::<Vec<_>>()
        )),
        Request::Read { name } => read(&name),
        Request::Write {
            name,
            value,
            offset,
        } => write(&name, value, offset).map(|_| Value::Null),
        Request::Command { name, action } => {
            let id = state().command(&name, "");
            access::command(id, action);
            Ok(Value::Null)
        }
        Request::Nav(path) => {
            let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
            let nav = parse_nav(&text)?;
            let n = nav.len();
            state().nav = nav;
            Ok(json!(n))
        }
        Request::Stats => {
            let s = state();
            Ok(
                json!({"frames":s.frame,"sim_seconds":s.now,"sim_reads":s.sim_reads,"sim_writes":s.sim_writes,"plugin_reads":s.plugin_reads,"plugin_writes":s.plugin_writes,"commands_fired":s.commands_fired,"loops_run":s.loops_run,"find_misses":s.store.misses.len(),"faulted":s.faulted}),
            )
        }
        Request::Misses => Ok(json!(
            state()
                .store
                .misses
                .iter()
                .map(|s| format!("{s}\n"))
                .collect::<String>()
        )),
        Request::Log(text) => {
            state().log_raw(&format!("xphost: {text}\n"));
            Ok(Value::Null)
        }
    }
}
fn entry(name: &str) -> Result<(usize, i32, bool), String> {
    let s = state();
    let id = s
        .store
        .find(name)
        .ok_or_else(|| format!("absent dataref: {name}"))?;
    let e = &s.store.entries[id];
    Ok((id, e.types, e.writable))
}
pub fn read(name: &str) -> Result<Value, String> {
    let (id, types, _) = entry(name)?;
    let n = access::length(id).min(MAX_ELEMENTS);
    if types & FLOAT_ARRAY != 0 {
        let mut values = vec![0.; n];
        let got = access::get_floats(id, Some(&mut values), 0);
        values.truncate(got);
        return Ok(json!(values));
    }
    if types & INT_ARRAY != 0 {
        let mut values = vec![0; n];
        let got = access::get_ints(id, Some(&mut values), 0);
        values.truncate(got);
        return Ok(json!(values));
    }
    if types & DATA != 0 {
        let mut values = vec![0; n];
        let got = access::get_bytes(id, Some(&mut values), 0);
        values.truncate(got);
        return Ok(json!(STANDARD.encode(values)));
    }
    let value = access::scalar(id);
    Ok(if value.is_finite() {
        json!(value)
    } else {
        json!(0)
    })
}
fn write(name: &str, value: Value, offset: usize) -> Result<(), String> {
    let (id, types, writable) = entry(name)?;
    if !writable && state().store.entries[id].owned {
        return Err("read-only dataref".into());
    }
    if offset > MAX_ELEMENTS {
        return Err("offset too large".into());
    }
    if types & DATA != 0 {
        let bytes = if let Some(text) = value.as_str() {
            STANDARD
                .decode(text)
                .unwrap_or_else(|_| text.as_bytes().to_vec())
        } else if let Some(arr) = value.as_array() {
            arr.iter()
                .map(|v| {
                    v.as_u64()
                        .filter(|&v| v <= 255)
                        .map(|v| v as u8)
                        .ok_or("invalid byte")
                })
                .collect::<Result<Vec<_>, _>>()?
        } else {
            return Err("expected byte array or string".into());
        };
        if offset
            .checked_add(bytes.len())
            .is_none_or(|n| n > MAX_ELEMENTS)
        {
            return Err("data too large".into());
        }
        access::set_bytes(id, &bytes, offset);
        return Ok(());
    }
    let numbers = if let Some(arr) = value.as_array() {
        arr.iter()
            .map(|v| v.as_f64().ok_or("expected numeric array"))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        vec![value.as_f64().ok_or("expected number")?]
    };
    if offset
        .checked_add(numbers.len())
        .is_none_or(|n| n > MAX_ELEMENTS)
    {
        return Err("array too large".into());
    }
    let integral = types & INT_ARRAY != 0 || types & INT != 0 && types & (FLOAT | DOUBLE) == 0;
    for &v in &numbers {
        if !v.is_finite()
            || integral && (v < i32::MIN as f64 || v > i32::MAX as f64 || v.fract() != 0.)
            || types & (FLOAT | FLOAT_ARRAY) != 0 && v.abs() > f32::MAX as f64
        {
            return Err("numeric value out of range".into());
        }
    }
    if types & FLOAT_ARRAY != 0 {
        access::set_floats(
            id,
            &numbers.iter().map(|&v| v as f32).collect::<Vec<_>>(),
            offset,
        );
    } else if types & INT_ARRAY != 0 {
        access::set_ints(
            id,
            &numbers.iter().map(|&v| v as i32).collect::<Vec<_>>(),
            offset,
        );
    } else {
        if numbers.len() != 1 || offset != 0 {
            return Err("expected scalar".into());
        }
        access::set_scalar(id, numbers[0]);
    }
    Ok(())
}
