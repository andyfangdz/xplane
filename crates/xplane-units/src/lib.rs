//! Physical quantities shared by the plugins, backed by `uom`.
//!
//! Construct quantities at input boundaries and extract explicit units only
//! for SDK calls, file formats, presentation, or frozen controller interfaces.
//!
//! ```compile_fail
//! use xplane_units::{meters, seconds};
//! let invalid = meters(10.0) + seconds(2.0);
//! ```
//! ```
//! use xplane_units::{feet, seconds, velocity::meter_per_second, Velocity};
//! let speed: Velocity = feet(100.0) / seconds(10.0);
//! assert!((speed.get::<meter_per_second>() - 3.048).abs() < 1e-12);
//! ```
#![forbid(unsafe_code)]

#[macro_use]
extern crate uom;

pub use uom::si::f64::{
    Acceleration, Angle, Length, Mass, MassDensity, MassRate, Power, Ratio, Time, Velocity, Volume,
    VolumeRate,
};
pub use uom::si::{acceleration, angle, length, mass_density, ratio, time, velocity};
mod exact;
/// Mass units with the exact international pound definition.
pub mod mass {
    pub use crate::exact::mass::pound;
    pub use uom::si::mass::*;
}
/// Volume units with the exact US liquid gallon definition.
pub mod volume {
    pub use crate::exact::volume::gallon;
    pub use uom::si::volume::*;
}
/// Power units with mechanical horsepower derived from standard gravity.
pub mod power {
    pub use crate::exact::power::horsepower;
    pub use uom::si::power::*;
}
/// Mass flow units consistent with the exact pound definition.
pub mod mass_rate {
    pub use crate::exact::mass_rate::{
        pound_per_day, pound_per_hour, pound_per_minute, pound_per_second,
    };
    pub use uom::si::mass_rate::*;
}
/// Volume flow units consistent with the exact US liquid gallon definition.
pub mod volume_rate {
    pub use crate::exact::volume_rate::{
        gallon_per_day, gallon_per_hour, gallon_per_minute, gallon_per_second,
    };
    pub use uom::si::volume_rate::*;
}
pub mod f32 {
    pub use uom::si::f32::{Acceleration, Angle, Length, Mass, Time, Velocity};
}

macro_rules! constructors {
    ($($name:ident: $quantity:ident, $unit:path;)*) => {$(
        pub fn $name(value: f64) -> $quantity {
            $quantity::new::<$unit>(value)
        }
    )*};
}

constructors! {
    meters: Length, length::meter;
    feet: Length, length::foot;
    nautical_miles: Length, length::nautical_mile;
    degrees: Angle, angle::degree;
    radians: Angle, angle::radian;
    seconds: Time, time::second;
    knots: Velocity, velocity::knot;
    meters_per_second: Velocity, velocity::meter_per_second;
    feet_per_second: Velocity, velocity::foot_per_second;
    feet_per_minute: Velocity, velocity::foot_per_minute;
    kilograms: Mass, mass::kilogram;
    pounds: Mass, mass::pound;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_units_compose_as_physical_quantities() {
        let distance = feet(100.0) + meters(30.48);
        assert!((distance.get::<length::foot>() - 200.0).abs() < 1e-12);
        let duration = Time::new::<time::hour>(1.0);
        let travelled: Length = knots(120.0) * duration;
        assert!((travelled.get::<length::nautical_mile>() - 120.0).abs() < 1e-12);
        assert!((feet_per_minute(600.0).get::<velocity::meter_per_second>() - 3.048).abs() < 1e-12);
        assert!(
            (Volume::new::<volume::gallon>(1.0).get::<volume::liter>() - 3.785411784).abs() < 1e-12
        );
        assert!(
            (Power::new::<power::horsepower>(1.0).get::<power::watt>() - 745.6998715822702).abs()
                < 1e-10
        );
        assert!((pounds(1.0).get::<mass::kilogram>() - 0.45359237).abs() < 1e-12);
        assert!((degrees(180.0).get::<angle::radian>() - std::f64::consts::PI).abs() < 1e-12);
    }

    #[test]
    fn sdk_float_conversions_preserve_sign_and_roundtrip() {
        for value in [-300.0_f32, 0.0, 300.0] {
            let speed = f32::Velocity::new::<velocity::foot_per_minute>(value);
            let roundtrip = f32::Velocity::new::<velocity::meter_per_second>(
                speed.get::<velocity::meter_per_second>(),
            );
            assert!((roundtrip.get::<velocity::foot_per_minute>() - value).abs() < 0.0001);
        }
    }

    #[test]
    fn flow_units_agree_with_mass_and_volume_over_time() {
        let duration = Time::new::<time::hour>(1.0);
        let flow: MassRate = pounds(12.0) / duration;
        assert!((flow.get::<mass_rate::pound_per_hour>() - 12.0).abs() < 1e-12);
        let density: MassDensity = kilograms(2.72155) / Volume::new::<volume::gallon>(1.0);
        let fuel_flow: VolumeRate = (kilograms(2.72155) / duration) / density;
        assert!((fuel_flow.get::<volume_rate::gallon_per_hour>() - 1.0).abs() < 1e-12);
        assert!(((fuel_flow * duration).get::<volume::gallon>() - 1.0).abs() < 1e-12);
    }
}
