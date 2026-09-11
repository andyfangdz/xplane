# xplane-hud

Pure display geometry shared by the SR20 and Shuttle HUD plugins:

- `Point`, `point`, and `rotate` describe the Y-down display plane.
- `View` projects bearings/elevations through independently calibrated horizontal
  and vertical focal lengths, including an offset optical center and camera
  reprojection.
- `Segment` clips centerlines to rectangles and expands thick strokes into quads.

Angles are degrees; coordinates, focal lengths, and widths use the caller's
design-plane units. The crate has no SDK, OpenGL, or platform dependencies.

Projection marks rays at or behind the 0.01 forward cutoff as `limited` while
retaining clamped coordinates. The Shuttle uses these coordinates for caged
symbols; the SR20's local projection adapter discards limited rays and validates
its field of view. A segment's rectangular clipping applies to its centerline;
the SR20 retains GL scissoring for complete primitives and text.

Fonts, layouts, instrument rules, guidance, native scenery matrices, and screen
or panel transforms stay in each plugin. The optional `opengl` feature of
[`xplane-plugin`](../xplane-plugin) provides scoped GL attribute/matrix restoration.

Run `cargo test -p xplane-hud` for geometry tests. Plugin scene regressions cover
the consuming displays; `cargo test -p xplane-plugin --features opengl` also checks
state restoration in a real Windows compatibility context using an invisible
window, without launching X-Plane.
