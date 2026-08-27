use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use xplane_plugin::{PluginMenu, PluginStateSlot};
use xplane_sdk_sys::XPWidgetID;

use super::config::{RatingScale, Settings};
use super::datarefs::DataRefs;
use super::landing::{FiftyFootMetrics, LandingResult, LandingTracker};
use super::runway::{RunwayDatabase, TouchdownMetrics};
use super::support::log;
use super::ui::OverlayWindow;

thread_local! {
    static STATE: PluginStateSlot<PluginState> = const { PluginStateSlot::new() };
}

pub(super) struct MenuState {
    pub(super) menu: Option<PluginMenu>,
    pub(super) log_index: i32,
    pub(super) replay_index: i32,
    pub(super) duration_indices: Vec<i32>,
}

impl Default for MenuState {
    fn default() -> Self {
        Self {
            menu: None,
            log_index: -1,
            replay_index: -1,
            duration_indices: Vec::new(),
        }
    }
}

pub(in crate::runtime) struct PluginState {
    pub(super) xplane_root: PathBuf,
    pub(super) plugin_directory: PathBuf,
    pub(super) settings: Settings,
    pub(super) ratings: RatingScale,
    pub(super) datarefs: Option<DataRefs>,
    pub(super) runways: Option<RunwayDatabase>,
    pub(super) overlay: OverlayWindow,
    pub(super) tracker: LandingTracker,
    pub(super) menu: MenuState,
    pub(super) enabled: bool,
    pub(super) aircraft_icao: String,
    pub(super) aircraft_tail_number: String,
}

impl PluginState {
    pub(super) fn new(xplane_root: PathBuf, plugin_directory: PathBuf, settings: Settings) -> Self {
        Self {
            xplane_root,
            plugin_directory,
            settings,
            ratings: RatingScale::default(),
            datarefs: None,
            runways: None,
            overlay: OverlayWindow::default(),
            tracker: LandingTracker::default(),
            menu: MenuState::default(),
            enabled: false,
            aircraft_icao: String::new(),
            aircraft_tail_number: String::new(),
        }
    }

    pub(super) fn aircraft_loaded(&mut self, aircraft_path: Option<PathBuf>) {
        let Some(datarefs) = self.datarefs.as_mut() else {
            return;
        };
        self.aircraft_icao = datarefs.aircraft_icao.read_string(64);
        self.aircraft_tail_number = datarefs.aircraft_tail_number.read_string(64);
        datarefs.refresh_aircraft_specific(&self.aircraft_icao);
        self.ratings = RatingScale::for_aircraft(
            aircraft_path.as_deref().and_then(|path| path.parent()),
            &self.plugin_directory,
            &self.aircraft_icao,
        );
        self.tracker.reset(datarefs.on_ground());
        log(&format!(
            "aircraft loaded: {} {}",
            self.aircraft_icao, self.aircraft_tail_number
        ));
    }

    pub(super) fn flight_loop(&mut self, elapsed: f32) -> f32 {
        let replaying = self
            .datarefs
            .as_ref()
            .is_some_and(|datarefs| datarefs.replay.get_i32() != 0);
        if self.datarefs.is_none() {
            return 2.0;
        }
        if !self.settings.show_in_replay && replaying {
            if self.overlay.tick(elapsed, false) {
                self.hide_overlay();
            }
            return 1.0;
        }
        let (interval, update) = self.tracker.tick(
            self.datarefs.as_ref().expect("checked above"),
            self.runways.as_ref(),
            elapsed,
        );
        if self.overlay.tick(elapsed, update.teleported) {
            self.hide_overlay();
        }
        if update.result_changed && self.overlay.is_visible() {
            let lines = self.current_lines();
            self.overlay
                .update_lines(lines, self.settings.window_x, self.settings.window_y);
        }
        if update.show_overlay {
            self.show_current_result();
        }
        if update.finished && self.settings.log_enabled && !replaying {
            self.write_landing_log();
        }
        interval
    }

    pub(super) fn show_current_result(&mut self) {
        if self.tracker.result.is_none() {
            return;
        }
        let lines = self.current_lines();
        let in_vr = self
            .datarefs
            .as_ref()
            .is_some_and(|datarefs| datarefs.vr_enabled.get_i32() != 0);
        let duration = self.settings.duration();
        self.overlay.show(
            lines,
            &mut self.settings.window_x,
            &mut self.settings.window_y,
            duration,
            in_vr,
        );
    }

    pub(super) fn preview_overlay(&mut self) {
        self.tracker.result = Some(LandingResult {
            vertical_speed_mps: -0.71,
            touchdown_pitch_deg: 4.2,
            crab_angle_deg: -1.8,
            fifty_foot: Some(FiftyFootMetrics {
                ias: 74.0,
                pitch_deg: 3.7,
            }),
            g: 1.14,
            ias: 71.0,
            vls: Some(68.0),
            metrics: Some(TouchdownMetrics {
                airport: "KPHL".to_owned(),
                runway: "27R".to_owned(),
                threshold_elevation_m: 10.0,
                distance_from_threshold_m: 382.0,
                centerline_deviation_m: -1.8,
                centerline_angle_deg: 0.6,
            }),
            crossing_height_m: Some(15.0),
            nose_wheel_distance_m: Some(431.0),
        });
        self.show_current_result();
    }

    fn current_lines(&self) -> Vec<String> {
        let Some(result) = self.tracker.result.as_ref() else {
            return Vec::new();
        };
        let (multiplier, unit, toliss) = self
            .datarefs
            .as_ref()
            .map(|datarefs| {
                (
                    datarefs.ias_multiplier,
                    datarefs.ias_unit,
                    datarefs.toliss_strut.is_some(),
                )
            })
            .unwrap_or((1.0, "kts", false));
        result.lines(&self.ratings, multiplier, unit, toliss)
    }

    pub(super) fn hide_overlay(&mut self) {
        self.overlay
            .hide(&mut self.settings.window_x, &mut self.settings.window_y);
        let on_ground = self
            .datarefs
            .as_ref()
            .is_none_or(|datarefs| datarefs.on_ground());
        self.tracker.reset(on_ground);
        self.settings.save();
    }

    pub(super) fn set_vr(&mut self, in_vr: bool) {
        self.overlay.set_vr(
            in_vr,
            &mut self.settings.window_x,
            &mut self.settings.window_y,
        );
    }

    pub(super) fn overlay_root(&self) -> XPWidgetID {
        self.overlay.root()
    }

    pub(super) fn overlay_custom(&self) -> XPWidgetID {
        self.overlay.custom()
    }

    pub(super) fn draw_overlay(&self) {
        self.overlay.draw();
    }

    pub(super) fn shutdown_ui(&mut self) {
        self.overlay
            .destroy(&mut self.settings.window_x, &mut self.settings.window_y);
        self.settings.save();
    }

    fn write_landing_log(&self) {
        let Some(result) = self.tracker.result.as_ref() else {
            return;
        };
        let path = self.xplane_root.join("Output").join("xgs_landing.log");
        let Ok(mut file) = OpenOptions::new().create(true).append(true).open(&path) else {
            log(&format!("could not append {}", path.display()));
            return;
        };
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        let airport = result
            .metrics
            .as_ref()
            .map(|metrics| metrics.airport.as_str())
            .unwrap_or("");
        let rating = self.ratings.text_for(result.vertical_speed_mps);
        let (ias_multiplier, ias_unit) = self
            .datarefs
            .as_ref()
            .map(|datarefs| (datarefs.ias_multiplier, datarefs.ias_unit))
            .unwrap_or((1.0, "kts"));
        let fifty_foot = result.fifty_foot.map_or_else(
            || "50 ft IAS/pitch unavailable".to_owned(),
            |metrics| {
                format!(
                    "50 ft {:.0} {ias_unit} IAS / {:.1}° pitch",
                    metrics.ias * ias_multiplier,
                    metrics.pitch_deg
                )
            },
        );
        let _ = writeln!(
            file,
            "{timestamp} {} {} {airport} {:.3} m/s {:.0} fpm {:.1}° touchdown pitch {:+.1}° crab, {fifty_foot}, {:.3} G, {rating}",
            self.aircraft_icao,
            self.aircraft_tail_number,
            result.vertical_speed_mps,
            result.vertical_speed_mps * 196.850,
            result.touchdown_pitch_deg,
            result.crab_angle_deg,
            result.g,
        );
    }
}

pub(in crate::runtime) fn replace_state(state: Option<PluginState>) {
    STATE.with(|slot| {
        slot.replace(state);
    });
}

pub(in crate::runtime) fn with_state_mut<T>(
    callback: impl FnOnce(&mut PluginState) -> T,
) -> Option<T> {
    STATE.with(|slot| slot.with_mut(callback))
}
