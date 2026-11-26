#![deny(unsafe_op_in_unsafe_fn)]

mod callbacks;
mod command;
mod dataref;
mod draw_callback;
mod drawing;
mod flight_loop;
mod frame_buffer;
mod geometry;
pub mod guard;
pub mod lifecycle;
mod menu;
mod menu_item;
mod navigation;
#[cfg(all(windows, feature = "opengl"))]
pub mod opengl;
mod owned_array;
mod owned_dataref;
#[cfg(feature = "panel-image")]
pub mod panel_image;
pub mod panel_ui;
mod paths;
mod plugin;
pub mod registrations;
mod retained;
mod state;
mod terrain;
mod thread_id;
mod widget;
mod window;
pub mod xplm;

pub use callbacks::{
    ArrayRead, ArrayWrite, AvionicsDraw, AvionicsParams, CallbackFlightLoop, CallbackMenu,
    CallbackWindow, CameraControl, CommandHandler, CustomAvionics, DataAccess, DataAccessor,
    DrawHandler, WindowHandler, WindowParams, copy_array,
};
pub use command::command_once;
pub use command::{Command, CommandCallbackFn};
pub use dataref::{DataRef, DataRefCache};
pub use draw_callback::{DrawCallback, DrawCallbackFn};
pub use drawing::set_graphics_state;
pub use drawing::{bind_texture_2d, set_textured_graphics_state};
pub use drawing::{draw_string, measure_string, set_2d_graphics_state};
pub use flight_loop::FlightLoop;
pub use flight_loop::FlightLoopFn;
pub use flight_loop::PhaseFlightLoop;
pub use flight_loop::{
    FlightLoopError, FlightLoopFault, FlightLoopFrame, FlightLoopHandler, ManagedFlightLoop,
    SimulationTimingError, SimulationTimingSample, SimulationTimingSampler,
};
pub use frame_buffer::FrameBuffer;
pub use geometry::{Bounds, magnetic_variation, screen_bounds, world_to_local};
pub use geometry::{local_to_world, screen_size};
pub use menu::{MenuHandlerFn, PluginMenu};
pub use menu_item::MenuItem;
pub use navigation::{FmsEntry, fms_destination, fms_entries, load_fms_plan, set_fms_destination};
pub use owned_array::OwnedFloatArray;
pub use owned_dataref::OwnedDataRef;
pub use paths::{current_aircraft_path, plugin_directory, preferences_directory, system_path};
#[doc(hidden)]
pub use plugin::{install_development_reload, remove_development_reload, write_plugin_metadata};
pub use plugin::{DebugLogger, PluginMetadata, c_string, enable_feature};
pub use state::PluginStateSlot;
pub use terrain::TerrainProbe;
pub use thread_id::current_thread_id;
pub use widget::{WidgetFn, WidgetWindow};
pub use window::{
    Window, WindowCallbacks, WindowConfig, WindowCursorFn, WindowDrawFn, WindowKeyFn,
    WindowMouseClickFn, WindowMouseWheelFn, WindowPosition,
};
pub use xplm::PlainMenu;
