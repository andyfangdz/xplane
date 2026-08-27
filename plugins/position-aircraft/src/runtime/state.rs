use std::path::PathBuf;

use crate::pad::{Form, PadData};
use xplane_airports::RunwayDatabase;
use xplane_plugin::{FlightLoop, PluginMenu, PluginStateSlot, Window};

use super::commands::RegisteredCommand;
use super::datarefs::DataRefs;
use super::pattern::PatternState;
use super::ui::EguiIntegration;

thread_local! {
    /// XPLM invokes this plugin's lifecycle, flight-loop, command, and window
    /// callbacks on its plugin thread. Keeping state thread-local makes that
    /// affinity explicit and avoids claiming that XPLM/GL handles are `Send`.
    static STATE: PluginStateSlot<PluginState> = const { PluginStateSlot::new() };
}

pub(in crate::runtime) fn with_state_mut<R>(f: impl FnOnce(&mut PluginState) -> R) -> Option<R> {
    STATE.with(|slot| slot.with_mut(f))
}

pub(super) fn replace_state(state: Option<PluginState>) -> Option<PluginState> {
    STATE.with(|slot| slot.replace(state))
}

pub(in crate::runtime) struct PendingReapply {
    pub(in crate::runtime) data: PadData,
    pub(in crate::runtime) wait_frames: i32,
    pub(in crate::runtime) remaining_frames: i32,
}

pub(in crate::runtime) struct PluginState {
    pub(in crate::runtime) window: Option<Window>,
    pub(in crate::runtime) flight_loop: Option<FlightLoop>,
    pub(in crate::runtime) pad_directory: PathBuf,
    pub(in crate::runtime) pads: Vec<String>,
    pub(in crate::runtime) selected_index: usize,
    pub(in crate::runtime) form: Form,
    pub(in crate::runtime) status: String,
    pub(in crate::runtime) ui: Option<EguiIntegration>,
    pub(in crate::runtime) datarefs: DataRefs,
    pub(in crate::runtime) commands: Vec<RegisteredCommand>,
    pub(in crate::runtime) menu: Option<PluginMenu>,
    pub(in crate::runtime) pending: Option<PendingReapply>,
    pub(in crate::runtime) airports: Option<RunwayDatabase>,
    pub(in crate::runtime) pattern: PatternState,
}
