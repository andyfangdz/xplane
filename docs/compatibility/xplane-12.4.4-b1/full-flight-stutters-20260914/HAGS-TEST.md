# Prepared HAGS test

Completed follow-up: see [the six-flight HAGS-off results](HAGS-RESULTS.md). The flag stayed on. Two of three second flights still stuttered, with 15 total intervals over 50 ms and a worst interval of 174.61 ms. No independent, consistent HAGS benefit is established. The remaining text preserves the original plan and startup history.

Post-reboot update, 2026-09-15 00:14 UTC: RTX 5090 HAGS is verified inactive (`Enabled:False`, `HwSchMode=1`). Windows changed from 26200.9278 to 26200.9445; NVIDIA 616.92 and display mode are unchanged. The fresh harness check suite passed. The first attempt, `XPT_HAGSOFF_DXGI_01_20260915`, stopped before either measurement because XPlane Map Enhancement was not running and its virtual `terrain` folders were absent. All simulator state and the temporary NVIDIA override were restored and the three persistent preference hashes match. This attempt supplies no performance result. The existing scenery app launch is waiting for its Windows administrator prompt; the three-process comparison remains pending. Evidence: `V:\src\xplane\build\stutter-hags-20260915\postboot-preflight.json` and `startup-failure-audit.json`.

Status at 2026-09-14 23:58 UTC: **user approved HAGS off and this Windows restart; the toggle is now off, pending restart**. Windows Settings displayed the restart-required message and registry readback verified `HwSchMode` DWORD 1. The original setting had no registry override and DxDiag confirmed HAGS active. The original state is saved in `V:\src\xplane\build\stutter-fix-20260914\hags-before.json`; preserve it when restoring defaults. No post-reboot measurement has run yet.

Windows Update already has KB5126052 and KB5124008 (target build 26200.9445) pending restart. This reboot can change both Windows and HAGS. Record the post-reboot build and do not attribute an improvement solely to HAGS without an on/off comparison on the same updated build. Details are retained in `hags-reboot-confound.json` beside the original-state snapshot.

1. Finish and verify all simulator and WPR recovery. Save the current graphics-setting state. In Windows Settings, System → Display → Graphics, turn **Hardware-accelerated GPU scheduling** off. Do not change other settings. Restart only when the user is ready; do not force-close unsaved applications.
2. After restarting, verify DxDiag says HAGS `Enabled:False` on the RTX 5090 and record the new boot time, driver version and graphics preference hashes. A changed toggle before reboot is not proof the scheduler changed.
3. Keep `--gfx_no_defragment`, cache 10 GB, maximum textures, full scenery, the same 2D flight card and temporary TelemFFB isolation. For the direct comparison against this investigation, temporarily select the same DXGI presentation method using the verified wrapper below. Changing presentation method and HAGS together would confound the comparison.
4. Run the same two-flight workload in three fresh processes, without WPR or video, using unique names. Require valid focus/2D/QPC/continuous-exposure checks and verified restoration for every process. Preserve landing misses and all errors. Compare per-flight distributions, >50/>100 ms counts per minute and worst intervals; do not infer a cure from a quiet first flight.
5. If there is a large consistent improvement, confirm by returning HAGS to its original state, restarting with the user's approval, and repeating the workload. A separate trace can determine whether GPU completion and graphics-memory preparation waits changed. Do not mix traced and untraced rates.
6. Keep only a demonstrated improvement, with the user's intended final setting made explicit. If HAGS does not help, restore the original absent registry override/default-enabled state and verify the active scheduler after restart. No global UAC or privileged tracing-task permissions need to change.

The existing wrapper is `V:\src\xplane\build\stutter-fix-20260914\Run-PresentTest.ps1`. For example, after validating HAGS off:

```powershell
& 'V:\src\xplane\build\stutter-fix-20260914\Run-PresentTest.ps1' `
  -Name XPT_HAGSOFF_DXGI_01_20260914 -Method DXGI
```

It uses the original harness transaction and restores the exact NVIDIA override state in `finally`. The NVIDIA backup/readback records are stored alongside the wrapper and copied into each run. HAGS itself is a separate Windows setting and is not restored by that wrapper. The restricted installed WPR task remains scoped to tracing; never repurpose it to edit Windows graphics settings or restart the computer.

At the follow-up check the scenery app and consent prompt had both closed, without the virtual folders mounting. The app must be started successfully before retrying; no post-reboot flight timing is available yet.
