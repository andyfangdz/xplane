#![deny(unsafe_op_in_unsafe_fn)]
mod runtime;
xplane_plugin::export_plugin! {
    metadata:xplane_plugin::PluginMetadata {
        name:"XPT Rust guidance and telemetry",
        signature:"local.xpt.native.v1",
        description:"Temporary deterministic simulator-test guidance; no flight-model force overrides.",
    },
    start:runtime::start,stop:runtime::stop,enable:runtime::enable,
    disable:runtime::disable,receive_message:runtime::receive_message,
}
