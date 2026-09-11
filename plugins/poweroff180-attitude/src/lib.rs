// SPDX-License-Identifier: GPL-3.0-or-later
#![deny(unsafe_op_in_unsafe_fn)]
mod runtime;
xplane_plugin::export_plugin! {
    metadata:xplane_plugin::PluginMetadata {
        name:"XPT Rust attitude controller",
        signature:"andyf.sr20g6.testcontroller.ardupilot",
        description:"Temporary aircraft-local attitude control, translated from the v1.9 adapter.",
    },
    start:runtime::start,stop:runtime::stop,enable:runtime::enable,
    disable:runtime::disable,receive_message:runtime::receive_message,
}
