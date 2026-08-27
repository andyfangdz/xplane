#![allow(non_snake_case)]
#![deny(unsafe_op_in_unsafe_fn)]

mod pad;
mod runtime;

use xplane_plugin::PluginMetadata;

xplane_plugin::export_plugin! {
    metadata: PluginMetadata {
        name: "Position Aircraft Native",
        signature: "com.openai.position-aircraft-native-rust",
        description: "Native VR and joystick PositionAircraft replacement written in Rust",
    },
    start: runtime::start,
    stop: runtime::stop,
    enable: runtime::enable,
    disable: runtime::disable,
    receive_message: runtime::receive_message,
}
