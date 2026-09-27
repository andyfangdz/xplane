# Post-reboot HAGS-off measurements

Native after-physics QPC intervals; initial 5 seconds excluded from primary statistics; no WPR/video/GPU observer.

HAGS changed from enabled to disabled, Windows updated from 26200.9278 to 26200.9445, and the system rebooted. A matched updated-Windows HAGS-on replication is needed for causal attribution.

| Session | Flight | Timing valid | >25 ms | >50 ms | >100 ms | p99.9 (ms) | Worst (ms) | Exposure (s) |
|---|---|---|---:|---:|---:|---:|---:|---:|
| XPT_DXGI_B1_20260914 | calm-01 | True | 18 | 0 | 0 | 25.94 | 30.25 | 155.41 |
| XPT_DXGI_B1_20260914 | calm-02 | True | 45 | 7 | 3 | 46.86 | 148.58 | 154.95 |
| XPT_HAGSOFF_DXGI_02_20260915 | calm-01 | True | 25 | 0 | 0 | 26.12 | 31.07 | 155.23 |
| XPT_HAGSOFF_DXGI_02_20260915 | calm-02 | True | 30 | 0 | 0 | 35.41 | 49.94 | 155.13 |
| XPT_HAGSOFF_DXGI_03_20260915 | calm-01 | True | 40 | 0 | 0 | 27.07 | 32.05 | 155.39 |
| XPT_HAGSOFF_DXGI_03_20260915 | calm-02 | True | 67 | 9 | 3 | 49.60 | 174.61 | 154.96 |
| XPT_HAGSOFF_DXGI_04_20260915 | calm-01 | True | 22 | 0 | 0 | 26.44 | 30.35 | 155.35 |
| XPT_HAGSOFF_DXGI_04_20260915 | calm-02 | True | 44 | 6 | 2 | 43.27 | 127.41 | 155.10 |

The earlier DXGI session and the final post-reboot session have valid individual timing records but retain their original shutdown recovery guard errors; restoration later succeeded through the existing recovery entry point. This is a diagnostic comparison, not a clean completed A/B campaign. All included post-reboot records pass timing and verified restoration checks; runner completeness and original errors remain in the result JSON.

Landing acceptance is separate from frame timing. Every landing result and rejected startup is retained in the raw runs.

Primary statistics exclude the first five unpaused seconds. The result JSON also retains activation-window statistics and every primary interval above 50 ms. Shorter delays can remain visible even when no interval exceeds 50 ms.
