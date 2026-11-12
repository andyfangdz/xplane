//! SDK entry points using xplane-sdk-sys ABI types. Keep behavior in safe
//! state/core helpers; unsafe blocks only adapt SDK-owned buffers and strings.
#![allow(unused_variables)]
use crate::{access, state::*};
use xphost_core::*;
use xplane_plugin::Bounds;
use xplane_sdk_sys::*;
/// # Safety
/// Pointers, buffers, callbacks and refcons must satisfy the X-Plane SDK contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMFindDataRef(
    inDataRefName: *const ::std::os::raw::c_char,
) -> XPLMDataRef {
    // SAFETY: SDK caller provides live strings and correctly sized output buffers.
    unsafe {
        if inDataRefName.is_null() {
            return std::ptr::null_mut();
        }
        state()
            .store
            .lookup(&string(inDataRefName))
            .map(handle)
            .unwrap_or(std::ptr::null_mut())
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMIsDataRefGood(inDataRef: XPLMDataRef) -> ::std::os::raw::c_int {
    state()
        .store
        .entries
        .get(index(inDataRef))
        .is_some_and(|e| e.alive) as i32
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMGetDataRefTypes(inDataRef: XPLMDataRef) -> XPLMDataTypeID {
    state()
        .store
        .entries
        .get(index(inDataRef))
        .filter(|e| e.alive)
        .map(|e| e.types)
        .unwrap_or(0)
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMCanWriteDataRef(inDataRef: XPLMDataRef) -> ::std::os::raw::c_int {
    state()
        .store
        .entries
        .get(index(inDataRef))
        .is_some_and(|e| e.alive && e.writable) as i32
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMGetDatai(inDataRef: XPLMDataRef) -> ::std::os::raw::c_int {
    access::scalar(index(inDataRef)) as i32
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMGetDataf(inDataRef: XPLMDataRef) -> f32 {
    access::scalar(index(inDataRef)) as f32
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMGetDatad(inDataRef: XPLMDataRef) -> f64 {
    access::scalar(index(inDataRef))
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMSetDatai(inDataRef: XPLMDataRef, inValue: ::std::os::raw::c_int) {
    access::set_scalar(index(inDataRef), inValue as f64);
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMSetDataf(inDataRef: XPLMDataRef, inValue: f32) {
    access::set_scalar(index(inDataRef), inValue as f64);
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMSetDatad(inDataRef: XPLMDataRef, inValue: f64) {
    access::set_scalar(index(inDataRef), inValue);
}
/// # Safety
/// Pointers, buffers, callbacks and refcons must satisfy the X-Plane SDK contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMGetDatavi(
    inDataRef: XPLMDataRef,
    outValues: *mut ::std::os::raw::c_int,
    inOffset: ::std::os::raw::c_int,
    inMax: ::std::os::raw::c_int,
) -> ::std::os::raw::c_int {
    // SAFETY: SDK caller provides live strings and correctly sized output buffers.
    unsafe {
        if inOffset < 0 || inMax < 0 || inMax as usize > MAX_ELEMENTS {
            return 0;
        }
        let out = if outValues.is_null() {
            None
        } else {
            Some(std::slice::from_raw_parts_mut(
                outValues.cast::<i32>(),
                inMax as usize,
            ))
        };
        access::get_ints(index(inDataRef), out, inOffset as usize) as i32
    }
}
/// # Safety
/// Pointers, buffers, callbacks and refcons must satisfy the X-Plane SDK contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMGetDatavf(
    inDataRef: XPLMDataRef,
    outValues: *mut f32,
    inOffset: ::std::os::raw::c_int,
    inMax: ::std::os::raw::c_int,
) -> ::std::os::raw::c_int {
    // SAFETY: SDK caller provides live strings and correctly sized output buffers.
    unsafe {
        if inOffset < 0 || inMax < 0 || inMax as usize > MAX_ELEMENTS {
            return 0;
        }
        let out = if outValues.is_null() {
            None
        } else {
            Some(std::slice::from_raw_parts_mut(
                outValues.cast::<f32>(),
                inMax as usize,
            ))
        };
        access::get_floats(index(inDataRef), out, inOffset as usize) as i32
    }
}
/// # Safety
/// Pointers, buffers, callbacks and refcons must satisfy the X-Plane SDK contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMGetDatab(
    inDataRef: XPLMDataRef,
    outValue: *mut ::std::os::raw::c_void,
    inOffset: ::std::os::raw::c_int,
    inMaxBytes: ::std::os::raw::c_int,
) -> ::std::os::raw::c_int {
    // SAFETY: SDK caller provides live strings and correctly sized output buffers.
    unsafe {
        if inOffset < 0 || inMaxBytes < 0 || inMaxBytes as usize > MAX_ELEMENTS {
            return 0;
        }
        let out = if outValue.is_null() {
            None
        } else {
            Some(std::slice::from_raw_parts_mut(
                outValue.cast::<u8>(),
                inMaxBytes as usize,
            ))
        };
        access::get_bytes(index(inDataRef), out, inOffset as usize) as i32
    }
}
/// # Safety
/// Pointers, buffers, callbacks and refcons must satisfy the X-Plane SDK contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMSetDatavi(
    inDataRef: XPLMDataRef,
    inValues: *mut ::std::os::raw::c_int,
    inoffset: ::std::os::raw::c_int,
    inCount: ::std::os::raw::c_int,
) {
    // SAFETY: SDK caller provides live strings and correctly sized output buffers.
    unsafe {
        if inValues.is_null() || inoffset < 0 || inCount < 0 || inCount as usize > MAX_ELEMENTS {
            return;
        }
        let input = std::slice::from_raw_parts(inValues.cast::<i32>(), inCount as usize);
        access::set_ints(index(inDataRef), input, inoffset as usize);
    }
}
/// # Safety
/// Pointers, buffers, callbacks and refcons must satisfy the X-Plane SDK contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMSetDatavf(
    inDataRef: XPLMDataRef,
    inValues: *mut f32,
    inoffset: ::std::os::raw::c_int,
    inCount: ::std::os::raw::c_int,
) {
    // SAFETY: SDK caller provides live strings and correctly sized output buffers.
    unsafe {
        if inValues.is_null() || inoffset < 0 || inCount < 0 || inCount as usize > MAX_ELEMENTS {
            return;
        }
        let input = std::slice::from_raw_parts(inValues.cast::<f32>(), inCount as usize);
        access::set_floats(index(inDataRef), input, inoffset as usize);
    }
}
/// # Safety
/// Pointers, buffers, callbacks and refcons must satisfy the X-Plane SDK contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMSetDatab(
    inDataRef: XPLMDataRef,
    inValue: *mut ::std::os::raw::c_void,
    inOffset: ::std::os::raw::c_int,
    inLength: ::std::os::raw::c_int,
) {
    // SAFETY: SDK caller provides live strings and correctly sized output buffers.
    unsafe {
        if inValue.is_null() || inOffset < 0 || inLength < 0 || inLength as usize > MAX_ELEMENTS {
            return;
        }
        let input = std::slice::from_raw_parts(inValue.cast::<u8>(), inLength as usize);
        access::set_bytes(index(inDataRef), input, inOffset as usize);
    }
}
/// # Safety
/// Pointers, buffers, callbacks and refcons must satisfy the X-Plane SDK contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMRegisterDataAccessor(
    inDataName: *const ::std::os::raw::c_char,
    inDataType: XPLMDataTypeID,
    inIsWritable: ::std::os::raw::c_int,
    inReadInt: XPLMGetDatai_f,
    inWriteInt: XPLMSetDatai_f,
    inReadFloat: XPLMGetDataf_f,
    inWriteFloat: XPLMSetDataf_f,
    inReadDouble: XPLMGetDatad_f,
    inWriteDouble: XPLMSetDatad_f,
    inReadIntArray: XPLMGetDatavi_f,
    inWriteIntArray: XPLMSetDatavi_f,
    inReadFloatArray: XPLMGetDatavf_f,
    inWriteFloatArray: XPLMSetDatavf_f,
    inReadData: XPLMGetDatab_f,
    inWriteData: XPLMSetDatab_f,
    inReadRefcon: *mut ::std::os::raw::c_void,
    inWriteRefcon: *mut ::std::os::raw::c_void,
) -> XPLMDataRef {
    // SAFETY: SDK caller provides live strings and correctly sized output buffers.
    unsafe {
        let name = string(inDataName);
        if name.is_empty() {
            return std::ptr::null_mut();
        }
        let mut s = state();
        if s.store.find(&name).is_some() {
            return std::ptr::null_mut();
        }
        let id = s
            .store
            .insert(Entry::new(&name, inDataType, inIsWritable != 0, true));
        s.accessors.insert(
            id,
            Accessor {
                gi: inReadInt,
                si: inWriteInt,
                gf: inReadFloat,
                sf: inWriteFloat,
                gd: inReadDouble,
                sd: inWriteDouble,
                gvi: inReadIntArray,
                svi: inWriteIntArray,
                gvf: inReadFloatArray,
                svf: inWriteFloatArray,
                gb: inReadData,
                sb: inWriteData,
                read: inReadRefcon as usize,
                write: inWriteRefcon as usize,
            },
        );
        handle(id)
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMUnregisterDataAccessor(inDataRef: XPLMDataRef) {
    let mut s = state();
    if let Some(e) = s.store.entries.get_mut(index(inDataRef))
        && e.owned
    {
        e.alive = false;
        s.accessors.remove(&index(inDataRef));
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMGetElapsedTime() -> f32 {
    state().now as f32
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMGetCycleNumber() -> ::std::os::raw::c_int {
    state().frame as i32
}
/// # Safety
/// The callback and refcon remain valid until the loop is unregistered.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMRegisterFlightLoopCallback(
    inFlightLoop: XPLMFlightLoop_f,
    inInterval: f32,
    inRefcon: *mut ::std::os::raw::c_void,
) {
    if inFlightLoop.is_none() {
        return;
    }
    let mut s = state();
    let mut schedule = Schedule::default();
    schedule.set(inInterval, s.now, s.frame);
    s.loops.push(FlightLoop {
        callback: inFlightLoop,
        refcon: inRefcon as usize,
        schedule,
    });
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMUnregisterFlightLoopCallback(
    inFlightLoop: XPLMFlightLoop_f,
    inRefcon: *mut ::std::os::raw::c_void,
) {
    for l in &mut state().loops {
        if l.callback.map(|f| f as usize) == inFlightLoop.map(|f| f as usize)
            && l.refcon == inRefcon as usize
        {
            l.schedule.dead = true;
            l.schedule.scheduled = false;
        }
    }
}
/// # Safety
/// Pointers, buffers, callbacks and refcons must satisfy the X-Plane SDK contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMCreateFlightLoop(
    inParams: *mut XPLMCreateFlightLoop_t,
) -> XPLMFlightLoopID {
    // SAFETY: SDK caller provides live strings and correctly sized output buffers.
    unsafe {
        if inParams.is_null()
            || (*inParams).structSize < std::mem::size_of::<XPLMCreateFlightLoop_t>() as i32
        {
            return std::ptr::null_mut();
        }
        let p = &*inParams;
        if p.callbackFunc.is_none() {
            return std::ptr::null_mut();
        }
        let mut s = state();
        let id = s.loops.len();
        s.loops.push(FlightLoop {
            callback: p.callbackFunc,
            refcon: p.refcon as usize,
            schedule: Schedule::new(p.phase),
        });
        handle(id)
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMDestroyFlightLoop(inFlightLoopID: XPLMFlightLoopID) {
    if let Some(l) = state().loops.get_mut(index(inFlightLoopID)) {
        l.schedule.dead = true;
        l.schedule.scheduled = false;
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMScheduleFlightLoop(
    inFlightLoopID: XPLMFlightLoopID,
    inInterval: f32,
    inRelativeToNow: ::std::os::raw::c_int,
) {
    let mut s = state();
    let now = s.now;
    let frame = s.frame;
    if let Some(l) = s.loops.get_mut(index(inFlightLoopID)) {
        l.schedule.set(inInterval, now, frame);
    }
}
/// # Safety
/// Pointers, buffers, callbacks and refcons must satisfy the X-Plane SDK contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMDebugString(inString: *const ::std::os::raw::c_char) {
    // SAFETY: SDK caller provides live strings and correctly sized output buffers.
    unsafe {
        state().log_raw(&string(inString));
    }
}
/// # Safety
/// Pointers, buffers, callbacks and refcons must satisfy the X-Plane SDK contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMGetSystemPath(outSystemPath: *mut ::std::os::raw::c_char) {
    // SAFETY: SDK caller provides live strings and correctly sized output buffers.
    unsafe {
        let s = state();
        let mut path = s
            .config
            .as_ref()
            .map(|c| c.system_path.clone())
            .unwrap_or_default();
        if !path.ends_with(['/', '\\']) {
            path.push('\\');
        }
        put(outSystemPath, &path, 512);
    }
}
/// # Safety
/// Pointers, buffers, callbacks and refcons must satisfy the X-Plane SDK contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMFindCommand(inName: *const ::std::os::raw::c_char) -> XPLMCommandRef {
    // SAFETY: SDK caller provides live strings and correctly sized output buffers.
    unsafe {
        if inName.is_null() {
            return std::ptr::null_mut();
        }
        let name = string(inName);
        let id = state().command(&name, "");
        handle(id)
    }
}
/// # Safety
/// Pointers, buffers, callbacks and refcons must satisfy the X-Plane SDK contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMCreateCommand(
    inName: *const ::std::os::raw::c_char,
    inDescription: *const ::std::os::raw::c_char,
) -> XPLMCommandRef {
    // SAFETY: SDK caller provides live strings and correctly sized output buffers.
    unsafe {
        if inName.is_null() {
            return std::ptr::null_mut();
        }
        let name = string(inName);
        let desc = string(inDescription);
        let id = state().command(&name, &desc);
        handle(id)
    }
}
/// # Safety
/// The callback and refcon remain valid until the handler is unregistered.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMRegisterCommandHandler(
    inComand: XPLMCommandRef,
    inHandler: XPLMCommandCallback_f,
    inBefore: ::std::os::raw::c_int,
    inRefcon: *mut ::std::os::raw::c_void,
) {
    if inHandler.is_none() {
        return;
    }
    if let Some(c) = state().commands.get_mut(index(inComand)) {
        c.handlers.push(Handler {
            callback: inHandler,
            before: inBefore != 0,
            refcon: inRefcon as usize,
        });
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMUnregisterCommandHandler(
    inComand: XPLMCommandRef,
    inHandler: XPLMCommandCallback_f,
    inBefore: ::std::os::raw::c_int,
    inRefcon: *mut ::std::os::raw::c_void,
) {
    if let Some(c) = state().commands.get_mut(index(inComand)) {
        c.handlers.retain(|h| {
            !(h.callback.map(|f| f as usize) == inHandler.map(|f| f as usize)
                && h.before == (inBefore != 0)
                && h.refcon == inRefcon as usize)
        });
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMCommandBegin(inCommand: XPLMCommandRef) {
    access::command(index(inCommand), 1);
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMCommandEnd(inCommand: XPLMCommandRef) {
    access::command(index(inCommand), 2);
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMCommandOnce(inCommand: XPLMCommandRef) {
    access::command(index(inCommand), 0);
}
/// # Safety
/// Pointers, buffers, callbacks and refcons must satisfy the X-Plane SDK contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMGetVersions(
    outXPlaneVersion: *mut ::std::os::raw::c_int,
    outXPLMVersion: *mut ::std::os::raw::c_int,
    outHostID: *mut XPLMHostApplicationID,
) {
    // SAFETY: SDK caller provides live strings and correctly sized output buffers.
    unsafe {
        output(outXPlaneVersion, 12100);
        output(outXPLMVersion, 411);
        output(outHostID, 1);
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMGetMyID() -> XPLMPluginID {
    1
}
/// # Safety
/// Pointers, buffers, callbacks and refcons must satisfy the X-Plane SDK contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMGetPluginInfo(
    inPlugin: XPLMPluginID,
    outName: *mut ::std::os::raw::c_char,
    outFilePath: *mut ::std::os::raw::c_char,
    outSignature: *mut ::std::os::raw::c_char,
    outDescription: *mut ::std::os::raw::c_char,
) {
    // SAFETY: SDK caller provides live strings and correctly sized output buffers.
    unsafe {
        let s = state();
        let (name, path, sig, desc) = if inPlugin == 0 {
            (
                "X-Plane",
                "",
                "xplanesdk.simulator",
                "xphost fake simulator",
            )
        } else if inPlugin == 1 {
            (
                s.name.as_str(),
                s.config
                    .as_ref()
                    .map(|c| c.plugin_path.as_str())
                    .unwrap_or(""),
                s.signature.as_str(),
                s.description.as_str(),
            )
        } else {
            ("", "", "", "")
        };
        put(outName, name, 256);
        put(outFilePath, path, 512);
        put(outSignature, sig, 256);
        put(outDescription, desc, 256);
    }
}
/// # Safety
/// Pointers, buffers, callbacks and refcons must satisfy the X-Plane SDK contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMFindPluginBySignature(
    inSignature: *const ::std::os::raw::c_char,
) -> XPLMPluginID {
    // SAFETY: SDK caller provides live strings and correctly sized output buffers.
    unsafe {
        let name = string(inSignature);
        let s = state();
        if !name.is_empty() && name == s.signature {
            1
        } else if name == "xplanesdk.simulator" {
            0
        } else {
            -1
        }
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMIsPluginEnabled(inPluginID: XPLMPluginID) -> ::std::os::raw::c_int {
    if inPluginID == 0 {
        1
    } else if inPluginID == 1 {
        state().enabled as i32
    } else {
        0
    }
}
/// # Safety
/// Pointers, buffers, callbacks and refcons must satisfy the X-Plane SDK contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMGetNthAircraftModel(
    inIndex: ::std::os::raw::c_int,
    outFileName: *mut ::std::os::raw::c_char,
    outPath: *mut ::std::os::raw::c_char,
) {
    // SAFETY: SDK caller provides live strings and correctly sized output buffers.
    unsafe {
        let s = state();
        if let Some(c) = &s.config {
            put(
                outFileName,
                if inIndex == 0 { &c.acf_name } else { "" },
                256,
            );
            put(outPath, if inIndex == 0 { &c.acf_path } else { "" }, 512);
        }
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMFindPluginsMenu() -> XPLMMenuID {
    let mut s = state();
    s.menus.entry(1).or_default();
    handle(1)
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMFindAircraftMenu() -> XPLMMenuID {
    let mut s = state();
    s.menus.entry(2).or_default();
    handle(2)
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMCreateMenu(
    inName: *const ::std::os::raw::c_char,
    inParentMenu: XPLMMenuID,
    inParentItem: ::std::os::raw::c_int,
    inHandler: XPLMMenuHandler_f,
    inMenuRef: *mut ::std::os::raw::c_void,
) -> XPLMMenuID {
    let mut s = state();
    let id = s.handle();
    s.menus.insert(id, Menu::default());
    handle(id)
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMDestroyMenu(inMenuID: XPLMMenuID) {
    if let Some(m) = state().menus.get_mut(&index(inMenuID)) {
        m.dead = true;
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMClearAllMenuItems(inMenuID: XPLMMenuID) {
    if let Some(m) = state().menus.get_mut(&index(inMenuID)) {
        m.items.clear();
    }
}
/// # Safety
/// Pointers, buffers, callbacks and refcons must satisfy the X-Plane SDK contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMAppendMenuItem(
    inMenu: XPLMMenuID,
    inItemName: *const ::std::os::raw::c_char,
    inItemRef: *mut ::std::os::raw::c_void,
    inDeprecatedAndIgnored: ::std::os::raw::c_int,
) -> ::std::os::raw::c_int {
    // SAFETY: SDK caller provides live strings and correctly sized output buffers.
    unsafe {
        let name = string(inItemName);
        let mut s = state();
        let Some(m) = s.menus.get_mut(&index(inMenu)).filter(|m| !m.dead) else {
            return -1;
        };
        let id = m.items.len();
        m.items.push(name);
        id as i32
    }
}
/// # Safety
/// Pointers, buffers, callbacks and refcons must satisfy the X-Plane SDK contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMAppendMenuItemWithCommand(
    inMenu: XPLMMenuID,
    inItemName: *const ::std::os::raw::c_char,
    inCommandToExecute: XPLMCommandRef,
) -> ::std::os::raw::c_int {
    // SAFETY: SDK caller provides live strings and correctly sized output buffers.
    unsafe {
        let name = string(inItemName);
        let mut s = state();
        let Some(m) = s.menus.get_mut(&index(inMenu)).filter(|m| !m.dead) else {
            return -1;
        };
        let id = m.items.len();
        m.items.push(name);
        id as i32
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMAppendMenuSeparator(inMenu: XPLMMenuID) {
    if let Some(m) = state().menus.get_mut(&index(inMenu)) {
        m.items.push("-".into());
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMRemoveMenuItem(inMenu: XPLMMenuID, inIndex: ::std::os::raw::c_int) {
    let mut s = state();
    let Some(m) = s.menus.get_mut(&index(inMenu)) else {
        return;
    };
    if inIndex >= 0 && (inIndex as usize) < m.items.len() {
        m.items.remove(inIndex as usize);
    } else {
        // X-Plane logs this line for the same call; test runners look for it.
        let count = m.items.len();
        s.log_raw(&format!(
            "xphost: XPLMRemoveMenuItem was passed an out of range item index \
             ({inIndex}; the menu has {count} items)\n"
        ));
    }
}
/// # Safety
/// Pointers, buffers, callbacks and refcons must satisfy the X-Plane SDK contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMSetMenuItemName(
    inMenu: XPLMMenuID,
    inIndex: ::std::os::raw::c_int,
    inItemName: *const ::std::os::raw::c_char,
    inDeprecatedAndIgnored: ::std::os::raw::c_int,
) {
    // SAFETY: SDK caller provides live strings and correctly sized output buffers.
    unsafe {
        let name = string(inItemName);
        if let Some(m) = state().menus.get_mut(&index(inMenu))
            && let Some(item) = m.items.get_mut(inIndex as usize)
        {
            *item = name;
        }
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMCheckMenuItem(
    inMenu: XPLMMenuID,
    index: ::std::os::raw::c_int,
    inCheck: XPLMMenuCheck,
) {
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMEnableMenuItem(
    inMenu: XPLMMenuID,
    index: ::std::os::raw::c_int,
    enabled: ::std::os::raw::c_int,
) {
}
/// # Safety
/// Pointers, buffers, callbacks and refcons must satisfy the X-Plane SDK contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMCreateWindowEx(inParams: *mut XPLMCreateWindow_t) -> XPLMWindowID {
    // SAFETY: SDK caller provides live strings and correctly sized output buffers.
    unsafe {
        if inParams.is_null()
            || (*inParams).structSize < std::mem::size_of::<XPLMCreateWindow_t>() as i32
        {
            return std::ptr::null_mut();
        }
        let p = &*inParams;
        let mut s = state();
        let id = s.handle();
        s.windows.insert(
            id,
            Window {
                bounds: Bounds::new(p.left, p.top, p.right, p.bottom),
                visible: p.visible != 0,
                title: String::new(),
            },
        );
        handle(id)
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMDestroyWindow(inWindowID: XPLMWindowID) {
    let mut s = state();
    let id = index(inWindowID);
    s.windows.remove(&id);
    if s.keyboard_focus == Some(id) {
        s.keyboard_focus = None;
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMTakeKeyboardFocus(window: XPLMWindowID) {
    let mut s = state();
    let id = index(window);
    if window.is_null() {
        s.keyboard_focus = None;
    } else if s.windows.contains_key(&id) {
        s.keyboard_focus = Some(id);
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMHasKeyboardFocus(window: XPLMWindowID) -> i32 {
    let s = state();
    i32::from(
        !window.is_null()
            && s.windows.contains_key(&index(window))
            && s.keyboard_focus == Some(index(window)),
    )
}
/// Retains camera registration for lifecycle bookkeeping. No camera is simulated.
#[unsafe(no_mangle)]
pub extern "C" fn XPLMControlCamera(
    duration: XPLMCameraControlDuration,
    callback: XPLMCameraControl_f,
    refcon: *mut std::ffi::c_void,
) {
    state().camera = callback.map(|_| (duration, callback, refcon as usize));
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMDontControlCamera() {
    state().camera = None;
}
/// # Safety
/// Pointers, buffers, callbacks and refcons must satisfy the X-Plane SDK contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMGetScreenSize(
    outWidth: *mut ::std::os::raw::c_int,
    outHeight: *mut ::std::os::raw::c_int,
) {
    // SAFETY: SDK caller provides live strings and correctly sized output buffers.
    unsafe {
        output(outWidth, 1920);
        output(outHeight, 1080);
    }
}
/// # Safety
/// Pointers, buffers, callbacks and refcons must satisfy the X-Plane SDK contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMGetScreenBoundsGlobal(
    outLeft: *mut ::std::os::raw::c_int,
    outTop: *mut ::std::os::raw::c_int,
    outRight: *mut ::std::os::raw::c_int,
    outBottom: *mut ::std::os::raw::c_int,
) {
    // SAFETY: SDK caller provides live strings and correctly sized output buffers.
    unsafe {
        output(outLeft, 0);
        output(outTop, 1080);
        output(outRight, 1920);
        output(outBottom, 0);
    }
}
/// # Safety
/// Pointers, buffers, callbacks and refcons must satisfy the X-Plane SDK contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMGetAllMonitorBoundsGlobal(
    inMonitorBoundsCallback: XPLMReceiveMonitorBoundsGlobal_f,
    inRefcon: *mut ::std::os::raw::c_void,
) {
    // SAFETY: SDK caller provides live strings and correctly sized output buffers.
    unsafe {
        if let Some(cb) = inMonitorBoundsCallback {
            cb(0, 0, 1080, 1920, 0, inRefcon);
        }
    }
}
/// # Safety
/// Pointers, buffers, callbacks and refcons must satisfy the X-Plane SDK contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMGetMouseLocationGlobal(
    outX: *mut ::std::os::raw::c_int,
    outY: *mut ::std::os::raw::c_int,
) {
    // SAFETY: SDK caller provides live strings and correctly sized output buffers.
    unsafe {
        output(outX, 0);
        output(outY, 0);
    }
}
/// # Safety
/// Pointers, buffers, callbacks and refcons must satisfy the X-Plane SDK contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMSetWindowTitle(
    inWindowID: XPLMWindowID,
    inWindowTitle: *const ::std::os::raw::c_char,
) {
    // SAFETY: SDK caller provides live strings and correctly sized output buffers.
    unsafe {
        let name = string(inWindowTitle);
        if let Some(w) = state().windows.get_mut(&index(inWindowID)) {
            w.title = name;
        }
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMSetWindowGeometry(
    inWindowID: XPLMWindowID,
    inLeft: ::std::os::raw::c_int,
    inTop: ::std::os::raw::c_int,
    inRight: ::std::os::raw::c_int,
    inBottom: ::std::os::raw::c_int,
) {
    if let Some(w) = state().windows.get_mut(&index(inWindowID)) {
        w.bounds = Bounds::new(inLeft, inTop, inRight, inBottom);
    }
}
/// # Safety
/// Pointers, buffers, callbacks and refcons must satisfy the X-Plane SDK contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMGetWindowGeometry(
    inWindowID: XPLMWindowID,
    outLeft: *mut ::std::os::raw::c_int,
    outTop: *mut ::std::os::raw::c_int,
    outRight: *mut ::std::os::raw::c_int,
    outBottom: *mut ::std::os::raw::c_int,
) {
    // SAFETY: SDK caller provides live strings and correctly sized output buffers.
    unsafe {
        let b = state()
            .windows
            .get(&index(inWindowID))
            .map(|w| w.bounds)
            .unwrap_or_default();
        output(outLeft, b.left);
        output(outTop, b.top);
        output(outRight, b.right);
        output(outBottom, b.bottom);
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMSetWindowResizingLimits(
    inWindowID: XPLMWindowID,
    inMinWidthBoxels: ::std::os::raw::c_int,
    inMinHeightBoxels: ::std::os::raw::c_int,
    inMaxWidthBoxels: ::std::os::raw::c_int,
    inMaxHeightBoxels: ::std::os::raw::c_int,
) {
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMSetWindowPositioningMode(
    inWindowID: XPLMWindowID,
    inPositioningMode: XPLMWindowPositioningMode,
    inMonitorIndex: ::std::os::raw::c_int,
) {
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMSetWindowIsVisible(
    inWindowID: XPLMWindowID,
    inIsVisible: ::std::os::raw::c_int,
) {
    if let Some(w) = state().windows.get_mut(&index(inWindowID)) {
        w.visible = inIsVisible != 0;
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMGetWindowIsVisible(inWindowID: XPLMWindowID) -> ::std::os::raw::c_int {
    state()
        .windows
        .get(&index(inWindowID))
        .is_some_and(|w| w.visible) as i32
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMWindowIsPoppedOut(inWindowID: XPLMWindowID) -> ::std::os::raw::c_int {
    0
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMWindowIsInVR(inWindowID: XPLMWindowID) -> ::std::os::raw::c_int {
    0
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMSetWindowGeometryVR(
    inWindowID: XPLMWindowID,
    widthBoxels: ::std::os::raw::c_int,
    heightBoxels: ::std::os::raw::c_int,
) {
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMBringWindowToFront(inWindow: XPLMWindowID) {}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMRegisterDrawCallback(
    inCallback: XPLMDrawCallback_f,
    inPhase: XPLMDrawingPhase,
    inWantsBefore: ::std::os::raw::c_int,
    inRefcon: *mut ::std::os::raw::c_void,
) -> ::std::os::raw::c_int {
    state().draws.push((
        inCallback.map(|f| f as usize).unwrap_or(0),
        inPhase,
        inWantsBefore,
        inRefcon as usize,
    ));
    1
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMUnregisterDrawCallback(
    inCallback: XPLMDrawCallback_f,
    inPhase: XPLMDrawingPhase,
    inWantsBefore: ::std::os::raw::c_int,
    inRefcon: *mut ::std::os::raw::c_void,
) -> ::std::os::raw::c_int {
    state().draws.retain(|d| {
        *d != (
            inCallback.map(|f| f as usize).unwrap_or(0),
            inPhase,
            inWantsBefore,
            inRefcon as usize,
        )
    });
    1
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMRegisterAvionicsCallbacksEx(
    inParams: *mut XPLMCustomizeAvionics_t,
) -> XPLMAvionicsID {
    let mut s = state();
    let id = s.handle();
    s.popups.insert(id, false);
    handle(id)
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMUnregisterAvionicsCallbacks(inAvionicsId: XPLMAvionicsID) {
    state().popups.remove(&index(inAvionicsId));
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMCreateAvionicsEx(inParams: *mut XPLMCreateAvionics_t) -> XPLMAvionicsID {
    let mut s = state();
    let id = s.handle();
    s.popups.insert(id, false);
    handle(id)
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMDestroyAvionics(inHandle: XPLMAvionicsID) {
    state().popups.remove(&index(inHandle));
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMIsAvionicsBound(inHandle: XPLMAvionicsID) -> ::std::os::raw::c_int {
    state().popups.contains_key(&index(inHandle)) as i32
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMGetAvionicsBusVoltsRatio(inHandle: XPLMAvionicsID) -> f32 {
    let s = state();
    s.config
        .as_ref()
        .map(|c| c.profile.avionics_ratio(&s.store))
        .unwrap_or(0.)
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMSetAvionicsPopupVisible(
    inHandle: XPLMAvionicsID,
    inVisible: ::std::os::raw::c_int,
) {
    if let Some(p) = state().popups.get_mut(&index(inHandle)) {
        *p = inVisible != 0;
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMIsAvionicsPopupVisible(inHandle: XPLMAvionicsID) -> ::std::os::raw::c_int {
    state()
        .popups
        .get(&index(inHandle))
        .copied()
        .unwrap_or(false) as i32
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMAvionicsNeedsDrawing(inHandle: XPLMAvionicsID) {}
/// # Safety
/// Pointers, buffers, callbacks and refcons must satisfy the X-Plane SDK contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMGenerateTextureNumbers(
    outTextureIDs: *mut ::std::os::raw::c_int,
    inCount: ::std::os::raw::c_int,
) {
    // SAFETY: SDK caller provides live strings and correctly sized output buffers.
    unsafe {
        if outTextureIDs.is_null() || inCount < 0 || inCount as usize > MAX_ELEMENTS {
            return;
        }
        let out = std::slice::from_raw_parts_mut(outTextureIDs, inCount as usize);
        let mut s = state();
        for v in out {
            *v = s.next_texture;
            s.next_texture = s.next_texture.saturating_add(1);
        }
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMBindTexture2d(
    inTextureNum: ::std::os::raw::c_int,
    inTextureUnit: ::std::os::raw::c_int,
) {
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMSetGraphicsState(
    inEnableFog: ::std::os::raw::c_int,
    inNumberTexUnits: ::std::os::raw::c_int,
    inEnableLighting: ::std::os::raw::c_int,
    inEnableAlphaTesting: ::std::os::raw::c_int,
    inEnableAlphaBlending: ::std::os::raw::c_int,
    inEnableDepthTesting: ::std::os::raw::c_int,
    inEnableDepthWriting: ::std::os::raw::c_int,
) {
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMDrawTranslucentDarkBox(
    inLeft: ::std::os::raw::c_int,
    inTop: ::std::os::raw::c_int,
    inRight: ::std::os::raw::c_int,
    inBottom: ::std::os::raw::c_int,
) {
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMDrawString(
    inColorRGB: *mut f32,
    inXOffset: ::std::os::raw::c_int,
    inYOffset: ::std::os::raw::c_int,
    inChar: *const ::std::os::raw::c_char,
    inWordWrapWidth: *mut ::std::os::raw::c_int,
    inFontID: XPLMFontID,
) {
}
/// # Safety
/// Pointers, buffers, callbacks and refcons must satisfy the X-Plane SDK contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMMeasureString(
    inFontID: XPLMFontID,
    inChar: *const ::std::os::raw::c_char,
    inNumChars: ::std::os::raw::c_int,
) -> f32 {
    // SAFETY: SDK caller provides live strings and correctly sized output buffers.
    unsafe {
        if inChar.is_null() {
            return 0.;
        }
        let n = if inNumChars < 0 {
            string(inChar).len()
        } else {
            inNumChars as usize
        };
        n as f32 * if inFontID == 0 { 10. } else { 6. }
    }
}
/// # Safety
/// Pointers, buffers, callbacks and refcons must satisfy the X-Plane SDK contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMWorldToLocal(
    inLatitude: f64,
    inLongitude: f64,
    inAltitude: f64,
    outX: *mut f64,
    outY: *mut f64,
    outZ: *mut f64,
) {
    // SAFETY: SDK caller provides live strings and correctly sized output buffers.
    unsafe {
        let (x, y, z) = state()
            .local_frame()
            .world_to_local(inLatitude, inLongitude, inAltitude);
        output(outX, x);
        output(outY, y);
        output(outZ, z);
    }
}
/// # Safety
/// Pointers, buffers, callbacks and refcons must satisfy the X-Plane SDK contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMLocalToWorld(
    inX: f64,
    inY: f64,
    inZ: f64,
    outLatitude: *mut f64,
    outLongitude: *mut f64,
    outAltitude: *mut f64,
) {
    // SAFETY: SDK caller provides live strings and correctly sized output buffers.
    unsafe {
        let (lat, lon, alt) = state().local_frame().local_to_world(inX, inY, inZ);
        output(outLatitude, lat);
        output(outLongitude, lon);
        output(outAltitude, alt);
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMCreateProbe(inProbeType: XPLMProbeType) -> XPLMProbeRef {
    handle(state().handle())
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMDestroyProbe(inProbe: XPLMProbeRef) {}
/// # Safety
/// Pointers, buffers, callbacks and refcons must satisfy the X-Plane SDK contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMProbeTerrainXYZ(
    inProbe: XPLMProbeRef,
    inX: f32,
    inY: f32,
    inZ: f32,
    outInfo: *mut XPLMProbeInfo_t,
) -> XPLMProbeResult {
    // SAFETY: SDK caller provides live strings and correctly sized output buffers.
    unsafe {
        if outInfo.is_null()
            || (*outInfo).structSize < std::mem::size_of::<XPLMProbeInfo_t>() as i32
        {
            return 1;
        }
        let p = &mut *outInfo;
        p.locationX = inX;
        p.locationY = 0.;
        p.locationZ = inZ;
        p.normalX = 0.;
        p.normalY = 1.;
        p.normalZ = 0.;
        p.velocityX = 0.;
        p.velocityY = 0.;
        p.velocityZ = 0.;
        p.is_wet = 0;
        0
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMGetMagneticVariation(latitude: f64, longitude: f64) -> f32 {
    state().magvar()
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMDegTrueToDegMagnetic(headingDegreesTrue: f32) -> f32 {
    headingDegreesTrue + state().magvar()
}
/// # Safety
/// Pointers, buffers, callbacks and refcons must satisfy the X-Plane SDK contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMLoadObject(inPath: *const ::std::os::raw::c_char) -> XPLMObjectRef {
    // SAFETY: SDK caller provides live strings and correctly sized output buffers.
    unsafe {
        let path = string(inPath);
        if !std::path::Path::new(&path).is_file() {
            state().log_raw(&format!("xphost: XPLMLoadObject missed: {path}\n"));
            return std::ptr::null_mut();
        }
        handle(state().handle())
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMUnloadObject(inObject: XPLMObjectRef) {}
/// # Safety
/// Pointers, buffers, callbacks and refcons must satisfy the X-Plane SDK contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMGetWeatherAtLocation(
    latitude: f64,
    longitude: f64,
    altitude_m: f64,
    out_info: *mut XPLMWeatherInfo_t,
) -> ::std::os::raw::c_int {
    // SAFETY: SDK caller provides live strings and correctly sized output buffers.
    unsafe {
        if out_info.is_null() {
            return 0;
        }
        let size = (*out_info).structSize;
        if size < 4 {
            return 0;
        }
        let mut weather: XPLMWeatherInfo_t = std::mem::zeroed();
        weather.structSize = size;
        weather.temperature_alt = isa_temperature(altitude_m) as f32;
        weather.dewpoint_alt = weather.temperature_alt - 8.;
        weather.pressure_alt = isa_pressure(altitude_m) as f32;
        weather.pressure_sl = 101325.;
        weather.visibility = 32186.88;
        std::ptr::copy_nonoverlapping(
            (&weather as *const XPLMWeatherInfo_t).cast::<u8>(),
            out_info.cast::<u8>(),
            (size as usize).min(std::mem::size_of::<XPLMWeatherInfo_t>()),
        );
        1
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMGetFirstNavAid() -> XPLMNavRef {
    if state().nav.is_empty() { -1 } else { 0 }
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMGetNextNavAid(inNavAidRef: XPLMNavRef) -> XPLMNavRef {
    inNavAidRef
        .checked_add(1)
        .filter(|&i| i >= 0 && (i as usize) < state().nav.len())
        .unwrap_or(-1)
}
/// # Safety
/// Pointers, buffers, callbacks and refcons must satisfy the X-Plane SDK contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMFindNavAid(
    inNameFragment: *const ::std::os::raw::c_char,
    inIDFragment: *const ::std::os::raw::c_char,
    inLat: *mut f32,
    inLon: *mut f32,
    inFrequency: *mut ::std::os::raw::c_int,
    inType: XPLMNavType,
) -> XPLMNavRef {
    // SAFETY: SDK caller provides live strings and correctly sized output buffers.
    unsafe {
        let name = string(inNameFragment);
        let ident = string(inIDFragment);
        let lat = inLat.as_ref().copied();
        let lon = inLon.as_ref().copied();
        let freq = inFrequency.as_ref().copied().unwrap_or(0);
        let s = state();
        s.nav
            .iter()
            .enumerate()
            .filter(|(_, n)| {
                (inType == 0 || n.kind & inType != 0)
                    && n.name.contains(&name)
                    && n.id.contains(&ident)
                    && (freq == 0 || n.freq == freq)
            })
            .min_by(|(i, n), (j, m)| {
                let distance = |i: usize, n: &Nav| {
                    if let (Some(lat), Some(lon)) = (lat, lon) {
                        ((n.lat - lat) as f64).powi(2) + ((n.lon - lon) as f64).powi(2)
                    } else {
                        i as f64
                    }
                };
                distance(*i, n).total_cmp(&distance(*j, m))
            })
            .map(|(i, _)| i as i32)
            .unwrap_or(-1)
    }
}
/// # Safety
/// Pointers, buffers, callbacks and refcons must satisfy the X-Plane SDK contract.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn XPLMGetNavAidInfo(
    inRef: XPLMNavRef,
    outType: *mut XPLMNavType,
    outLatitude: *mut f32,
    outLongitude: *mut f32,
    outHeight: *mut f32,
    outFrequency: *mut ::std::os::raw::c_int,
    outHeading: *mut f32,
    outID: *mut ::std::os::raw::c_char,
    outName: *mut ::std::os::raw::c_char,
    outReg: *mut ::std::os::raw::c_char,
) {
    // SAFETY: SDK caller provides live strings and correctly sized output buffers.
    unsafe {
        let s = state();
        let Some(n) = s.nav.get(inRef as usize) else {
            output(outType, 0);
            return;
        };
        output(outType, n.kind);
        output(outLatitude, n.lat);
        output(outLongitude, n.lon);
        output(outHeight, n.height);
        output(outFrequency, n.freq);
        output(outHeading, n.heading);
        put(outID, &n.id, 32);
        put(outName, &n.name, 256);
        put(outReg, &n.region, 32);
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMGetGPSDestination() -> XPLMNavRef {
    let s = state();
    let p = &s.plans[0];
    let i = if p.direct >= 0 {
        p.direct
    } else if p.displayed >= 0 {
        p.displayed
    } else {
        0
    };
    p.entries.get(i as usize).copied().unwrap_or(-1)
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMCountFMSFlightPlanEntries(
    inFlightPlan: XPLMNavFlightPlan,
) -> ::std::os::raw::c_int {
    state()
        .plans
        .get(inFlightPlan as usize)
        .map(|p| p.entries.len() as i32)
        .unwrap_or(0)
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMSetFMSFlightPlanEntryInfo(
    inFlightPlan: XPLMNavFlightPlan,
    inIndex: ::std::os::raw::c_int,
    inRef: XPLMNavRef,
    inAltitude: ::std::os::raw::c_int,
) {
    if !(0..=10000).contains(&inIndex) {
        return;
    }
    if let Some(p) = state().plans.get_mut(inFlightPlan as usize) {
        let i = inIndex as usize;
        if i >= p.entries.len() {
            p.entries.resize(i + 1, -1);
            p.alts.resize(i + 1, 0);
        }
        p.entries[i] = inRef;
        p.alts[i] = inAltitude;
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMClearFMSFlightPlanEntry(
    inFlightPlan: XPLMNavFlightPlan,
    inIndex: ::std::os::raw::c_int,
) {
    if let Some(p) = state().plans.get_mut(inFlightPlan as usize) {
        let i = inIndex as usize;
        if i < p.entries.len() {
            p.entries.remove(i);
            p.alts.remove(i);
        }
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMSetDisplayedFMSFlightPlanEntry(
    inFlightPlan: XPLMNavFlightPlan,
    inIndex: ::std::os::raw::c_int,
) {
    if let Some(p) = state().plans.get_mut(inFlightPlan as usize) {
        p.displayed = inIndex;
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMSetDirectToFMSFlightPlanEntry(
    inFlightPlan: XPLMNavFlightPlan,
    inIndex: ::std::os::raw::c_int,
) {
    if let Some(p) = state().plans.get_mut(inFlightPlan as usize) {
        p.direct = inIndex;
    }
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMPlayPCMOnBus(
    audioBuffer: *mut ::std::os::raw::c_void,
    bufferSize: u32,
    soundFormat: FMOD_SOUND_FORMAT,
    freqHz: ::std::os::raw::c_int,
    numChannels: ::std::os::raw::c_int,
    loop_: ::std::os::raw::c_int,
    audioType: XPLMAudioBus,
    inCallback: XPLMPCMComplete_f,
    inRefcon: *mut ::std::os::raw::c_void,
) -> *mut FMOD_CHANNEL {
    handle(state().handle())
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMStopAudio(fmod_channel: *mut FMOD_CHANNEL) -> FMOD_RESULT {
    0
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMSetAudioPosition(
    fmod_channel: *mut FMOD_CHANNEL,
    position: *mut FMOD_VECTOR,
    velocity: *mut FMOD_VECTOR,
) -> FMOD_RESULT {
    0
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMSetAudioPitch(
    fmod_channel: *mut FMOD_CHANNEL,
    audio_pitch_hz: f32,
) -> FMOD_RESULT {
    0
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMSetAudioVolume(
    fmod_channel: *mut FMOD_CHANNEL,
    source_volume: f32,
) -> FMOD_RESULT {
    0
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMSetAudioCone(
    fmod_channel: *mut FMOD_CHANNEL,
    inside_angle: f32,
    outside_angle: f32,
    outside_volume: f32,
    orientation: *mut FMOD_VECTOR,
) -> FMOD_RESULT {
    0
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMSetAudioFadeDistance(
    fmod_channel: *mut FMOD_CHANNEL,
    min_fade_distance: f32,
    max_fade_distance: f32,
) -> FMOD_RESULT {
    0
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMCreateInstance(
    obj: XPLMObjectRef,
    datarefs: *mut *const ::std::os::raw::c_char,
) -> XPLMInstanceRef {
    handle(state().handle())
}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMDestroyInstance(instance: XPLMInstanceRef) {}
#[unsafe(no_mangle)]
pub extern "C" fn XPLMInstanceSetPosition(
    instance: XPLMInstanceRef,
    new_position: *const XPLMDrawInfo_t,
    data: *const f32,
) {
}
