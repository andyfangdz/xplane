use xplane_plugin::DebugLogger;

static LOGGER: DebugLogger = DebugLogger::new("xgs-rs");

pub(in crate::runtime) fn log(message: &str) {
    LOGGER.log(message);
}

pub(super) fn angular_delta(from: f64, to: f64) -> f64 {
    (to - from + 180.0).rem_euclid(360.0) - 180.0
}
