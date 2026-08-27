#![deny(unsafe_op_in_unsafe_fn)]

mod dataref;
mod menu;
mod paths;
mod plugin;
mod state;

pub use dataref::DataRef;
pub use menu::PluginMenu;
pub use paths::{current_aircraft_path, plugin_directory, preferences_directory, system_path};
pub use plugin::{c_string, enable_feature, write_plugin_metadata, DebugLogger, PluginMetadata};
pub use state::PluginStateSlot;
