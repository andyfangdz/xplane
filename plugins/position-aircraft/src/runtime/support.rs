use xplane_plugin::DebugLogger;

static LOGGER: DebugLogger = DebugLogger::new("PositionAircraftNative");

pub(in crate::runtime) fn log(message: &str) {
    LOGGER.log(message);
}
