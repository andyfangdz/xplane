//! A switchable electrical page showing bus 1 voltage. All example artwork is generated here.
#![cfg(windows)]

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::Duration,
};
use xplane_g1000::{BitmapFont, Display, Gdi, Layer, Page};
use xplane_plugin::{
    AvionicsDraw, CommandHandler, DataRef, PluginMetadata, PluginStateSlot,
    registrations::CommandHandle,
};

thread_local! {static PLUGIN:PluginStateSlot<(AvionicsDraw,CommandHandler)>=const {PluginStateSlot::new()};}

// Original 3-by-5 dot patterns, expanded into 12-by-15 texel glyphs.
const CHARS: &str = "0123456789.BUS V";
fn font_masks() -> Vec<u8> {
    let rows = [
        [7, 5, 5, 5, 7],
        [2, 6, 2, 2, 7],
        [7, 1, 7, 4, 7],
        [7, 1, 7, 1, 7],
        [5, 5, 7, 1, 1],
        [7, 4, 7, 1, 7],
        [7, 4, 7, 5, 7],
        [7, 1, 2, 2, 2],
        [7, 5, 7, 5, 7],
        [7, 5, 7, 1, 7],
        [0, 0, 0, 0, 2],
        [6, 5, 6, 5, 6],
        [5, 5, 5, 5, 7],
        [7, 4, 7, 1, 7],
        [0, 0, 0, 0, 0],
        [5, 5, 5, 5, 2],
    ];
    let mut masks = Vec::new();
    for glyph in rows {
        for y in 0..15 {
            for x in 0..12 {
                let on = x < 9 && glyph[y / 3] & (4 >> (x / 3)) != 0;
                masks.push(if on { 255 } else { 0 });
            }
        }
    }
    masks
}

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
    let masks = font_masks();
    let draw = Display::Mfd.overlay("Example bus voltage", move |target| {
        if !visible.get() {
            return;
        }
        let mut volts = [0.];
        if bus.read_f32(&mut volts) != 1 || volts[0] <= 0. {
            return;
        }
        let mut page = page.borrow_mut();
        let font = BitmapFont::new([12, 15], &masks).unwrap();
        if page.update(&gdi, |c, layer| {
            let glyph = |ch| CHARS.find(ch).map(|index| (index, 0));
            match layer {
                Layer::Background => {
                    c.text_mapped([12, 10], "BUS 1", &font, 0xffffffff, glyph);
                    c.rect(12., 60., 186., 6., 0xff585858);
                }
                Layer::Indications => {
                    c.text_mapped(
                        [12, 35],
                        &format!("{:.1} V", volts[0]),
                        &font,
                        0xff50ff80,
                        glyph,
                    );
                    c.rect(
                        12.,
                        60.,
                        186. * (volts[0] / 32.).clamp(0., 1.),
                        6.,
                        0xff50ff80,
                    );
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
