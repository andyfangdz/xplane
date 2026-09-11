//! Safe owned snapshots of X-Plane's FMS entries. Calls remain on the SDK thread.
use std::ptr;
use xplane_sdk_sys::*;

#[derive(Debug, Clone)]
pub struct FmsEntry {
    pub identifier: String,
    pub latitude: f32,
    pub longitude: f32,
}
pub fn fms_entries(plan: i32) -> Vec<FmsEntry> {
    // SAFETY: valid plan indices are checked before invoking the SDK.
    if !(0..4).contains(&plan) {
        return Vec::new();
    }
    let count = unsafe { XPLMCountFMSFlightPlanEntries(plan) };
    (0..count)
        .map(|i| {
            let mut id = [0_i8; 256];
            let mut latitude = 0.0;
            let mut longitude = 0.0;
            // SAFETY: buffers satisfy SDK sizes; unused output pointers may be null.
            unsafe {
                XPLMGetFMSFlightPlanEntryInfo(
                    plan,
                    i,
                    ptr::null_mut(),
                    id.as_mut_ptr(),
                    ptr::null_mut(),
                    ptr::null_mut(),
                    &mut latitude,
                    &mut longitude,
                )
            };
            let identifier = crate::plugin::read_c_buffer(&id);
            FmsEntry {
                identifier,
                latitude,
                longitude,
            }
        })
        .collect()
}
pub fn fms_destination(plan: i32) -> i32 {
    if !(0..4).contains(&plan) {
        return -1;
    }
    // SAFETY: plan was range checked.
    unsafe { XPLMGetDestinationFMSFlightPlanEntry(plan) }
}
pub fn set_fms_destination(plan: i32, index: i32) -> bool {
    if !(0..4).contains(&plan) || index < 0 {
        return false;
    }
    // SAFETY: validate the index against the current plan on the same thread.
    unsafe {
        if index >= XPLMCountFMSFlightPlanEntries(plan) {
            return false;
        }
        XPLMSetDestinationFMSFlightPlanEntry(plan, index)
    };
    true
}
/// Returns whether the request was dispatched. XPLM reports no load result;
/// callers must read the resulting entries before activating navigation.
pub fn load_fms_plan(plan: i32, text: &str) -> bool {
    if !(0..4).contains(&plan) {
        return false;
    }
    let Ok(size) = u32::try_from(text.len()) else {
        return false;
    };
    let text = crate::c_string(text);
    // SAFETY: sized, NUL-terminated buffer remains live through the SDK call.
    unsafe { XPLMLoadFMSFlightPlan(plan, text.as_ptr(), size) };
    true
}
