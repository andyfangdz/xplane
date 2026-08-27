#![allow(non_snake_case)]

mod aircraft;
mod commands;
mod datarefs;
mod lifecycle;
mod pad_library;
mod state;
mod support;
mod ui;

pub(crate) use lifecycle::{disable, enable, receive_message, start, stop};

pub(in crate::runtime) use commands::CommandAction;
pub(in crate::runtime) use state::{with_state_mut, PluginState};
