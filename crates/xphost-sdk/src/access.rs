//! Callback invocation is the only unsafe operation in dataref dispatch.
//! Snapshot registrations, release the mutex, then enter foreign code. No Rust
//! references into the store survive a callback (which may re-enter the SDK).
use crate::state::*;
use std::ffi::c_void;
use xphost_core::*;
use xphost_native::guard::*;

fn registration(id: usize, write: bool) -> Option<(Entry, Option<Accessor>)> {
    let mut s = state();
    let e = s.store.entries.get(id).filter(|e| e.alive)?.clone();
    if e.owned {
        if write {
            s.plugin_writes += 1;
        } else {
            s.plugin_reads += 1;
        }
    } else if write {
        s.sim_writes += 1;
    } else {
        s.sim_reads += 1;
    }
    let cb = s.accessors.get(&id).copied();
    Some((e, cb))
}
pub fn scalar(id: usize) -> f64 {
    let Some((e, a)) = registration(id, false) else {
        return 0.;
    };
    let Some(a) = a else {
        return e.scalar;
    };
    let (mut d, mut f, mut i) = (0., 0., 0);
    // SAFETY: registration supplies SDK callback/refcon pairs. Output pointers
    // are live stack values. The C guards trap faults without unwinding Rust.
    unsafe {
        let r = a.read as *mut c_void;
        if let Some(cb) = a.gd {
            fault(xp_get_d(cb, r, &mut d));
            return d;
        }
        if let Some(cb) = a.gf {
            fault(xp_get_f(cb, r, &mut f));
            return f as f64;
        }
        if let Some(cb) = a.gi {
            fault(xp_get_i(cb, r, &mut i));
            return i as f64;
        }
    }
    if a.gvf.is_some() {
        let mut v = [0.];
        get_floats(id, Some(&mut v), 0);
        return v[0] as f64;
    }
    if a.gvi.is_some() {
        let mut v = [0];
        get_ints(id, Some(&mut v), 0);
        return v[0] as f64;
    }
    0.
}
pub fn set_scalar(id: usize, value: f64) {
    let Some((e, a)) = registration(id, true) else {
        return;
    };
    if e.owned && !e.writable {
        return;
    }
    let Some(a) = a else {
        if let Some(e) = state().store.entries.get_mut(id) {
            e.scalar = value;
        }
        return;
    };
    // SAFETY: registered callbacks/refcons remain owned by the loaded plugin.
    unsafe {
        let r = a.write as *mut c_void;
        if let Some(cb) = a.sd {
            fault(xp_set_d(cb, r, value));
            return;
        }
        if let Some(cb) = a.sf {
            fault(xp_set_f(cb, r, value as f32));
            return;
        }
        if let Some(cb) = a.si {
            fault(xp_set_i(cb, r, value as i32));
            return;
        }
    }
    if a.svf.is_some() {
        set_floats(id, &[value as f32], 0);
    } else if a.svi.is_some() {
        set_ints(id, &[value as i32], 0);
    }
}

macro_rules! array_access {
    ($get:ident,$set:ident,$ty:ty,$field:ident,$read:ident,$write:ident,$guard_get:ident,$guard_set:ident,$other_field:ident,$other_ty:ty,$other_read:ident,$other_write:ident,$other_get:ident,$other_set:ident) => {
        pub fn $get(id: usize, mut out: Option<&mut [$ty]>, offset: usize) -> usize {
            let Some((e, a)) = registration(id, false) else {
                return 0;
            };
            let Some(a) = a else {
                return match out {
                    Some(out) => read_range(&e.$field, offset, out),
                    None => e.$field.len(),
                };
            };
            let mut got = 0;
            let count = out.as_ref().map(|v| v.len()).unwrap_or(0).min(MAX_ELEMENTS);
            if offset > i32::MAX as usize {
                return 0;
            }
            // SAFETY: callback gets a live, bounded buffer (or null for length).
            // Clamp returned counts before inspecting any buffer contents.
            unsafe {
                if let Some(cb) = a.$read {
                    let p = out
                        .as_mut()
                        .map(|v| v.as_mut_ptr())
                        .unwrap_or(std::ptr::null_mut());
                    fault($guard_get(
                        cb,
                        a.read as *mut c_void,
                        p,
                        offset as i32,
                        count as i32,
                        &mut got,
                    ));
                } else if let Some(cb) = a.$other_read {
                    let mut tmp = vec![0 as $other_ty; count];
                    let p = if out.is_some() {
                        tmp.as_mut_ptr()
                    } else {
                        std::ptr::null_mut()
                    };
                    fault($other_get(
                        cb,
                        a.read as *mut c_void,
                        p,
                        offset as i32,
                        count as i32,
                        &mut got,
                    ));
                    if let Some(out) = out.as_mut() {
                        for (dest, src) in out
                            .iter_mut()
                            .zip(tmp)
                            .take((got.max(0) as usize).min(count))
                        {
                            *dest = src as $ty;
                        }
                    }
                } else {
                    if out.is_none() {
                        return 1;
                    }
                    if offset == 0 && count > 0 {
                        if let Some(out) = out.as_mut() {
                            out[0] = scalar(id) as $ty;
                        }
                        return 1;
                    }
                }
            }
            if out.is_some() {
                (got.max(0) as usize).min(count)
            } else {
                (got.max(0) as usize).min(MAX_ELEMENTS)
            }
        }
        pub fn $set(id: usize, input: &[$ty], offset: usize) {
            if offset
                .checked_add(input.len())
                .is_none_or(|n| n > MAX_ELEMENTS)
            {
                return;
            }
            let Some((e, a)) = registration(id, true) else {
                return;
            };
            if e.owned && !e.writable {
                return;
            }
            let Some(a) = a else {
                if let Some(e) = state().store.entries.get_mut(id) {
                    write_range(&mut e.$field, offset, input);
                }
                return;
            };
            // SAFETY: pass a mutable temporary; plugin setters cannot mutate a
            // shared Rust slice. Foreign code must not retain this buffer.
            unsafe {
                if let Some(cb) = a.$write {
                    let mut tmp = input.to_vec();
                    fault($guard_set(
                        cb,
                        a.write as *mut c_void,
                        tmp.as_mut_ptr(),
                        offset as i32,
                        tmp.len() as i32,
                    ));
                } else if let Some(cb) = a.$other_write {
                    let mut tmp: Vec<$other_ty> = input.iter().map(|&v| v as $other_ty).collect();
                    fault($other_set(
                        cb,
                        a.write as *mut c_void,
                        tmp.as_mut_ptr(),
                        offset as i32,
                        tmp.len() as i32,
                    ));
                } else if offset == 0 && !input.is_empty() {
                    set_scalar(id, input[0] as f64);
                }
            }
        }
    };
}
array_access!(
    get_floats, set_floats, f32, floats, gvf, svf, xp_get_vf, xp_set_vf, ints, i32, gvi, svi,
    xp_get_vi, xp_set_vi
);
array_access!(
    get_ints, set_ints, i32, ints, gvi, svi, xp_get_vi, xp_set_vi, floats, f32, gvf, svf,
    xp_get_vf, xp_set_vf
);

pub fn get_bytes(id: usize, mut out: Option<&mut [u8]>, offset: usize) -> usize {
    let Some((e, a)) = registration(id, false) else {
        return 0;
    };
    let Some(a) = a else {
        return match out {
            Some(out) => read_range(&e.bytes, offset, out),
            None => e.bytes.len(),
        };
    };
    let Some(cb) = a.gb else {
        return 0;
    };
    if offset > i32::MAX as usize {
        return 0;
    }
    let count = out.as_ref().map(|v| v.len()).unwrap_or(0).min(MAX_ELEMENTS);
    let mut got = 0;
    let p = out
        .as_mut()
        .map(|v| v.as_mut_ptr().cast())
        .unwrap_or(std::ptr::null_mut());
    // SAFETY: registered byte callback receives the live output slice or null.
    unsafe {
        fault(xp_get_b(
            cb,
            a.read as *mut c_void,
            p,
            offset as i32,
            count as i32,
            &mut got,
        ));
    }
    (got.max(0) as usize).min(if out.is_some() { count } else { MAX_ELEMENTS })
}
pub fn set_bytes(id: usize, input: &[u8], offset: usize) {
    if offset
        .checked_add(input.len())
        .is_none_or(|n| n > MAX_ELEMENTS)
    {
        return;
    }
    let Some((e, a)) = registration(id, true) else {
        return;
    };
    if e.owned && !e.writable {
        return;
    }
    let Some(a) = a else {
        if let Some(e) = state().store.entries.get_mut(id) {
            write_range(&mut e.bytes, offset, input);
        }
        return;
    };
    if let Some(cb) = a.sb {
        let mut tmp = input.to_vec();
        // SAFETY: plugin setter receives a live, uniquely owned temporary.
        unsafe {
            fault(xp_set_b(
                cb,
                a.write as *mut c_void,
                tmp.as_mut_ptr().cast(),
                offset as i32,
                tmp.len() as i32,
            ));
        }
    }
}
pub fn length(id: usize) -> usize {
    let (types, accessor) = {
        let s = state();
        let Some(e) = s.store.entries.get(id).filter(|e| e.alive) else {
            return 0;
        };
        if !e.owned {
            return e.len();
        }
        (e.types, s.accessors.get(&id).copied().unwrap_or_default())
    };
    if accessor.gvf.is_some() {
        get_floats(id, None, 0)
    } else if accessor.gvi.is_some() {
        get_ints(id, None, 0)
    } else if accessor.gb.is_some() {
        get_bytes(id, None, 0)
    } else if types & (FLOAT_ARRAY | INT_ARRAY | DATA) != 0 {
        0
    } else {
        1
    }
}

pub fn message(id: i32) {
    let cb = state().message;
    if let Some(cb) = cb {
        // SAFETY: host attaches the live plugin entry point until detach.
        unsafe {
            fault(xp_message(cb, 0, id, std::ptr::null_mut()));
        }
    }
}
pub fn dispatch(id: usize, phase: i32) {
    let (name, handlers) = {
        let mut s = state();
        let Some(c) = s.commands.get(id) else {
            return;
        };
        let values = (c.name.clone(), c.handlers.clone());
        s.commands_fired += 1;
        if phase != 1 {
            let now = s.now;
            if let Some(log) = &mut s.command_log {
                use std::io::Write;
                let _ = writeln!(log, "{now:.4}\t{}\t{phase}", values.0);
                let _ = log.flush();
            }
        }
        values
    };
    for before in [true, false] {
        if !before && phase == 0 {
            let mut s = state();
            let paused = s.store.scalar("sim/time/paused");
            match name.as_str() {
                "sim/operation/pause_on" => s.store.set("sim/time/paused", 1.),
                "sim/operation/pause_off" => s.store.set("sim/time/paused", 0.),
                "sim/operation/pause_toggle" => s
                    .store
                    .set("sim/time/paused", if paused == 0. { 1. } else { 0. }),
                "sim/operation/reset_flight" => s.reset_pending = true,
                _ => (),
            }
        }
        for h in handlers.iter().filter(|h| h.before == before) {
            if let Some(cb) = h.callback {
                let mut result = 0;
                // SAFETY: callback/refcon pair is a snapshot, and no locks are held.
                unsafe {
                    fault(xp_command(
                        cb,
                        handle(id),
                        phase,
                        h.refcon as *mut c_void,
                        &mut result,
                    ));
                }
                if result == 0 {
                    return;
                }
            }
        }
    }
}
pub fn command(id: usize, action: i32) {
    let fire = {
        let mut s = state();
        let Some(c) = s.commands.get_mut(id) else {
            return;
        };
        match action {
            1 if !c.down => {
                c.down = true;
                true
            }
            2 if c.down => {
                c.down = false;
                true
            }
            0 => true,
            _ => false,
        }
    };
    if !fire {
        return;
    }
    match action {
        1 => dispatch(id, 0),
        2 => dispatch(id, 2),
        _ => {
            dispatch(id, 0);
            dispatch(id, 2);
        }
    }
    if action != 1 {
        let reset = {
            let mut s = state();
            std::mem::take(&mut s.reset_pending)
        };
        if reset {
            message(103);
        }
    }
}
pub fn step(dt: f64) {
    let (now, frame, held, nloops) = {
        let mut s = state();
        s.dt = dt;
        s.now += dt;
        s.frame += 1;
        let now = s.now;
        for name in [
            "sim/time/total_running_time_sec",
            "sim/time/total_flight_time_sec",
        ] {
            s.store.set(name, now);
        }
        for name in ["sim/time/local_time_sec", "sim/time/zulu_time_sec"] {
            s.store.set(name, (43200. + now) % 86400.);
        }
        for name in [
            "sim/operation/misc/frame_rate_period",
            "sim/time/framerate_period",
        ] {
            s.store.set(name, dt);
        }
        for name in ["sim/time/sim_speed_actual", "sim/time/sim_speed_actual_ogl"] {
            s.store.set(name, 1.);
        }
        (
            now,
            s.frame,
            s.commands
                .iter()
                .enumerate()
                .filter(|(_, c)| c.down)
                .map(|(i, _)| i)
                .collect::<Vec<_>>(),
            s.loops.len(),
        )
    };
    for id in held {
        dispatch(id, 1);
    }
    for phase in [0, 1] {
        if phase == 1 {
            let mut s = state();
            {
                let State { store, config, .. } = &mut *s;
                if let Some(config) = config {
                    config.profile.refresh_indicators(store);
                }
            }
            let f = s.local_frame();
            let previous = s.last_local;
            s.last_local = Some(s.store.refresh(&f, previous));
        }
        for id in 0..nloops {
            let call = {
                let mut s = state();
                let l = &mut s.loops[id];
                if !l.schedule.due(now, frame, phase) {
                    continue;
                }
                let (elapsed, revision) = l.schedule.begin(now, dt);
                (l.callback, l.refcon, elapsed, revision)
            };
            if let Some(cb) = call.0 {
                let mut next = 0.;
                // SAFETY: callback is registered by the live plugin; stack result
                // is valid, and callback may freely re-enter any SDK export.
                let code = unsafe {
                    xp_loop(
                        cb,
                        call.2,
                        dt as f32,
                        frame as i32,
                        call.1 as *mut c_void,
                        &mut next,
                    )
                };
                fault(code);
                let mut s = state();
                s.loops_run += 1;
                let schedule = &mut s.loops[id].schedule;
                if code != 0 {
                    schedule.scheduled = false;
                } else {
                    schedule.finish(next, now, frame, call.3);
                }
            }
        }
    }
}
