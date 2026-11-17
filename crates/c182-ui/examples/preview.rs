//! Asset-free snapshots of the real egui views, for offline visual review.
//! Every page here is synthetic sample data.
use c182_ui::{
    theme,
    view::{
        Bar, Choice, Control, Dialog, Item, Notice, PRESS, Page, Panel, RELEASE, Readout, Row,
        TABS, ViewState,
    },
};
use egui::{Pos2, Rect, Vec2, epaint::Primitive};
use serde_json::json;

fn control(id: u32, label: &str) -> Control {
    Control {
        id,
        label: label.into(),
        enabled: true,
        mask: PRESS | RELEASE,
    }
}
fn status(label: &str, state: &str, tone: u32, action: Option<Control>) -> Item {
    Item::Status {
        label: label.into(),
        state: state.into(),
        tone,
        note: String::new(),
        action,
    }
}
fn toggle(id: u32, label: &str, on: bool, words: [&str; 2]) -> Item {
    Item::Toggle {
        label: label.into(),
        on,
        state: words[usize::from(!on)].into(),
        control: control(id, label),
    }
}
fn choice(first: u32, label: &str, options: &[&str], selected: usize) -> Item {
    Item::Choice {
        label: label.into(),
        options: options
            .iter()
            .enumerate()
            .map(|(i, text)| Choice {
                control: control(first + i as u32, text),
                selected: i == selected,
            })
            .collect(),
    }
}
fn stepper(first: u32, label: &str, value: &str) -> Item {
    Item::Stepper {
        label: label.into(),
        value: value.into(),
        minus: control(first, "−"),
        plus: control(first + 1, "+"),
    }
}
fn text(text: &str) -> Item {
    Item::Text {
        text: text.into(),
        tone: 0,
    }
}
fn panel(title: &str, column: usize, items: Vec<Item>) -> Panel {
    Panel {
        title: title.into(),
        column,
        items,
    }
}
fn row(id: u32, label: &str, section: &str, column: usize) -> Row {
    Row {
        id,
        plus: None,
        label: label.into(),
        section: section.into(),
        reason: String::new(),
        column,
        enabled: true,
        plus_enabled: true,
        visible: true,
        mask: PRESS | RELEASE,
        plus_mask: PRESS | RELEASE,
        tone: 0,
        value: None,
    }
}

fn controls() -> Page {
    let mut rows = vec![];
    let groups: [(&str, usize, &[&str]); 4] = [
        (
            "Electrical",
            0,
            &[
                "Battery master (On)",
                "Alternator (On)",
                "Avionics master (Off)",
                "Pitot heat (Off)",
            ],
        ),
        (
            "Lights",
            0,
            &["Beacon (On)", "Landing light (Off)", "Nav lights (Off)"],
        ),
        (
            "Engine",
            1,
            &["Fuel selector (Both)", "Auto start engine", "Cowl flaps"],
        ),
        (
            "Ground",
            1,
            &[
                "Parking brake (Set)",
                "Wheel chocks (In place)",
                "Tow bar (Stowed)",
            ],
        ),
    ];
    for (section, column, labels) in groups {
        for label in labels {
            rows.push(row(rows.len() as u32, label, section, column));
        }
    }
    rows[2].enabled = false;
    rows[2].reason = "Set by the glass panel".into();
    rows[9].value = Some((65., 0., 100.));
    Page {
        title: TABS[0].into(),
        tab: Some(0),
        rows,
        ..Page::default()
    }
}

fn payload() -> Page {
    let labels = [
        ("Pilot (175 lb)", "People"),
        ("Front passenger (150 lb)", "People"),
        ("Rear left (0 lb)", "People"),
        ("Rear right (0 lb)", "People"),
        ("Baggage (20 lb)", "Cargo"),
        ("Left tank (38.0 US gal)", "Fuel"),
        ("Right tank (38.0 US gal)", "Fuel"),
    ];
    Page {
        title: TABS[1].into(),
        tab: Some(1),
        rows: labels
            .iter()
            .enumerate()
            .map(|(i, (label, section))| Row {
                plus: Some(i as u32 + 100),
                ..row(i as u32, label, section, 0)
            })
            .collect(),
        readouts: vec![Readout {
            text: "Total weight: 2,704 lb\nPayload: 345 lb\nFuel weight: 456 lb\n\nLeft tank: 38.0 US gal\nRight tank: 38.0 US gal\n\nCentre of gravity: 41.2 in".into(),
            tone: 0,
        }],
        ..Page::default()
    }
}

fn maintenance() -> Page {
    let parts = [
        ("Propeller", 3),
        ("Fuel system", 3),
        ("Oil system", 2),
        ("Left brake", 1),
        ("Right brake", 3),
        ("Left tire", 3),
        ("Right tire", 3),
        ("Nose tire", 2),
        ("Left flap", 3),
        ("Right flap", 3),
        ("Battery", 3),
        ("Ailerons", 3),
        ("Rudder", 3),
        ("Elevator", 3),
    ];
    let mut condition: Vec<Item> = parts
        .iter()
        .enumerate()
        .map(|(i, (label, tone))| {
            status(
                label,
                ["", "Failed", "Worn", "Good"][*tone as usize],
                *tone,
                (*tone != 3).then(|| control(i as u32, "Repair")),
            )
        })
        .collect();
    condition.push(Item::Actions(vec![
        control(40, "Inspect engine"),
        control(41, "Complete overhaul"),
    ]));
    Page {
        title: TABS[2].into(),
        tab: Some(2),
        panels: vec![
            panel("Airframe & systems", 0, condition),
            panel(
                "Equipment",
                1,
                vec![
                    toggle(50, "Engine heater kit", false, ["Installed", "Removed"]),
                    toggle(51, "Flap gap seals", true, ["Installed", "Removed"]),
                    toggle(52, "Main wheel fairings", true, ["Installed", "Removed"]),
                    toggle(53, "Nose wheel fairing", true, ["Installed", "Removed"]),
                    status("Propeller", "Three blade", 0, Some(control(54, "Swap"))),
                    choice(55, "Battery", &["Standard", "High capacity"], 0),
                    choice(57, "Tires", &["Standard", "Long wear"], 1),
                ],
            ),
            panel(
                "Mechanic's report",
                1,
                vec![text(
                    "Airframe 1,204.6 h, engine 412.3 h since overhaul.\n\nGeneral condition is good. The left brake has failed and the oil system and nose tire show wear.\n\nSynthetic preview data.",
                )],
            ),
        ],
        ..Page::default()
    }
}

fn locked() -> Page {
    Page {
        title: TABS[2].into(),
        tab: Some(2),
        notice: Some(Notice {
            text: "The hangar opens on the ground with the engine stopped.".into(),
            tone: 2,
            action: Some(control(13, "Open anyway")),
        }),
        ..Page::default()
    }
}

fn preflight() -> Page {
    let field = |label: &str, value: &str| Item::Field {
        label: label.into(),
        value: value.into(),
        tone: 0,
    };
    let stations = [
        "Cabin",
        "Left wing",
        "Left main gear",
        "Nose",
        "Right main gear",
        "Right wing",
        "Tail",
    ];
    Page {
        title: TABS[3].into(),
        tab: Some(3),
        panels: vec![
            panel(
                "Left wing",
                0,
                vec![
                    Item::Actions(vec![
                        control(0, "Previous station"),
                        control(1, "Next station"),
                    ]),
                    Item::Actions(vec![
                        control(2, "Remove pitot cover"),
                        control(3, "Take fuel sample"),
                        control(4, "Remove tie down"),
                    ]),
                    Item::Text {
                        text: "Fuel sample: clear and blue".into(),
                        tone: 3,
                    },
                    text("A and Shift+A also walk between stations."),
                ],
            ),
            panel(
                "Checklist: left wing",
                0,
                vec![
                    field("Pitot cover", "Removed"),
                    field("Fuel sample", "Checked"),
                    field("Fuel cap", "Secure"),
                    field("Aileron", "Free and correct"),
                    field("Tie down", "Removed"),
                ],
            ),
            panel("Walk to", 1, vec![choice(10, "", &stations, 1)]),
        ],
        ..Page::default()
    }
}

fn engine() -> Page {
    let parts = [
        ("Crankshaft", 3, "Repair"),
        ("Left magneto", 3, "Repair"),
        ("Right magneto", 2, "Repair"),
        ("Alternator", 3, "Repair"),
        ("Starter", 3, "Repair"),
        ("Fuel pump", 3, "Repair"),
        ("Oil pump", 3, "Repair"),
        ("Vacuum pump", 1, "Repair"),
        ("Fuel injection", 3, "Repair"),
        ("Oil lines", 3, "Repair"),
        ("Air filter", 2, "Replace"),
        ("Oil filter", 3, "Replace"),
        ("Fuel filter", 3, "Replace"),
    ];
    let levels = [0.78, 0.76, 0.52, 0.77, 0.21, 0.74];
    Page {
        title: "Engine".into(),
        tab: Some(4),
        panels: vec![
            panel(
                "Components",
                0,
                parts
                    .iter()
                    .enumerate()
                    .map(|(i, (label, tone, verb))| {
                        status(
                            label,
                            ["", "Failed", "Worn", "Good"][*tone as usize],
                            *tone,
                            (*tone != 3).then(|| control(i as u32, verb)),
                        )
                    })
                    .collect(),
            ),
            panel(
                "Cylinder compression",
                1,
                vec![
                    Item::Bars(
                        levels
                            .iter()
                            .enumerate()
                            .map(|(i, level)| {
                                let tone = if *level < 0.3 {
                                    1
                                } else if *level < 0.6 {
                                    2
                                } else {
                                    3
                                };
                                Bar {
                                    label: format!("{}", i + 1),
                                    level: *level,
                                    text: format!("{:.0} %", level * 100.),
                                    tone,
                                    action: (tone != 3).then(|| control(20 + i as u32, "Replace")),
                                }
                            })
                            .collect(),
                    ),
                    Item::Actions(vec![control(30, "Run compression test")]),
                ],
            ),
            panel(
                "Consumables",
                1,
                vec![
                    status(
                        "Spark plugs",
                        "Massive electrode",
                        0,
                        Some(control(31, "Change…")),
                    ),
                    status(
                        "Oil",
                        "20W-50 with additive",
                        0,
                        Some(control(32, "Change…")),
                    ),
                ],
            ),
        ],
        ..Page::default()
    }
}

fn dialog() -> Page {
    let mut page = engine();
    page.dialog = Some(Dialog {
        title: "Change the oil".into(),
        items: vec![
            choice(60, "Grade", &["100", "20W-50", "25W-60"], 1),
            toggle(63, "Additive", true, ["Added", "None"]),
        ],
        confirm: control(64, "Change oil"),
        cancel: control(65, "Cancel"),
    });
    page
}

fn radios() -> Page {
    let mut panels = vec![];
    for (i, (name, active, standby)) in [
        ("COM 1", "124.850", "121.900"),
        ("NAV 1", "113.60", "110.50"),
        ("COM 2", "118.300", "126.275"),
        ("NAV 2", "109.90", "116.80"),
    ]
    .iter()
    .enumerate()
    {
        let id = i as u32 * 10;
        panels.push(panel(
            name,
            i % 2,
            vec![
                Item::Field {
                    label: "Active".into(),
                    value: active.to_string(),
                    tone: 3,
                },
                stepper(id, "Standby MHz", &standby[..3]),
                stepper(id + 2, "Standby kHz", &standby[4..]),
                Item::Actions(vec![control(id + 4, "Swap"), control(id + 5, "Power")]),
            ],
        ));
    }
    Page {
        title: TABS[5].into(),
        tab: Some(5),
        panels,
        ..Page::default()
    }
}

fn pause() -> Page {
    Page {
        title: TABS[6].into(),
        tab: Some(6),
        panels: vec![panel(
            "Pause the simulator",
            0,
            vec![
                toggle(1, "On an emergency", true, ["On", "Off"]),
                toggle(2, "At an altitude", true, ["On", "Off"]),
                stepper(3, "Altitude", "5,000 ft"),
                toggle(5, "After a time", false, ["On", "Off"]),
                stepper(6, "Time", "30 min"),
                toggle(8, "After a distance", false, ["On", "Off"]),
                stepper(9, "Distance", "50 nm"),
            ],
        )],
        ..Page::default()
    }
}

fn notes() -> Page {
    Page {
        title: TABS[7].into(),
        tab: Some(7),
        panels: vec![
            panel(
                "Conditions",
                0,
                vec![text(
                    "Outside temperature 15 °C\nCabin is comfortable\n\nGround speed 0 kt\nEndurance 5 h 10 min",
                )],
            ),
            panel(
                "Checklist, page 2 of 8",
                1,
                vec![
                    text(
                        "Before starting the engine\n\nPreflight inspection complete\nSeats and belts adjusted and locked\nBrakes tested and set\nCircuit breakers in\nAvionics off\nFuel selector both",
                    ),
                    Item::Actions(vec![control(2, "Previous page"), control(3, "Next page")]),
                ],
            ),
        ],
        ..Page::default()
    }
}

fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).expect("output directory"));
    std::fs::create_dir_all(&out).unwrap();
    let pages = [
        ("0-controls", controls()),
        ("1-payload", payload()),
        ("2-maintenance", maintenance()),
        ("2-maintenance-locked", locked()),
        ("3-preflight", preflight()),
        ("4-engine", engine()),
        ("4-engine-dialog", dialog()),
        ("5-radios", radios()),
        ("6-pause", pause()),
        ("7-notes", notes()),
    ];
    for (name, page) in pages {
        let ctx = egui::Context::default();
        theme::apply(&ctx);
        let state = ViewState::default();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1000., 800.))),
                ..Default::default()
            },
            |ui| state.clone().show(ui, &page),
        );
        let mut textures = vec![];
        for (id, deltas) in &output.textures_delta.set {
            for delta in deltas {
                let egui::ImageData::Color(image) = &delta.image;
                textures.push(json!({"id":format!("{id:?}"),"size":image.size,"pos":delta.pos,"pixels":image.pixels.iter().map(|p|p.to_array()).collect::<Vec<_>>()}));
            }
        }
        let meshes=ctx.tessellate(std::mem::take(&mut output.shapes),output.pixels_per_point).into_iter().filter_map(|p|if let Primitive::Mesh(m)=p.primitive{Some(json!({"clip":[p.clip_rect.min.x,p.clip_rect.min.y,p.clip_rect.max.x,p.clip_rect.max.y],"texture":format!("{:?}",m.texture_id),"vertices":m.vertices.iter().map(|v|[v.pos.x,v.pos.y,v.uv.x,v.uv.y,v.color.r()as f32,v.color.g()as f32,v.color.b()as f32,v.color.a()as f32]).collect::<Vec<_>>(),"indices":m.indices}))}else{None}).collect::<Vec<_>>();
        std::fs::write(
            out.join(format!("{name}.json")),
            serde_json::to_vec(&json!({"size":[1000,800],"textures":textures,"meshes":meshes}))
                .unwrap(),
        )
        .unwrap();
        output.textures_delta.clear();
    }
}
