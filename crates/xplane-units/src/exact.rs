//! uom 0.38 rounds these factors. Keep quantities and arithmetic in uom while
//! defining precise unit markers for aviation mass, power, and fuel-flow output.
//! 1 lb = 0.45359237 kg; 1 US gal = 231 cubic inches; 1 in = 0.0254 m.
//! 1 mechanical hp = 550 ft*lbf/s using g0 = 9.80665 m/s^2.
// unit! emits both f32 and f64 implementations from each coefficient. Retain
// the f64 precision even though Clippy sees the same literal in the f32 branch.
#![allow(clippy::excessive_precision)]

pub mod mass {
    unit! { system: uom::si; quantity: uom::si::mass;
        @pound: 0.453_592_37; "lb", "pound", "pounds";
    }
}
pub mod volume {
    unit! { system: uom::si; quantity: uom::si::volume;
        @gallon: 0.003_785_411_784; "gal", "US gallon", "US gallons";
    }
}
pub mod power {
    unit! { system: uom::si; quantity: uom::si::power;
        @horsepower: 745.699_871_582_270_2; "hp", "mechanical horsepower", "mechanical horsepower";
    }
}
pub mod mass_rate {
    unit! { system: uom::si; quantity: uom::si::mass_rate;
        @pound_per_second: 0.453_592_37; "lb/s", "pound per second", "pounds per second";
        @pound_per_minute: 0.453_592_37 / 60.0; "lb/min", "pound per minute", "pounds per minute";
        @pound_per_hour: 0.453_592_37 / 3600.0; "lb/h", "pound per hour", "pounds per hour";
        @pound_per_day: 0.453_592_37 / 86400.0; "lb/d", "pound per day", "pounds per day";
    }
}
pub mod volume_rate {
    unit! { system: uom::si; quantity: uom::si::volume_rate;
        @gallon_per_second: 0.003_785_411_784; "gal/s", "US gallon per second", "US gallons per second";
        @gallon_per_minute: 0.003_785_411_784 / 60.0; "gal/min", "US gallon per minute", "US gallons per minute";
        @gallon_per_hour: 0.003_785_411_784 / 3600.0; "gal/h", "US gallon per hour", "US gallons per hour";
        @gallon_per_day: 0.003_785_411_784 / 86400.0; "gal/d", "US gallon per day", "US gallons per day";
    }
}
