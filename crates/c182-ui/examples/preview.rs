//! Asset-free snapshots of the real egui views, for offline visual review.
use c182_ui::{
    theme,
    view::{Page, Readout, Row, ViewState, PRESS, RELEASE, TABS},
};
use egui::{epaint::Primitive, Pos2, Rect, Vec2};
use serde_json::json;
fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).expect("output directory"));
    std::fs::create_dir_all(&out).unwrap();
    for (tab, title) in TABS.iter().enumerate() {
        let labels: &[&str] = match tab {
            0 => &[
                "Battery master: On",
                "Alternator: On",
                "Parking brake: Set",
                "Pitot heat: Off",
                "Landing light: Off",
                "Cowl flaps",
                "GPS configuration",
            ],
            1 => &[
                "Pilot (175 lb)",
                "Front passenger (150 lb)",
                "Rear left (0 lb)",
                "Rear right (0 lb)",
                "Left tank (38 US gal)",
                "Right tank (38 US gal)",
            ],
            2 => &[
                "Inspect aircraft",
                "Change engine oil",
                "Repair left brake",
                "Oil system: good",
                "Fuel system: good",
                "Battery: good",
            ],
            3 => &[
                "Previous station",
                "Next station",
                "Remove pitot cover",
                "Check fuel sample",
                "Inspect left tire",
            ],
            4 => &[
                "Prime engine",
                "Hold starter",
                "Inspect spark plugs",
                "Check oil",
            ],
            5 => &[
                "COM 1 coarse",
                "COM 1 fine",
                "COM 1 swap",
                "NAV 1 coarse",
                "NAV 1 fine",
                "NAV 1 swap",
            ],
            6 => &[
                "Pause on emergency: On",
                "Pause at altitude: Off",
                "Pause after time: Off",
                "Pause after distance: Off",
            ],
            _ => &["Previous page", "Next page"],
        };
        let rows = labels
            .iter()
            .enumerate()
            .map(|(id, label)| Row {
                id: id as u32,
                plus: if tab == 1 {
                    Some(id as u32 + 100)
                } else {
                    None
                },
                label: label.to_string(),
                section: if tab == 0 && id < 3 {
                    "Aircraft".into()
                } else if tab == 0 {
                    "Equipment".into()
                } else {
                    String::new()
                },
                reason: if tab == 0 && id == 6 {
                    "Set by the G1000".into()
                } else {
                    String::new()
                },
                column: usize::from(id >= 3),
                enabled: !(tab == 0 && id == 6 || tab == 2 && id >= 3),
                plus_enabled: true,
                visible: true,
                mask: PRESS | RELEASE,
                plus_mask: PRESS | RELEASE,
                tone: 0,
                value: if tab == 0 && id == 5 {
                    Some((65., 0., 100.))
                } else {
                    None
                },
            })
            .collect();
        let readouts=match tab{0=>vec![],1=>vec![Readout{text:"Total weight: 2,684 lb\nPayload: 325 lb\nFuel weight: 456 lb\n\nLeft tank: 38.0 US gal\nRight tank: 38.0 US gal".into(),tone:0}],2=>vec![Readout{text:"Left brake requires attention.".into(),tone:2}],_=>vec![Readout{text:match tab{3=>"Station 4 · Left wing\n\nInspect the tire, brake and fuel sample.\nUse A / Shift+A to step between stations.",4=>"Oil temperature: 166 °F\nOil pressure: 62 psi\nBattery: 24.3 V",5=>"COM 1 active: 124.850\nCOM 1 standby: 121.900\n\nNAV 1 active: 113.60\nNAV 1 standby: 110.50",6=>"Emergency pause is armed.\nAltitude, time and distance rules are off.",_=>"Aircraft notes\n\nSample text for layout review.\nNo aircraft assets or captured state are included."}.into(),tone:0}]};
        let page = Page {
            title: title.to_string(),
            tab: Some(tab),
            section: 0,
            ready: true,
            opacity: 1.,
            rows,
            readouts,
            report: if tab == 2 {
                "Inspection complete\n\nEngine oil and fuel system are in good condition.\nThe left brake shows wear and can be serviced here.\n\nThis is synthetic preview data.".into()
            } else {
                String::new()
            },
        };
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
            out.join(format!("{tab}.json")),
            serde_json::to_vec(&json!({"size":[1000,800],"textures":textures,"meshes":meshes}))
                .unwrap(),
        )
        .unwrap();
        output.textures_delta.clear();
    }
}
