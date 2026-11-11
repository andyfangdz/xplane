use std::collections::BTreeMap;
use xplane_gauge_xml::{
    Document,
    definition::{self, MacroStyle},
    geometry,
    presentation::{self, Error, Instruction, Op},
    programs::{Programs, SourceProgram},
    template,
};

#[test]
fn macro_styles_preserve_empty_nested_values_duplicates_and_unknown_names() {
    let doc=Document::parse(br#"<Gauge><Macro Name="a">first</Macro><Macro Name="b"><MacroValue>2 &gt; 1</MacroValue></Macro><Macro Name="a">last</Macro><Macro>ignored</Macro></Gauge>"#).unwrap();
    assert_eq!(definition::macros(&doc, MacroStyle::Direct)["a"], "last");
    let nested = definition::macros(
        &doc,
        MacroStyle::Nested {
            direct_fallback: false,
        },
    );
    assert_eq!(nested["a"], "");
    assert_eq!(nested["b"], "2 > 1");
    assert_eq!(
        definition::macros(
            &doc,
            MacroStyle::Nested {
                direct_fallback: true
            }
        )["a"],
        "last"
    );
    assert_eq!(nested.len(), 2);
}

#[test]
fn keys_preserve_order_normalize_event_names_and_allow_an_empty_group() {
    let doc =
        Document::parse(br#"<Keys><On Event=" panel ">1</On><On Event="other">2</On></Keys>"#)
            .unwrap();
    let keys = definition::keys(&doc, doc.root()).unwrap();
    assert_eq!(
        keys.iter()
            .map(|k| (k.ordinal, k.event.as_str(), k.expression.as_str()))
            .collect::<Vec<_>>(),
        [(0, "PANEL", "1"), (1, "OTHER", "2")]
    );
    let empty = Document::parse(b"<Keys/>").unwrap();
    assert!(definition::keys(&empty, empty.root()).unwrap().is_empty());
    for bad in [
        r#"<Keys><On Event=" ">1</On></Keys>"#,
        r#"<Keys><Off Event="A"/></Keys>"#,
        r#"<Keys><On Event="A" Extra="B"/></Keys>"#,
        r#"<Keys><On Event="A"><Value/></On></Keys>"#,
        r#"<Keys><On Name="A"/></Keys>"#,
    ] {
        let doc = Document::parse(bad.as_bytes()).unwrap();
        assert!(definition::keys(&doc, doc.root()).is_err(), "{bad}");
    }
}

#[test]
fn mouse_bindings_and_click_masks_reject_ambiguity_without_legacy_aliases() {
    assert_eq!(definition::click_flags("").unwrap(), 0x20000000);
    assert_eq!(
        definition::click_flags("LeftSingle + LeftDrag + LeftRelease + Wheel").unwrap(),
        0x20826000
    );
    assert_eq!(
        definition::click_flags("MiddleSingle+RightDouble").unwrap(),
        0x50000000
    );
    for bad in ["LeftWheel", "Unknown", "LeftSingle+"] {
        assert!(definition::click_flags(bad).is_err());
    }
    let doc = Document::parse(
        b"<MouseArea><MouseClick><KeyEvent> TEST </KeyEvent></MouseClick></MouseArea>",
    )
    .unwrap();
    let click = definition::mouse_click(&doc, doc.root()).unwrap().unwrap();
    assert_eq!(
        definition::click_action(&doc, click).unwrap().expression,
        "(>K:TEST)"
    );
    let doc = Document::parse(b"<MouseArea><MouseClick/><MouseClick/></MouseArea>").unwrap();
    assert!(definition::mouse_click(&doc, doc.root()).is_err());
    let doc =
        Document::parse(b"<MouseClick><KeyEvent>TEST</KeyEvent><Script>1</Script></MouseClick>")
            .unwrap();
    assert!(definition::click_action(&doc, doc.root()).is_err());
}

#[test]
fn inherited_visibility_is_root_to_leaf_and_preserves_expression_boundaries() {
    let doc=Document::parse(b"<Gauge><Visibility> 1 </Visibility><Element><Visibility>2 3 &gt;</Visibility><Image><Visibility>4</Visibility></Image></Element></Gauge>").unwrap();
    let image = doc.named(doc.root(), "Image")[0];
    assert_eq!(definition::ancestors(&doc, image), [2, 0]);
    assert_eq!(
        definition::visibility_expression(&doc, image).unwrap(),
        "1 0 !=\n2 3 > 0 != and\n4 0 != and"
    );
}

#[test]
fn image_geometry_composes_clamped_shifts_clips_and_axis_without_evaluation() {
    let doc=Document::parse(br#"<Gauge><FloatPosition>10,20</FloatPosition><Element><FloatPosition>2,3</FloatPosition><Visibility>1</Visibility><Shift><Scale>2,0</Scale><Expression><Script>7</Script><Minimum>0</Minimum><Maximum>5</Maximum></Expression></Shift><ClipRect><FloatPosition>4,5</FloatPosition><Size>30,40</Size></ClipRect><Image><Axis>1,2</Axis><Visibility>2</Visibility></Image></Element></Gauge>"#).unwrap();
    let node = doc.named(doc.root(), "Image")[0];
    let image = geometry::image(&doc, node).unwrap();
    assert_eq!(image.x, "10.0 2.0 + 7 0 max 5 min 2.0 * + -1.0 +");
    assert_eq!(image.y, "20.0 3.0 + -2.0 +");
    assert_eq!(image.visibility.as_deref(), Some("1 0 != 2 0 != and"));
    assert_eq!(image.clips.len(), 1);
    assert_eq!(image.clips[0].depth, 1);
    assert_eq!(image.clips[0].x, "10.0 2.0 + 7 0 max 5 min 2.0 * + 4.0 +");
    assert_eq!((image.clips[0].width, image.clips[0].height), (30., 40.));
    for invalid in [
        "<Gauge><Rotation/><Image/></Gauge>",
        "<Gauge><Shift/><Image/></Gauge>",
        "<Gauge><ClipRect><Size>0,2</Size></ClipRect><Image/></Gauge>",
    ] {
        let doc = Document::parse(invalid.as_bytes()).unwrap();
        assert!(geometry::image(&doc, doc.named(doc.root(), "Image")[0]).is_err());
    }
    for invalid in ["1", "NaN,2", "1,inf", "1,2,3"] {
        assert!(geometry::coordinates(invalid).is_err());
    }
}

#[test]
fn program_ids_retain_duplicates_source_locations_and_macro_snapshots() {
    let mut programs = Programs::default();
    let mut macros = BTreeMap::from([("M".into(), "1".into())]);
    for source in ["one", "two"] {
        let id = programs.register(SourceProgram {
            stage: "frame".into(),
            source: source.into(),
            expression: "@M".into(),
            macros: macros.clone(),
        });
        assert_eq!(id, programs.entries().len());
    }
    macros.insert("M".into(), "2".into());
    assert_eq!(programs.entries()[1].macros["M"], "1");
    assert_eq!(programs.into_entries()[1].source, "two");
}

fn instructions(text: &str) -> Vec<Instruction> {
    let tokens = template::parse(text).unwrap();
    template::link(
        &tokens,
        0,
        |token| Ok::<_, ()>(token.text.parse::<i32>().unwrap()),
        |s| s.as_bytes().to_vec(),
    )
    .unwrap()
}
#[test]
fn rendering_only_evaluates_and_formats_active_nested_branches() {
    let code = instructions(
        "%(0)%{if}%(9)%!d!%{else}%(2)%{case}%{:1}bad%{:2}%(5)%!d!%{else}bad%{end}%{end}",
    );
    let mut calls = Vec::new();
    let mut formatted = Vec::new();
    let out = presentation::render(
        &code,
        |p| {
            calls.push(p);
            Ok::<_, ()>(f64::from(p))
        },
        |value, _| {
            formatted.push(value);
            Ok(value.to_string().into_bytes())
        },
    )
    .unwrap();
    assert_eq!(out, b"5");
    assert_eq!(calls, [0, 2, 5]);
    assert_eq!(formatted, [5.]);
    let error = presentation::render(
        &instructions("%(1)%!d!"),
        |_| Err::<f64, _>("host failed"),
        |_, _| unreachable!(),
    )
    .unwrap_err();
    assert_eq!(error, Error::Evaluation("host failed"));
}

#[test]
fn link_and_render_propagate_failures_and_enforce_output_and_nesting_limits() {
    assert!(matches!(
        template::link(
            &template::parse("%(1)text").unwrap(),
            0,
            |_| Ok::<_, ()>(1),
            |s| s.as_bytes().to_vec()
        ),
        Err(template::LinkError::MissingConsumer { offset: 2 })
    ));
    assert_eq!(
        template::link(
            &template::parse("%(1)%!d!").unwrap(),
            0,
            |_| Err::<i32, _>("registration"),
            |s| s.as_bytes().to_vec()
        ),
        Err(template::LinkError::Registration("registration"))
    );
    let code = instructions("%(1)%!d!");
    let error = presentation::render(
        &code,
        |_| Ok::<_, ()>(1.),
        |_, _| Err(Error::Policy(b"format profile")),
    )
    .unwrap_err();
    assert_eq!(error, Error::Policy(b"format profile"));
    let code = vec![Instruction {
        op: Op::Text,
        program: 0,
        text: vec![b'a'; 4096],
        number: 0.,
    }];
    assert!(presentation::render(&code, |_| Ok::<_, ()>(0.), |_, _| unreachable!()).is_err());
    let code = instructions(&"%(0)%{if}".repeat(257));
    assert!(presentation::render(&code, |_| Ok::<_, ()>(0.), |_, _| unreachable!()).is_err());
}

#[test]
fn artwork_dimensions_accept_bmp_variants_and_png_and_reject_invalid_headers() {
    struct Scratch(std::path::PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let scratch = Scratch(std::env::temp_dir().join(format!(
            "gauge-xml-{}-{}.image",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )));
    let mut bmp = [0u8; 26];
    bmp[..2].copy_from_slice(b"BM");
    bmp[14..18].copy_from_slice(&40u32.to_le_bytes());
    bmp[18..22].copy_from_slice(&64i32.to_le_bytes());
    bmp[22..26].copy_from_slice(&(-32i32).to_le_bytes());
    std::fs::write(&scratch.0, bmp).unwrap();
    assert_eq!(geometry::image_size(&scratch.0).unwrap(), (64., 32.));
    bmp[14..18].copy_from_slice(&12u32.to_le_bytes());
    bmp[18..20].copy_from_slice(&16u16.to_le_bytes());
    bmp[20..22].copy_from_slice(&8u16.to_le_bytes());
    std::fs::write(&scratch.0, bmp).unwrap();
    assert_eq!(geometry::image_size(&scratch.0).unwrap(), (16., 8.));
    let mut png = [0u8; 26];
    png[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
    png[16..20].copy_from_slice(&128u32.to_be_bytes());
    png[20..24].copy_from_slice(&256u32.to_be_bytes());
    std::fs::write(&scratch.0, png).unwrap();
    assert_eq!(geometry::image_size(&scratch.0).unwrap(), (128., 256.));
    png[16..20].fill(0);
    std::fs::write(&scratch.0, png).unwrap();
    assert!(geometry::image_size(&scratch.0).is_err());
    std::fs::write(&scratch.0, b"short").unwrap();
    assert!(geometry::image_size(&scratch.0).is_err());
}

struct NoHost;
impl xplane_rpn::Host for NoHost {
    type Error = ();
    fn get_local(&mut self, _: u32) -> Result<f64, ()> {
        panic!("unexpected variable")
    }
    fn set_local(&mut self, _: u32, _: &xplane_rpn::Value) -> Result<(), ()> {
        panic!("unexpected variable")
    }
    fn get_external(&mut self, _: u32) -> Result<xplane_rpn::Value, ()> {
        panic!("unexpected variable")
    }
    fn set_external(&mut self, _: u32, _: &xplane_rpn::Value, _: bool) -> Result<(), ()> {
        panic!("unexpected effect")
    }
}
#[test]
fn synthetic_xml_to_registered_source_to_public_rpn_vm_to_rendered_text() {
    let doc = Document::parse(
        br#"<Gauge><Macro Name="VALUE">21 2 *</Macro><Text>Value: %(@VALUE)%!d!</Text></Gauge>"#,
    )
    .unwrap();
    let macros = definition::macros(&doc, MacroStyle::Direct);
    let mut programs = Programs::default();
    let tokens = template::parse(doc.text(doc.root(), "Text").unwrap()).unwrap();
    let linked = template::link(
        &tokens,
        0usize,
        |token| {
            Ok::<_, ()>(programs.register(SourceProgram {
                stage: "readout".into(),
                source: format!("synthetic.xml/Text@{}", token.offset),
                expression: token.text.clone(),
                macros: macros.clone(),
            }))
        },
        |s| s.as_bytes().to_vec(),
    )
    .unwrap();
    let out = presentation::render(
        &linked,
        |id| {
            let p = &programs.entries()[id - 1];
            let macros = p
                .macros
                .iter()
                .map(|(k, v)| (k.as_bytes().to_vec(), v.as_bytes().to_vec()))
                .collect();
            let parsed = xplane_rpn::source::parse(
                p.expression.as_bytes(),
                p.source.as_bytes(),
                &macros,
                Default::default(),
                |raw| std::str::from_utf8(raw).ok()?.parse().ok(),
            )
            .unwrap();
            let code: Vec<_> = parsed.into_iter().map(|p| p.instruction).collect();
            let mut vm = xplane_rpn::Vm::new(
                &code,
                &[xplane_rpn::Program {
                    first: 0,
                    count: code.len() as u32,
                }],
            )
            .unwrap();
            let xplane_rpn::Value::Number(value) = vm.evaluate(0, 100, &mut NoHost).unwrap() else {
                panic!("number required")
            };
            Ok::<_, ()>(value)
        },
        |value, _| Ok(format!("{value:.0}").into_bytes()),
    )
    .unwrap();
    assert_eq!(out, b"Value: 42");
}
