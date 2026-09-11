# Runway alignment acceptance

The outline must coincide with the installed runway's landing surface in the native cockpit and full-screen HUD. Match the installed runway definition's width, near displaced threshold and opposite physical end. KEDW 22L begins 542 m beyond its pavement start; KEDW 04R and KTTS 15/33 have no near displacement. Opposite-end displacement does not shorten the landing rollout area.

Check centered, laterally offset and banked views, a closer approach, and both directions at Edwards and Kennedy. Capture simulator screenshots and readbacks; compare the outline with visible edges, threshold paint and the far end. Treat initialized display cards separately from flown landing performance.

The runway projection must use sampled scenery elevation and the simulator's camera/aircraft transforms. Suppress it if terrain or matrices are unavailable. Verify the projection independently against matrix multiplication and preserve existing guidance and presentation comparisons. Confirm declutter hides the outline at the same levels, native HUD disable/power and view transitions remain functional, no flight-path override remains set, and the simulator exits cleanly.

Install only the tested aircraft-local Rust binary. Preserve the existing ACF, native HUD optics/meshes, approach guidance, landing-aid scenery, original aircraft and simulator configuration; retain the previous binary for restoration.
