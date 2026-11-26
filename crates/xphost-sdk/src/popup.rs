//! Avionics placement is independent of visibility and on-demand redraw.
use crate::state::{State, index, state};
use serde_json::{Value, json};
use xplane_sdk_sys::XPLMAvionicsID;

#[derive(PartialEq)]
enum Placement {
    Simulator,
    PoppedOut,
    Vr,
}

pub struct Popup {
    name: String,
    custom: bool,
    pub visible: bool,
    placement: Placement,
}

impl Popup {
    pub fn new(name: String, custom: bool) -> Self {
        Self {
            name,
            custom,
            visible: false,
            placement: Placement::Simulator,
        }
    }
}

pub fn capture(s: &State) -> Vec<Value> {
    s.popups
        .iter()
        .map(|(id, p)| {
            json!({
                "id":id, "name":p.name, "custom":p.custom, "visible":p.visible,
                "mapped_to_vr":p.placement == Placement::Vr,
                "popped_out":p.placement == Placement::PoppedOut
            })
        })
        .collect()
}

pub fn end_vr_session(s: &mut State) {
    for p in s.popups.values_mut() {
        if p.placement == Placement::Vr {
            p.placement = Placement::Simulator;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn XPLMPopOutAvionics(id: XPLMAvionicsID) {
    if let Some(p) = state().popups.get_mut(&index(id)) {
        p.placement = Placement::PoppedOut;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn XPLMIsAvionicsPoppedOut(id: XPLMAvionicsID) -> i32 {
    state()
        .popups
        .get(&index(id))
        .is_some_and(|p| p.placement == Placement::PoppedOut) as i32
}

#[unsafe(no_mangle)]
pub extern "C" fn XPLMSetAvionicsMappedToVR(id: XPLMAvionicsID, mapped: i32) {
    let mut s = state();
    let vr = s.store.scalar("sim/graphics/VR/enabled") != 0.0;
    if let Some(p) = s.popups.get_mut(&index(id)) {
        if !vr || !p.custom {
            s.log_raw("xphost: mapping an avionics popup to VR requires a custom device and an active VR session\n");
        } else if mapped != 0 {
            p.placement = Placement::Vr;
        } else if p.placement == Placement::Vr {
            p.placement = Placement::Simulator;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn XPLMIsAvionicsMappedToVR(id: XPLMAvionicsID) -> i32 {
    state()
        .popups
        .get(&index(id))
        .is_some_and(|p| p.placement == Placement::Vr) as i32
}
