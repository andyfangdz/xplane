#![allow(non_snake_case)]
#![deny(unsafe_op_in_unsafe_fn)]

pub mod config;
mod glyphs;
mod graphics;
pub mod guidance;
pub mod math;
pub mod presentation;
mod runtime;
pub mod scene;

xplane_plugin::export_plugin! {
    metadata: xplane_plugin::PluginMetadata {
        name: "Shuttle HUD Rust",
        signature: "local.shuttle.fsim-manual-hud",
        description: "Aircraft-local native Shuttle HUD, landing guidance and staged drag chute in Rust.",
    },
    start: runtime::start,
    stop: runtime::stop,
    enable: runtime::enable,
    disable: runtime::disable,
    receive_message: runtime::receive_message,
}
