# C182 reference measurements

This is measurement infrastructure for evaluating a future C182 simulation.
**It is not an X-Plane aircraft, an Accu-Sim port, or a validated replacement.**
No flight model, engine model, systems implementation, or visual conversion is
included here. A capture passing its integrity audit is never a fidelity pass.

`capture_p3d.cpp` observes native simulator variables through the documented
SimConnect API. It does not inject gauges or change controls. Aircraft identity
and values are requested together at approximately 10 Hz, including when paused.
Pause/running states, source version, exceptions and completeness are retained.
Native variables can differ from a custom aircraft's internal states and must be
cross-checked against its instruments before use as calibration evidence.

`load_p3d_scenario.cpp` is a separate, mutating helper: it requests a scenario
load after verifying that it connected to Prepar3D v5. A successful exit means
the request was sent without an observed SDK exception, **not** that the selected
aircraft actually loaded. In particular, the startup aircraft chooser can block
the load. Verify the loaded title and state with the observer.

## Build and capture

Requires Windows, MSVC x86 build tools, Python 3, 7-Zip, and the SimConnect SDK
redistributable shipped with the user's own Prepar3D installation. The SDK
headers and vendor binaries are not included or redistributed in this repository.

```powershell
.\Build-Observers.ps1 -Prepar3DPath <installed-simulator-directory>
<output-directory>\capture_p3d.exe <output-directory>\simconnect-runtime\SimConnect.dll <new-capture.jsonl> 30
python audit_capture.py <new-capture.jsonl> --expected-title '<exact loaded aircraft title>'
python -m unittest discover -s . -p test_audit_capture.py -v
```

Use an absolute DLL path. Choose a new evidence filename for every run. The
observer refuses to overwrite an existing file. Keep captures, private scenarios,
source hashes, purchased documentation and vendor runtimes in ignored local build
directories, not in this public repository.

The audit rejects missing or truncated captures, SDK exceptions, non-finite or
missing measurements, title mismatches, paused/unknown states, time resets,
accelerated simulation and insufficient sampling. Its statistics are descriptive.
It always reports `accepted_baseline: false` and `fidelity_matched: false` because
it does not validate a controlled test card, custom-gauge mapping, or an X-Plane
implementation.

## Fidelity requirements still outstanding

Acceptance requires independent evidence for each of these areas, with documented
initial conditions, a healthy reference aircraft, repeated runs, explicit units,
convergence criteria, tolerances and holdout cases:

- flight performance across altitude, temperature, weight, CG, power and flap
  settings; control response, trim, stalls, propeller effects and ground handling;
- cold, hot and flooded starts; mixture/power/fuel-flow curves; governor
  transients, magneto checks, plug fouling and clearing;
- individual-cylinder combustion and thermal behavior, oil viscosity/pressure,
  heat soak, detonation and damage;
- fuel selection, pumps, vents, contamination, starvation and restart;
- battery, starter and alternator behavior, electrical loads, buses, breakers,
  vacuum, pitot/static instruments and avionics failure effects;
- component wear, differential compression, maintenance, preflight defects and
  persistence across shutdown, reload and elapsed real time;
- cockpit controls, indications and sounds driven by the simulated state;
- X-Plane integration, pause/replay, disable/unload, aircraft switching and
  restoration without residual overrides.

The vendor's installed gauge DLLs do not provide an X-Plane plugin or a documented
standalone engine interface. Published feature descriptions and operating manuals
are not sufficient to prove equivalence of the internal models. A new
implementation must be described as independent and its demonstrated coverage
reported explicitly. Simulator work only; not real-world flight-planning data.
