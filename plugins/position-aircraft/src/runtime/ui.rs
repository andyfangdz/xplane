mod adapter;
mod pattern_tab;
mod theme;
mod view;

pub(super) use adapter::{
    draw_window, handle_cursor, handle_key, handle_mouse, handle_right_click, handle_wheel,
    EguiIntegration,
};
