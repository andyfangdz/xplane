//! `XPLMNavigation.h`: the nav database, FMS flight plans and the GPS.
//!
//! Nav refs and entry indices are integers X-Plane looks up in its own nav
//! database and flight plans (module note), passed through unchanged: a nav
//! ref may legitimately come from any lookup, including ones this module does
//! not wrap (`XPLMFindFirstNavAidOfType`, the out ref of the FMS entry
//! queries, which `XPLMNavigation.h` offers for exactly this "additional
//! information"), and the 4.1 flight-plan calls state no entry maximum. The
//! flight plan is a closed enum, so a value outside the six `xplm_Fpl_*`
//! constants panics. [`crate::fms_entries`] and the other FMS helpers at the
//! crate root check the plan and index more strictly before calling.

use std::ffi::CStr;

use xplane_sdk_sys as xp;

/// The flight plans `XPLMNavFlightPlan` names, `xplm_Fpl_Pilot_Primary` to
/// `xplm_Fpl_CoPilot_Temporary`.
const FLIGHT_PLANS: std::ops::Range<i32> = 0..6;

fn check_plan(plan: i32) -> i32 {
    assert!(
        FLIGHT_PLANS.contains(&plan),
        "not an XPLM flight plan: {plan}"
    );
    plan
}

value_calls! {
    /// `XPLMGetFirstNavAid()`.
    pub fn first_nav_aid() -> i32 = XPLMGetFirstNavAid;
    /// `XPLMGetNextNavAid(nav)`; any `nav` (X-Plane returns `XPLM_NAV_NOT_FOUND`
    /// for an invalid one).
    pub fn next_nav_aid(nav: i32) -> i32 = XPLMGetNextNavAid;
    /// `XPLMGetGPSDestination()`.
    pub fn gps_destination() -> i32 = XPLMGetGPSDestination;
    fn sdk_count_entries(plan: i32) -> i32 = XPLMCountFMSFlightPlanEntries;
    fn sdk_clear_entry(plan: i32, index: i32) = XPLMClearFMSFlightPlanEntry;
    fn sdk_set_entry_info(plan: i32, index: i32, nav: i32, altitude: i32) =
        XPLMSetFMSFlightPlanEntryInfo;
    fn sdk_set_direct_to_entry(plan: i32, index: i32) = XPLMSetDirectToFMSFlightPlanEntry;
    fn sdk_set_displayed_entry(plan: i32, index: i32) = XPLMSetDisplayedFMSFlightPlanEntry;
    fn sdk_destination_entry(plan: i32) -> i32 = XPLMGetDestinationFMSFlightPlanEntry;
    fn sdk_set_destination_entry(plan: i32, index: i32) = XPLMSetDestinationFMSFlightPlanEntry;
}

/// `XPLMGetDestinationFMSFlightPlanEntry(plan)`.
///
/// # Panics
/// If `plan` is not one of the six flight plans (module note).
pub fn destination_fms_flight_plan_entry(plan: i32) -> i32 {
    sdk_destination_entry(check_plan(plan))
}

/// `XPLMSetDestinationFMSFlightPlanEntry(plan, index)`.
///
/// # Panics
/// If `plan` is not one of the six flight plans (module note).
pub fn set_destination_fms_flight_plan_entry(plan: i32, index: i32) {
    sdk_set_destination_entry(check_plan(plan), index);
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
    // SAFETY: `nav` is an integer X-Plane looks up itself (module note); each output is null
    // (skipped) or an exclusively borrowed location of its type; the ID and name buffers are at
    // least 32 and 256 bytes, the sizes XPLMNavigation.h recommends. The call takes no lengths,
    // so that documented size is the bound relied on.
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
///
/// # Panics
/// If `plan` is not one of the six flight plans (module note).
pub fn count_fms_flight_plan_entries(plan: i32) -> i32 {
    sdk_count_entries(check_plan(plan))
}

/// `XPLMClearFMSFlightPlanEntry(plan, index)`.
///
/// # Panics
/// If `plan` is not one of the six flight plans (module note).
pub fn clear_fms_flight_plan_entry(plan: i32, index: i32) {
    sdk_clear_entry(check_plan(plan), index);
}

/// `XPLMSetFMSFlightPlanEntryInfo(plan, index, nav, altitude)`.
///
/// # Panics
/// If `plan` is not one of the six flight plans (module note).
pub fn set_fms_flight_plan_entry_info(plan: i32, index: i32, nav: i32, altitude: i32) {
    sdk_set_entry_info(check_plan(plan), index, nav, altitude);
}

/// `XPLMSetDirectToFMSFlightPlanEntry(plan, index)`.
///
/// # Panics
/// If `plan` is not one of the six flight plans (module note).
pub fn set_direct_to_fms_flight_plan_entry(plan: i32, index: i32) {
    sdk_set_direct_to_entry(check_plan(plan), index);
}

/// `XPLMSetDisplayedFMSFlightPlanEntry(plan, index)`.
///
/// # Panics
/// If `plan` is not one of the six flight plans (module note).
pub fn set_displayed_fms_flight_plan_entry(plan: i32, index: i32) {
    sdk_set_displayed_entry(check_plan(plan), index);
}

#[cfg(test)]
mod tests {
    use super::check_plan;
    use std::panic::catch_unwind;

    #[test]
    fn flight_plans_outside_the_enum_are_refused() {
        // Only the check is exercised; no SDK call is made.
        assert_eq!(check_plan(0), 0);
        assert_eq!(check_plan(5), 5);
        assert!(catch_unwind(|| check_plan(6)).is_err());
        assert!(catch_unwind(|| check_plan(-1)).is_err());
    }
}
