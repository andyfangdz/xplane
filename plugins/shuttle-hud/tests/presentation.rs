use shuttle_hud::presentation::*;

#[test]
fn numeric_boundaries_and_complete_display_sequence() {
    for (h, expected) in [
        (50.9, 50),
        (59.9, 50),
        (400.0, 400),
        (499.0, 400),
        (1000.0, 1000),
        (1199.0, 1000),
        (1200.0, 1200),
        (-1.0, 0),
    ] {
        assert_eq!(digital_height(h), expected);
    }
    for (h, expected) in [
        (500.0, 50.0),
        (500.1, 100.0),
        (1000.0, 100.0),
        (1000.1, 1000.0),
        (100000.0, 1000.0),
        (100001.0, 10000.0),
    ] {
        assert_eq!(altitude_step(h), expected);
    }
    assert_eq!(indicated_speed(199.9), 199);
    assert_eq!(indicated_speed(501.0), 500);
    let mut h = HudPresentation::default();
    let mut i = HudInput {
        height_ft: 20000.0,
        time: 10.0,
        bank: 40.0,
        groundspeed: 150.0,
        ..Default::default()
    };
    h.update(i);
    assert_eq!(h.phase, HudPhase::Hdg);
    assert!(h.nz_visible());
    assert_eq!(h.fade, 0.0);
    h.reset();
    i.bank = 0.0;
    i.height_ft = 18100.0;
    h.update(i);
    i.height_ft = 17900.0;
    for n in 1..=51 {
        i.time = 10.0 + f64::from(n) * 0.1;
        h.update(i);
    }
    assert_eq!(h.phase, HudPhase::Prfnl);
    assert_eq!(h.fade, 5.0);
    assert!(!h.nz_visible());
    i.height_ft = 9000.0;
    i.eas = 300.0;
    i.path_error_ft = 300.0;
    i.gamma_error = 1.0;
    h.update(i);
    assert_eq!(h.phase, HudPhase::Capt);
    for _ in 0..41 {
        i.time += 0.1;
        h.update(i);
    }
    assert_eq!(h.phase, HudPhase::Ogs);
    i.height_ft = 1800.0;
    h.update(i);
    assert_eq!(h.phase, HudPhase::Flare);
    i.height_ft = 100.0;
    i.eas = 250.0;
    h.update(i);
    assert_eq!(h.gear_cue, 1);
    i.gear = [1.0, 0.5, 1.0];
    h.update(i);
    assert_eq!(h.gear_cue, 2);
    i.gear = [1.0; 3];
    h.update(i);
    assert_eq!(h.gear_cue, 3);
    i.time += 5.0;
    h.update(i);
    assert_eq!(h.gear_cue, 0);
    i.final_flare = true;
    h.update(i);
    assert_eq!(h.phase, HudPhase::Fnlfl);
    assert!(!h.guidance_visible());
    i.automatic = true;
    h.update(i);
    assert!(h.guidance_visible());
    assert_eq!(h.level(3, 100.0, true), 3);
    assert_eq!(h.level(-1, 3900.0, true), 2);
    i.main = true;
    i.height_ft = 0.0;
    h.update(i);
    assert!(h.main && !h.nose && !h.guidance_visible());
    assert_eq!(h.gear_cue, 0);
    assert_eq!(h.level(3, 0.0, true), 0);
    assert!(h.attitude_visible(0) && h.horizon_visible(0));
    i.main = false;
    i.height_ft = 5.0;
    h.update(i);
    assert!(h.main);
    i.nose = true;
    h.update(i);
    assert!(h.nose && !h.horizon_visible(0) && !h.attitude_visible(0));
    i.nose = false;
    h.update(i);
    assert!(h.nose);
    h.ground_declutter = 2;
    assert_eq!(h.level(0, 0.0, true), 3);
    i.time = 1.0;
    i.height_ft = 15000.0;
    i.final_flare = false;
    h.update(i);
    assert!(!h.main && !h.nose);
    assert_eq!(h.fade, 5.0);
    i.main = true;
    h.update(i);
    i.main = false;
    i.replay = true;
    h.update(i);
    assert!(!h.main);
    i.main = true;
    i.height_ft = 0.0;
    h.update(i);
    i.main = false;
    i.height_ft = 150.0;
    i.along += 1000.0;
    h.update(i);
    assert!(!h.main && !h.nose);
    h.reset();
    i.time = 0.1;
    i.height_ft = 0.0;
    i.main = true;
    i.nose = true;
    h.update(i);
    i.time = 0.8;
    i.height_ft = 20.0;
    i.main = false;
    i.nose = false;
    h.update(i);
    assert!(!h.main && !h.nose);
    i.time = 50.0;
    i.height_ft = 0.0;
    i.main = true;
    h.update(i);
    i.time = 50.1;
    i.main = false;
    i.height_ft = 5.0;
    h.update(i);
    assert!(h.main);
    i.time = 51.0;
    i.height_ft = 100.0;
    h.update(i);
    assert!(!h.main && !h.nose);
}
