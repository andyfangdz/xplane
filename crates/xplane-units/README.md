# xplane-units

Physical quantities for the Rust workspace, using [uom 0.38](https://docs.rs/uom/0.38.0/uom/).
The crate re-exports dimensional quantity types and unit markers, with convenient
constructors for common aviation inputs. `f64` is the default; `f32` quantities
are available for native SDK datarefs. Quantities normalize to SI internally.

```rust
use xplane_units::{feet, knots, seconds, length::foot, Length, Velocity};

let height: Length = feet(1000.0);
let speed: Velocity = knots(90.0);
let travelled: Length = speed * seconds(10.0);
let height_ft = height.get::<foot>(); // explicit units at a display/SDK boundary
// height + seconds(1.0) does not compile.
```

Construct a quantity when parsing a physical value or reading an SDK dataref.
Keep quantities in model fields and function signatures; use `.get::<unit>()`
when writing SDK datarefs, serializing a defined file format, displaying a value,
or passing into a documented legacy controller interface. A `_ft`/`_m` suffix
alone is not dimensional safety. Ratios, screen pixels and control deflections
remain ordinary numbers where appropriate.

The shared airport database, runway projections and axes use `Length` and
`Angle`. Position Aircraft keeps these quantities through pattern geometry and
converts at PAD/SDK boundaries. XGS differentiates `Velocity` over `Time` to
produce acceleration before expressing it in standard gravity. Power-off 180
HUD calculations use typed speed, path distance, power and fuel flow; converting
fuel mass to volume explicitly uses the aircraft's assumed avgas density.

`uom` 0.38 rounds the built-in pound, US gallon and mechanical horsepower
coefficients. This crate defines precise markers for those units using uom's
`unit!` extension, and matching pound/gallon flow units. Use this crate's unit
modules consistently to retain that precision. Tests check known conversions,
mixed-unit arithmetic, rate consistency, f32 round trips, and compile-time
rejection of incompatible quantities.

## Compatibility boundaries

The P180 `Config`, `Sample` and 75-float snapshot retain their accepted numeric
formats. Rounded coefficients used by the v7 flight controller and f32 snapshot
adapter live in `poweroff180::calibration`; replacing them with standard-unit
conversions can change the frozen C++ replay. The Shuttle's authored path and
control/display model retain the factors documented in `shuttle_hud::math`.
The attitude helper retains its v1.9 f32 equivalent-airspeed quantization. These
are calibration exceptions, not unit definitions for new code.

XGS's V30 rating-file fpm rounding remains shared by the parser, overlay and
log. Its distance display uses standard feet. General HUD/positioning conversions
now use standard units rather than older rounded factors, so values immediately
next to a rounding boundary can display differently.

Geodetic latitude and longitude remain in source degrees to avoid a radians
round trip before subtracting close coordinates. Shared headings and projection
reference latitudes are typed angles. PAD files, configuration cards, dataref
names and the snapshot wire layout remain unchanged.

Run `cargo test --workspace --locked` for dimensional and replay checks. These
checks are offline verification; they do not replace simulator validation of a
future controller calibration change.
