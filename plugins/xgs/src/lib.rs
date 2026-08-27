#![allow(non_snake_case)]
#![deny(unsafe_op_in_unsafe_fn)]

mod runtime;

use xplane_plugin::PluginMetadata;

xplane_plugin::export_plugin! {
    metadata: PluginMetadata {
        name: "Landing Speed Rust 3.46.1",
        signature: "com.andyfang.xgs-rs",
        description: "Rust recreation of Landing Speed (xgs) 3.46",
    },
    start: runtime::start,
    stop: runtime::stop,
    enable: runtime::enable,
    disable: runtime::disable,
    receive_message: runtime::receive_message,
}
