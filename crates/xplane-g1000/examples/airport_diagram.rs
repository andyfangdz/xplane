//! Switchable MFD airport diagram using X-Plane's installed navdata and taxiway layer.
#![cfg(windows)]
use std::{cell::RefCell, ptr, rc::Rc, time::Duration};
use xplane_g1000::{
    Display, FontSize, Layer, Page, ProFont, TextAlign,
    map::{Location, MapDisplay, MapView, Side},
    widgets::{
        InstrumentFont, Readout, Renderer,
        egui::{Color32, Pos2, Rect, Stroke, pos2},
    },
};
use xplane_plugin::{
    AvionicsDraw, CommandHandler, DataRef, PluginMetadata, PluginStateSlot,
    registrations::CommandHandle, xplm,
};
use xplane_sdk_sys as sdk;

const MAP: [u16; 4] = [190, 96, 1004, 680];
const PANEL_ORIGIN: [u16; 2] = [190, 60];
const SIZE: [u16; 2] = [MAP[2] - PANEL_ORIGIN[0], MAP[3] - PANEL_ORIGIN[1]];

struct Airport {
    id: String,
    point: Location,
}

fn nearest(at: Location) -> Option<Airport> {
    if !at.is_valid() {
        return None;
    }
    let (mut lat, mut lon) = (at.latitude as f32, at.longitude as f32);
    // SAFETY: both search coordinates are live. All unused search filters may be null.
    let nav = unsafe {
        sdk::XPLMFindNavAid(
            ptr::null(),
            ptr::null(),
            &mut lat,
            &mut lon,
            ptr::null_mut(),
            sdk::xplm_Nav_Airport as _,
        )
    };
    if nav == sdk::XPLM_NAV_NOT_FOUND {
        return None;
    }
    let (mut kind, mut id) = (0, [0; 32]);
    xplm::nav_aid_info(
        nav,
        xplm::NavAidInfo {
            kind: Some(&mut kind),
            id: Some(&mut id),
            latitude: Some(&mut lat),
            longitude: Some(&mut lon),
            ..Default::default()
        },
    );
    let point = Location {
        latitude: lat.into(),
        longitude: lon.into(),
    };
    if kind != sdk::xplm_Nav_Airport || !point.is_valid() {
        return None;
    }
    let end = id.iter().position(|b| *b == 0).unwrap_or(id.len());
    Some(Airport {
        id: String::from_utf8_lossy(&id[..end]).into_owned(),
        point,
    })
}

struct Diagram {
    map: MapDisplay,
    airport: Option<Airport>,
}
struct Plugin {
    _draw: AvionicsDraw,
    _toggle: CommandHandler,
    selected: Rc<RefCell<Option<Diagram>>>,
}
thread_local! { static PLUGIN: PluginStateSlot<Plugin> = const { PluginStateSlot::new() }; }

fn local([x, y]: [f32; 2]) -> Pos2 {
    pos2(
        x - f32::from(PANEL_ORIGIN[0]),
        y - f32::from(PANEL_ORIGIN[1]),
    )
}

fn register() -> Option<Plugin> {
    let selected: Rc<RefCell<Option<Diagram>>> = Rc::default();
    let active = selected.clone();
    let latitude = DataRef::find("sim/flightmodel/position/latitude")?;
    let longitude = DataRef::find("sim/flightmodel/position/longitude")?;
    let power = DataRef::find("sim/cockpit/electrical/avionics_on")?;
    let ownship = move || Location {
        latitude: latitude.get_f64(),
        longitude: longitude.get_f64(),
    };
    let mut renderer = Renderer::default();
    let face = ProFont::load(&xplane_plugin::system_path()).ok()?;
    let font = InstrumentFont::new(renderer.context(), &face);
    let mut page = Page::new(SIZE, 0, Duration::ZERO)?;
    let scene = RefCell::new(move |target: &mut xplane_g1000::DisplayTarget| {
        let Ok(diagram) = active.try_borrow() else {
            return;
        };
        let Some(diagram) = diagram.as_ref().filter(|_| power.get_i32() != 0) else {
            return;
        };
        let view = diagram
            .airport
            .as_ref()
            .and_then(|a| MapView::airport_diagram(MAP, a.point, 1.25));
        let (mut reference, mut airplane) = (None, None);
        if let (Some(view), Some(airport)) = (&view, &diagram.airport) {
            diagram.map.draw(target, view);
            reference = diagram
                .map
                .project(view, airport.point)
                .filter(|p| view.contains(*p))
                .map(local);
            airplane = diagram
                .map
                .project(view, ownship())
                .filter(|p| view.contains(*p))
                .map(local);
        }
        if renderer.update(&mut page, |ui, layer| {
            if layer != Layer::Indications {
                return;
            }
            let text = |ui: &mut _, s: &str, x, y, align| {
                Readout {
                    font: &font,
                    text: s,
                    size: FontSize::Medium,
                    color: Color32::WHITE,
                }
                .at(ui, pos2(x, y), align);
            };
            let (w, h) = (f32::from(SIZE[0]), f32::from(SIZE[1]));
            let map_top = f32::from(MAP[1] - PANEL_ORIGIN[1]);
            let bounds = Rect::from_min_max(pos2(0., map_top), pos2(w, h));
            ui.painter().rect_filled(
                Rect::from_min_max(pos2(0., 0.), pos2(w, map_top)),
                0.,
                Color32::BLACK,
            );
            let id = diagram.airport.as_ref().map_or("", |a| a.id.as_str());
            text(
                ui,
                &format!("AIRPORT DIAGRAM  {id}"),
                10.,
                25.,
                TextAlign::Left,
            );
            text(ui, "NORTH UP   1.25 NM", w - 10., 25., TextAlign::Right);
            let painter = ui.painter_at(bounds);
            if let Some(point) = reference {
                // White ring marks the database airport reference point. It is
                // independent of X-Plane's geographically positioned runway layout.
                painter.circle_stroke(point, 9., Stroke::new(4., Color32::BLACK));
                painter.circle_stroke(point, 9., Stroke::new(2., Color32::WHITE));
            } else {
                painter.rect_filled(bounds, 0., Color32::BLACK);
                text(
                    ui,
                    if diagram.airport.is_some() {
                        "LOADING MAP DATA"
                    } else {
                        "NO AIRPORT DATA"
                    },
                    w / 2.,
                    h / 2.,
                    TextAlign::Center,
                );
            }
            if let (Some(_), Some(point)) = (reference, airplane) {
                // A cyan position dot avoids implying a heading when stationary.
                painter.circle_filled(point, 5., Color32::BLACK);
                painter.circle_filled(point, 3.5, Color32::CYAN);
            }
        }) {
            target.blit(&mut page, [0, 0, SIZE[0], SIZE[1]], PANEL_ORIGIN);
        }
    });
    let draw = Display::Mfd.overlay("Example airport diagram", move |target| {
        if let Ok(mut scene) = scene.try_borrow_mut() {
            scene(target);
        }
    })?;
    let command = CommandHandle::create(
        c"examples/g1000/toggle_airport_diagram",
        c"Toggle nearest airport diagram",
    )?;
    let selection = selected.clone();
    let toggle =
        CommandHandler::register(command, true, "Toggle airport diagram", move |_, phase| {
            if phase == sdk::xplm_CommandBegin as _
                && let Ok(mut value) = selection.try_borrow_mut()
                && value.take().is_none()
            {
                *value = MapDisplay::new(Side::Pilot).map(|map| Diagram {
                    map,
                    airport: nearest(ownship()),
                });
            }
            0
        });
    Some(Plugin {
        _draw: draw,
        _toggle: toggle,
        selected,
    })
}
fn enable() -> bool {
    let Some(plugin) = register() else {
        return false;
    };
    PLUGIN.with(|s| s.replace(Some(plugin)));
    true
}
fn disable() {
    PLUGIN.with(|s| s.replace(None));
}
fn message(sender: sdk::XPLMPluginID, msg: i32, plane: *mut std::ffi::c_void) {
    if sender == sdk::XPLM_PLUGIN_XPLANE as _
        && matches!(
            msg as u32,
            sdk::XPLM_MSG_PLANE_LOADED | sdk::XPLM_MSG_PLANE_UNLOADED
        )
        && plane.is_null()
    {
        PLUGIN.with(|s| s.with_mut(|p| p.selected.try_borrow_mut().ok()?.take()));
    }
}
fn start() -> bool {
    true
}
xplane_plugin::export_plugin! {
    metadata: PluginMetadata {
        name: "G1000 airport diagram example",
        signature: "org.xplane.examples.g1000airportdiagram",
        description: "Nearest airport taxi diagram on the stock MFD.",
    },
    start: start, stop: disable, enable: enable, disable: disable, receive_message: message,
}
