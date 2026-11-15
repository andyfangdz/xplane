//! `XPLMNavigation.h`: the nav database, FMS flight plans and the GPS.
//!
//! Nav refs, flight plan numbers and entry indices are integers X-Plane looks
//! up itself (module note). [`crate::fms_entries`] and the other FMS helpers
//! at the crate root check the plan and index before calling; these do not.

use std::ffi::CStr;

use xplane_sdk_sys as xp;

/// `XPLMGetFirstNavAid()`.
pub fn first_nav_aid() -> i32 {
    // SAFETY: no arguments; returns a nav database index.
    unsafe { xp::XPLMGetFirstNavAid() }
}

/// `XPLMGetNextNavAid(nav)`.
pub fn next_nav_aid(nav: i32) -> i32 {
    // SAFETY: a plain index; XPLM returns XPLM_NAV_NOT_FOUND for an invalid one.
    unsafe { xp::XPLMGetNextNavAid(nav) }
}

/// The outputs of [`nav_aid_info`]; `None` passes a null pointer.
#[derive(Default)]
pub struct NavAidInfo<'a> {
    pub kind: Option<&'a mut i32>,
    pub latitude: Option<&'a mut f32>,
    pub longitude: Option<&'a mut f32>,
    pub height: Option<&'a mut f32>,
    pub frequency: Option<&'a mut i32>,
    pub heading: Option<&'a mut f32>,
    /// At least 32 bytes.
    pub id: Option<&'a mut [u8]>,
    /// At least 256 bytes.
    pub name: Option<&'a mut [u8]>,
}

fn out<T>(value: Option<&mut T>) -> *mut T {
    value.map_or(std::ptr::null_mut(), std::ptr::from_mut)
}

/// `XPLMGetNavAidInfo(nav, kind, lat, lon, height, freq, heading, id, name, null)`.
///
/// # Panics
/// If `id` is shorter than 32 bytes or `name` than 256 (the SDK's
/// recommended sizes, which bound what it writes).
pub fn nav_aid_info(nav: i32, info: NavAidInfo<'_>) {
    let id = info.id.map_or(std::ptr::null_mut(), |id| {
        assert!(id.len() >= 32, "XPLM nav aid ID buffer too small");
        id.as_mut_ptr().cast()
    });
    let name = info.name.map_or(std::ptr::null_mut(), |name| {
        assert!(name.len() >= 256, "XPLM nav aid name buffer too small");
        name.as_mut_ptr().cast()
    });
    // SAFETY: each output is null (skipped) or an exclusively borrowed location of its type;
    // the ID and name buffers are at least 32 and 256 bytes, the sizes XPLMNavigation.h
    // recommends. The call takes no lengths, so that documented size is the bound relied on.
    unsafe {
        xp::XPLMGetNavAidInfo(
            nav,
            out(info.kind),
            out(info.latitude),
            out(info.longitude),
            out(info.height),
            out(info.frequency),
            out(info.heading),
            id,
            name,
            std::ptr::null_mut(),
        )
    }
}

/// `XPLMFindNavAid(name, id, null, null, null, kinds)`.
pub fn find_nav_aid(name: Option<&CStr>, id: Option<&CStr>, kinds: i32) -> i32 {
    let text = |v: Option<&CStr>| v.map_or(std::ptr::null(), CStr::as_ptr);
    let null = std::ptr::null_mut();
    // SAFETY: the fragments are null or NUL-terminated for the call; X-Plane only reads them.
    unsafe {
        xp::XPLMFindNavAid(
            text(name),
            text(id),
            null,
            null,
            std::ptr::null_mut(),
            kinds,
        )
    }
}

/// `XPLMCountFMSFlightPlanEntries(plan)`.
pub fn count_fms_flight_plan_entries(plan: i32) -> i32 {
    // SAFETY: a plain plan number, which X-Plane looks up itself (module note); no pointers.
    unsafe { xp::XPLMCountFMSFlightPlanEntries(plan) }
}

/// `XPLMClearFMSFlightPlanEntry(plan, index)`.
pub fn clear_fms_flight_plan_entry(plan: i32, index: i32) {
    // SAFETY: plain integers, which X-Plane looks up itself (module note); no pointers.
    unsafe { xp::XPLMClearFMSFlightPlanEntry(plan, index) }
}

/// `XPLMSetFMSFlightPlanEntryInfo(plan, index, nav, altitude)`.
pub fn set_fms_flight_plan_entry_info(plan: i32, index: i32, nav: i32, altitude: i32) {
    // SAFETY: plain integers, which X-Plane looks up itself (module note); no pointers.
    unsafe { xp::XPLMSetFMSFlightPlanEntryInfo(plan, index, nav, altitude) }
}

/// `XPLMSetDirectToFMSFlightPlanEntry(plan, index)`.
pub fn set_direct_to_fms_flight_plan_entry(plan: i32, index: i32) {
    // SAFETY: plain integers, which X-Plane looks up itself (module note); no pointers.
    unsafe { xp::XPLMSetDirectToFMSFlightPlanEntry(plan, index) }
}

/// `XPLMSetDisplayedFMSFlightPlanEntry(plan, index)`.
pub fn set_displayed_fms_flight_plan_entry(plan: i32, index: i32) {
    // SAFETY: plain integers, which X-Plane looks up itself (module note); no pointers.
    unsafe { xp::XPLMSetDisplayedFMSFlightPlanEntry(plan, index) }
}

/// `XPLMGetGPSDestination()`.
pub fn gps_destination() -> i32 {
    // SAFETY: no arguments; returns a nav ref.
    unsafe { xp::XPLMGetGPSDestination() }
}
