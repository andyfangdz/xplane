#![deny(unsafe_op_in_unsafe_fn)]
mod runtime;
xplane_plugin::export_plugin! {
    metadata:xplane_plugin::PluginMetadata {
        name:"XPT passive performance recorder",
        signature:"local.xpt.performance.v1",
        description:"Buffered frame timing, repeatable parked camera sweeps and stutter markers.",
    },
    start:runtime::start,stop:runtime::stop,enable:runtime::enable,
    disable:runtime::disable,receive_message:runtime::receive_message,
}
