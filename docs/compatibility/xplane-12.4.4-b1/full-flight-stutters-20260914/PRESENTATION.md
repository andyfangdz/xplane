# Presentation-path investigation — September 14, 2026

Follow-up at 23:58 UTC: the user approved HAGS off and restarting Windows. HAGS is now configured off (registry readback 1), pending restart. Windows also has updates pending this reboot; see the [updated test status and comparison limitation](HAGS-TEST.md). The results below describe the completed tests before that change.

Changing NVIDIA's Vulkan/OpenGL presentation method did **not** eliminate the stutters. The setting took effect, switching the observed presentation mode from `Composed: Copy with GPU GDI` to `Composed: Flip`, but long native callback intervals and delayed GPU completion remained. The temporary driver override is removed; the original X-Plane and xOrganizer settings are restored.

The next targeted experiment is **hardware-accelerated GPU scheduling (HAGS) off**. DxDiag confirms it is currently enabled on the RTX 5090. This is a hypothesis to test, not a verified fix. Applying it requires restarting Windows. The [prepared test procedure](HAGS-TEST.md) keeps the presentation method constant across that comparison.

## What was tested

Both fresh processes used X-Plane 12.4.4-b1, NVIDIA 616.92, maximum textures (`_tex_res 5`), 10 GB disk texture cache, `--gfx_no_defragment`, the saved 2D graphics configuration, full custom scenery, temporary TelemFFB isolation, and two complete calm power-off-180 flights. Harness source hashes match. Both processes produced two timing-valid flights; each first landing passed and each second landing missed the touchdown-speed criterion.

The only driver change was the existing **X-Plane** NVIDIA application profile's public setting `OGL_CPL_PREFER_DXPRESENT_ID`, `0x20D690F8`, set to 1 (prefer layered on DXGI). That profile also contains Plane Maker and Airfoil Maker executable associations. No global driver profile was changed. The original setting had no override; the driver reports its default as Auto (2). NVIDIA's pinned [public NVAPI headers](https://github.com/NVIDIA/nvapi/blob/87dca625e83fd89a983e19b904e5f3a580da90d2/NvApiDriverSettings.h) define the setting and enum values. Readback and deletion of the temporary override were tested before flying and verified after both runs.

| New run | Measurement window | Intervals >50 ms | Worst interval |
|---|---|---:|---:|
| `XPT_DXGI_B1_20260914` | First complete flight, primary window | 0 | 30.247 ms |
| Same process | Second complete flight, primary window | 7 | 148.579 ms |
| `XPT_DXGI_TRACE_20260914` | Accepted 38.364-second portion of second flight | 13 | 189.264 ms |

The first process used the passive native recorder without WPR, video or GPU-memory sampling. Its first recovery attempt conservatively refused to move files because an X-Plane process was still visible during shutdown. The process subsequently exited and the existing recovery entry point restored and verified the installation. The original recovery error is retained, so its aggregate `flight-pacing.json` deliberately remains `complete: false` even though both individual flight recordings are valid. These observations are diagnostic evidence, not a newly completed A/B benchmark.

The second process included WPR and 500 ms board-wide NVIDIA memory sampling. Its entire transaction completed successfully. Trace startup and export are excluded from the accepted window. CPU samples, scheduling events and stack records cover that window; event loss is zero. WPR overhead is uncalibrated: do not compare the table's full-flight untraced counts directly with the traced subwindow or add these counts to the original flag A/B rates.

## What the new trace shows

The DXGI capture contains 1,967 accepted presentations, all `Composed: Flip`, with the `DXGI` runtime. The large 133.417 and 189.264 ms intervals no longer contain the original long main-thread `GetFullMDL` push-lock wait. Their longest completely contained main-thread off-CPU waits are only 0.084 and 0.290 ms. Samples instead repeatedly show `SwitchToThread`, `NtDelayExecution` and performance-counter queries called from the same X-Plane executable addresses. This establishes a repeated yielding/polling pattern; without X-Plane's private symbols it does not identify the exact condition being polled.

Around the 189.264 ms interval, a queued presentation records **235.718 ms GPU time, comprising 14.029 ms GPU busy and 221.689 ms GPU wait**. A following presentation gap is 188.531 ms. PresentMon's GPU metrics are not a claim that the GPU spent that time rendering, nor are these measurements VR headset scanout timings.

The System graphics-memory worker, TID 52112, still has samples in `PageInOneAllocation → PageInAllocation → CommitResources → VIDMM_SYSMEM_SEGMENT::CommitResource → LockAllocationBackingStore → LockHeapAllocation → MmProbeAndLockPages`. This activity overlaps both inspected pauses. Unlike the previous direct ReadyThread lock-release chains, this new overlap does **not** by itself establish the exact allocation owner or a complete dependency between the polling loop and those memory operations. It supports further investigation of GPU scheduling and memory preparation; it does not prove HAGS is faulty.

The nearest sample to the 189.264 ms pause reports **17,609 MiB used and 14,579 MiB free** out of 32,607 MiB, with 420 MiB reserved. Its receipt is 134.626 ms after the interval ends. This is a coarse board-wide measurement, not per-process residency or budget evidence. It provides no basis for recommending more VRAM.

Nine newly symbolized Microsoft module images match the trace's image size, checksum and available timestamp; matching PDBs and binary hashes are retained. Existing symbol results are reused only for the same machine binaries. No private NVIDIA or X-Plane symbols were available.

## Other candidates checked

- **HAGS:** DxDiag explicitly reports `Enabled:True`. The registry's `HwSchMode` value is absent, which alone would not establish the active state. Windows 11 Pro 25H2, build 26200; last boot September 7. Microsoft describes [HAGS as a change in how GPU scheduling work is performed](https://devblogs.microsoft.com/directx/hardware-accelerated-gpu-scheduling/). An off/on experiment is more targeted than changing unrelated graphics-quality options.
- **NVIDIA overlay and game filters:** both toggles were already off in the NVIDIA app. `nvspcap64.dll` is loaded in X-Plane, but DLL presence is insufficient evidence that capture or filters are active. No overlay setting was changed.
- **Texture quality:** the earlier controlled diagnostic found fewer pauses one step below maximum, including a remaining 138 ms graphics-memory wait. This remains a mitigation with a quality tradeoff, not a cure; maximum quality is restored.
- **Disk texture cache:** retained at 10 GB throughout. This experiment supplies no reason to clear it or reduce it. Disk cache capacity and physical VRAM are different resources.
- **Driver update:** NVIDIA app reports the installed 616.92 Game Ready driver as up to date. No verified driver fix for this specific wait was found.

## Evidence and recovery

The machine-readable results, exact settings snapshots, coverage, extracted intervals, symbol checks and analysis scripts are in [presentation-evidence](presentation-evidence/). Full run directories remain under `D:\X-Plane 12\Output\performance-tests`. The new ETL is retained there as `XPT_DXGI_TRACE_20260914\segments\calm-02-diagnostic\system.etl`; it is not duplicated into the repository.

The protected reusable WPR task started unelevated, passed its nonce/CPU-readiness checks, and was stopped after tracing. All temporary simulator files, TelemFFB, aircraft state, preferences and the NVIDIA profile were restored. No HAGS, Windows security, global driver, overlay or persistent texture-quality changes were applied. No traces or messages were sent to a third party.
