#![deny(unsafe_op_in_unsafe_fn)]
mod graphics;
mod runtime;
pub mod scene;
pub mod values;
xplane_plugin::export_plugin! {
    metadata:xplane_plugin::PluginMetadata {
        name:"XPT Rust custom HUD",signature:"local.xpt.video-hud.v5",
        description:"Native G1000 instruments, command bars and aircraft symbol for simulator flight tests.",
    },
    start:runtime::start,stop:runtime::stop,enable:runtime::enable,
    disable:runtime::disable,receive_message:runtime::receive_message,
}
