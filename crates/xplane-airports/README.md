# xplane-airports

SDK-independent airport data and runway geometry shared by the native plugins.

- `RunwayDatabase` reads the active scenery stack and resolves airport/runway
  selections, including physical ends and displaced landing thresholds.
- `GeoPoint` stores source latitude/longitude as geodetic degrees and elevation
  as a `Length`. Keeping source coordinates in degrees avoids rounding before
  subtracting nearby positions.
- `project`, `distance`, `bearing`, and `offset` provide the existing geographic
  helpers with `Length` distances and `Angle` headings.
- `LocalProjection` uses an explicit distance scale and fixed longitude reference
  latitude. Use it for calibrated runway and short-leg calculations where the
  same projection must apply to every sample. Its scale is a `Length` per
  geographic degree and its latitude argument is an `Angle`.
- `RunwayAxis` converts east/north vectors to along/cross-track coordinates and
  back. Along-track increases toward the opposite end; cross-track is positive
  to the right. Inputs and results carry `Length`, so feet and metres can be
  composed safely without conversion factors.

The Shuttle HUD uses 111,120 metres per degree, midpoint latitude for the runway
direction, and physical-end latitude for guidance positions. Power-off 180 uses
60 nautical miles per degree at runway-midpoint latitude; its navigation display
uses the same distance scale at leg-midpoint latitude. Those scales are
caller choices, independent of database lookup and the general geographic helpers.

Shared geometry accepts configured endpoints as well as scenery-derived ones.
The Shuttle's runway CSV and the power-off 180 test card continue to define their
landing datums, so loading another scenery pack does not silently change a
calibrated test. Terrain sampling and camera projection remain plugin concerns.
