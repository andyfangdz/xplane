//! A switchable electrical page showing bus 1 voltage.
#![cfg(windows)]

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};
use xplane_g1000::{Display, FontSize, Gdi, Layer, Page, ProFont, TextAlign};
use xplane_plugin::{
    AvionicsDraw, CommandHandler, DataRef, PluginMetadata, PluginStateSlot,
    registrations::CommandHandle,
};

thread_local! {static PLUGIN:PluginStateSlot<(AvionicsDraw,CommandHandler)>=const {PluginStateSlot::new()};}

fn register() -> Option<(AvionicsDraw, CommandHandler)> {
    let selected = Rc::new(Cell::new(false));
    let visible = Rc::clone(&selected);
    let bus = DataRef::find("sim/cockpit2/electrical/bus_volts")?;
    let page = RefCell::new(Page::new(
        [210, 86],
        0xff202020,
        Duration::from_millis(100),
    )?);
    let gdi = Gdi::start().ok()?;
    let font = ProFont::load(&xplane_plugin::system_path()).ok()?;
    let draw = Display::Mfd.overlay("Example bus voltage", move |target| {
        if !visible.get() {
            return;
        }
        let mut volts = [0.];
        if bus.read_f32(&mut volts) != 1 || volts[0] <= 0. {
            return;
        }
        let mut page = page.borrow_mut();
        let strike = font.strike(FontSize::Medium);
        if page.update(&gdi, |c, layer| match layer {
            Layer::Background => {
                strike.draw(c, 12., 24, TextAlign::Left, "BUS 1", 0xffffffff);
                c.rect(12., 60., 186., 6., 0xff585858);
            }
            Layer::Indications => {
                strike.draw(
                    c,
                    12.,
                    49,
                    TextAlign::Left,
                    &format!("{:.1} V", volts[0]),
                    0xff50ff80,
                );
                c.rect(
                    12.,
                    60.,
                    186. * (volts[0] / 32.).clamp(0., 1.),
                    6.,
                    0xff50ff80,
                );
            }
        }) {
            target.blit(&mut page, [0, 0, 210, 86], [170, 80]);
        }
    })?;
    let command = CommandHandle::create(
        c"examples/g1000/toggle_page",
        c"Toggle the example G1000 electrical page",
    )?;
    let toggle =
        CommandHandler::register(command, true, "Toggle electrical page", move |_, phase| {
            if phase == xplane_sdk_sys::xplm_CommandBegin as _ {
                selected.set(!selected.get());
            }
            0
        });
    Some((draw, toggle))
}
fn enable() -> bool {
    let Some(state) = register() else {
        return false;
    };
    PLUGIN.with(|s| s.replace(Some(state)));
    true
}
fn disable() {
    PLUGIN.with(|s| s.replace(None));
}
fn start() -> bool {
    true
}
fn message(_: xplane_sdk_sys::XPLMPluginID, _: i32, _: *mut std::ffi::c_void) {}
xplane_plugin::export_plugin! {
    metadata: PluginMetadata {
        name: "G1000 electrical page example",
        signature: "org.xplane.examples.g1000page",
        description: "Switchable bus voltage overlay on the stock MFD.",
    },
    start: start,
    stop: disable,
    enable: enable,
    disable: disable,
    receive_message: message,
}
