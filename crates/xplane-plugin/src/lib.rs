#![deny(unsafe_op_in_unsafe_fn)]

mod command;
mod dataref;
mod drawing;
mod flight_loop;
mod geometry;
mod menu;
mod paths;
mod plugin;
mod state;
mod widget;
mod window;

pub use command::Command;
pub use dataref::DataRef;
pub use drawing::{draw_string, measure_string, set_2d_graphics_state};
pub use flight_loop::FlightLoop;
pub use geometry::{screen_bounds, world_to_local, Bounds};
pub use menu::PluginMenu;
pub use paths::{current_aircraft_path, plugin_directory, preferences_directory, system_path};
#[doc(hidden)]
pub use plugin::write_plugin_metadata;
pub use plugin::{c_string, enable_feature, DebugLogger, PluginMetadata};
pub use state::PluginStateSlot;
pub use widget::WidgetWindow;
pub use window::{Window, WindowCallbacks, WindowConfig, WindowPosition};
