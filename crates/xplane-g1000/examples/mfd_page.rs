//! A switchable electrical page showing bus 1 voltage.
#![cfg(windows)]

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};
use xplane_g1000::{
    Display, FontSize, Layer, Page, ProFont, TextAlign,
    widgets::{
        InstrumentFont, Readout, Renderer,
        egui::{Color32, Rect, pos2, vec2},
    },
};
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
    let renderer = RefCell::new(Renderer::default());
    let font = ProFont::load(&xplane_plugin::system_path()).ok()?;
    let font = InstrumentFont::new(renderer.borrow().context(), &font);
    let draw = Display::Mfd.overlay("Example bus voltage", move |target| {
        if !visible.get() {
            return;
        }
        let mut volts = [0.];
        if bus.read_f32(&mut volts) != 1 || volts[0] <= 0. {
            return;
        }
        let mut page = page.borrow_mut();
        if renderer.borrow_mut().update(&mut page, |ui, layer| {
            let bar = |width: f32| Rect::from_min_size(pos2(12., 60.), vec2(width.round(), 6.));
            match layer {
                Layer::Background => {
                    Readout {
                        font: &font,
                        text: "BUS 1",
                        size: FontSize::Medium,
                        color: Color32::WHITE,
                    }
                    .at(ui, pos2(12., 24.), TextAlign::Left);
                    ui.painter()
                        .rect_filled(bar(186.), 0., Color32::from_gray(88));
                }
                Layer::Indications => {
                    let green = Color32::from_rgb(80, 255, 128);
                    Readout {
                        font: &font,
                        text: &format!("{:.1} V", volts[0]),
                        size: FontSize::Medium,
                        color: green,
                    }
                    .at(ui, pos2(12., 49.), TextAlign::Left);
                    ui.painter()
                        .rect_filled(bar(186. * (volts[0] / 32.).clamp(0., 1.)), 0., green);
                }
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
