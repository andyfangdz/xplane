# Runway alignment capture harness

## SDK 4.4 desktop check

On Windows, build the four candidate release DLLs and close X-Plane. From the repository root run:

```powershell
.\plugins\shuttle-hud\validation\runway-alignment\Run-NativePanel.ps1 -Name native-panels-check -Ack "<who authorized the desktop run and when>"
```

The runner takes the flight-test harness lock, generates a disposable Shuttle at a daylight KEDW 22L approach, and loads the candidate Shuttle HUD, Power-Off 180 HUD, Position Aircraft and XGS. It temporarily substitutes uniquely named copies of the two global plug-ins, backs up preferences and uses the harness recovery manifest. The Power-Off 180 HUD receives a fixed 75-value snapshot from a script in the disposable aircraft. This exercises its real painter without modifying the installed SR20 or flying a guidance test.

When `Output/performance-tests/<name>/xgs-ready` appears, choose **Plugins → Landing Speed Rust → Preview Overlay**. Once the preview appears, create the file `xgs-selected` in the same output directory. This captures the real XGS overlay with its sample KPHL touchdown. Check the two close boxes, 50-foot and touchdown text and translucent backdrop.

To rerun individual surfaces, pass `-Cases cockpit,fullscreen`, `-Cases poweroff180,position` or `-Cases xgs` with a fresh run name. Each requested surface writes a result even if an earlier one fails. Only the XGS case waits for a menu selection.

Review the off/on screenshots of the Shuttle cockpit and fullscreen HUD for transparency, clipping and runway alignment. The driver checks newly visible green pixels, advancing draw counters and runway projection. The Power-Off 180 check compares the title glyphs against a pre-migration SR20 capture and detects vertically mirrored letters. The Position Aircraft check measures the pixels changed by opening the dialog. Review text and alpha edges at full size. These captures cover the desktop at its current scale. The run saves DLL hashes, dataref readbacks, screenshots and the shutdown and restoration receipts. To recover a stopped session manually, use `tools/flight-test-harness/scripts/Restore-Session.ps1 -RunDirectory "<run directory>"`.

## Historical captures

Archived environment-specific native display checks. These use the existing local `flight_test` Web API client and screenshot helper, an aircraft copy under `Aircraft/Testing`, and the exact scenery backup/exclusion helper in the local run directory. The archived files are evidence of orchestration, not a standalone installer or flight controller. Read the [acceptance contract](../../../../docs/shuttle-hud/runway-alignment/ACCEPTANCE.md) before adapting a new test run. They are not installed in the release aircraft.
