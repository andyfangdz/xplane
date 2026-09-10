from pathlib import Path
import json,html,hashlib,shutil
R=Path(__file__).parent;ROOT=R.parents[1];SUP=ROOT/'Support/Shuttle-HUD'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def load(n):return json.loads((R/n).read_text())
flights=[]
for n in [201,202,203,204,206,207]:
 p=R/f'analysis-{n}.json'
 if p.exists():
  d=load(p.name);d['build']=142 if n>=206 else 140;flights.append(d)
rows=[]
for d in flights:
 t=d['touchdown'];rows.append(f"| {d['trial']} / {d['build']} | {d['mass_lb']:.0f} | {t['eas']:.2f} | {t['along_m']:.1f} | {t['sink_fps']:.2f} | {d['rollout']['max_agl_m_first10']:.3f} | {'Pass' if d['accepted'] else 'Rebound limit missed'} |")
table='| Flight / build | Mass (lb) | Touchdown KEAS | Distance (m) | Sink (ft/s) | Max native AGL after contact (m) | Result |\n|---|---:|---:|---:|---:|---:|---|\n'+'\n'.join(rows)
limits="""The original port does not provide Shuttle GPC/TAEM phase words, a full HAC solution, MLS failure states or authentic braking guidance. ACQ/HDG/PRFNL are geometry estimates; CAPT uses the published broad capture gates with a forced transition at 5,000 ft, OGS uses path/gamma capture, FLARE begins at 2,000 ft and FNLFL follows the existing sink-dependent final-flare model. S-TRN and unsupported fault annunciations are not fabricated.

The flare preview uses a continuous local interpolation from the lower display edge at 3,500 ft to the nominal OGS cue at 2,000 ft, then the reconstructed nominal landing profile. The deceleration guide is v²/(2 × remaining stopping distance × g), targeting 1,000 ft before the selected runway end; its 0–0.4 g scale is a local reconstruction. Speedbrake discrepancy uses normalized native deflection × 98.6° as an approximation. Nz clears at PRFNL, an interpretation of the handbook wording corroborated by the absence of Nz in the approach footage. CSS/AUTO follows native autopilot mode plus servo engagement; the test pilot commands ordinary controls in CSS and is not Shuttle AUTO flight software.

Font shape, brightness, compressed-video optics and mission software differences remain approximate. The unchanged flight model has a small rebound: build-140 light repeats ranged from 0.745 to 0.756 m against a 0.750 m native-aircraft-AGL limit, with two failures retained. This metric is not wheel clearance. Heavy preflare still reaches about 1.66 g versus the handbook's approximate 1.3 g nominal. No physics or landing calibration was adjusted for this symbology work. These dry, zero-wind Edwards tests do not establish crosswind, wet-runway, entry/orbital, emergency, VR or full-global-plugin compatibility."""
controls="""## Controls

- **Shift+W** or **Plugins → Shuttle HUD → Show shuttle cockpit HUD**: commander HUD view.
- **Cycle manual HUD declutter**: airborne full → runway off → digital data → boresight only → full. After main-wheel contact: normal ground format → attitude off → boresight only → normal.
- **Use F-SIM automatic declutter**: restore the convenience mode (10,000/4,000 ft transitions). Default is automatic.
- **Toggle ATT REF horizontal cage**: cage lateral flight-director movement; the symbol remains square while caged.
- Display toggle, runway selection and optional full-screen HUD remain in the menu. Cockpit and full-screen views use the same phase rules. Native HUD power and electrical brightness apply; `fsim_hud/brightness` scales brightness.

Supported runways: KEDW 04R/22L and KTTS 15/33. Hiding the HUD does not disable the separate chute model. The release contains no validation pilot.
"""
changes="""Release 142 rebuilds the approach-to-rollout symbology against NASA JSC-23266 Rev B §2.12, the F-SIM HUD brief and inspected STS-125/STS-108 footage. It adds explicit phase sequencing, a five-second flight-director transition and ATT REF cage; distinct outer-path/flare indices; timed GR/GR-DN and flashing GEAR; handbook altitude steps; five-mark speedbrake pointers and mismatch flashing; and separate airborne/ground declutter cycles.

Main-wheel contact latches the rollout format, clears airborne symbols, moves speed beside the boresight and adds the deceleration scale. Nose-wheel contact selects G-prefixed groundspeed and removes pitch references. CSS final flare clears the guidance diamond and gamma triangles while keeping the velocity vector. Low airborne reloads, replay and time discontinuities reset the presentation state.

The native collimated combiner, ACF, cockpit geometry, atlas, `landing_guidance.hpp`, validation control laws and landing-aid scenery remain byte-identical to release 136. The source `Shuttle_Init.lua` remains unchanged. Presentation state advances once per simulator frame; drawing reads that state. The aircraft-local plugin owns native display callbacks and the existing temporary chute-area adjustment, and does not command airborne position, attitude, velocity or forces.
"""
readme='# Space Shuttle — native HUD, release 142\n\nSelect **Space Shuttle - F-SIM HUD**, then press **Shift+W**.\n\n'+changes+'\n'+controls+'\n## Evidence\n\nRead `D:/X-Plane 12/Output/shuttle-symbology-20260910/REPORT.html` for the native phase gallery, source decisions and complete test ledger. Each image links to telemetry and identifies an initialized card or an observed flown state. The older release-136 video remains historical evidence of the preserved landing model.\n\n'+table+'\n\n## Limits\n\n'+limits+'''\n\n## Build and restoration

Run `Support/Shuttle-HUD/build.ps1` (optional `-OutputPath`). Zig/XPSDK430 builds the plugin and runs math, guidance and presentation tests with warnings treated as errors. The final immutable build/source snapshot is `Output/shuttle-symbology-20260910/build-142`; installed and original-file hashes are in the final audit and manifest there. `install_native.py` regenerates the derivative from the untouched source; the current display update replaces only the aircraft-local plugin and documentation.

Select the untouched **Space Shuttle-FX-V12** to revert to the source aircraft. To restore just release 136, close X-Plane and copy the binary from `Output/shuttle-symbology-20260910/baseline-136/aircraft/plugins/ShuttleHUD/64/win.xpl` back into the derivative. The full baseline is retained outside `Aircraft`.

This recreation contains original vector lettering and no F-SIM code, fonts or copied artwork. Simulator use only; not real-flight guidance or a qualified Shuttle training system.
'''
SUP.joinpath('README.md').write_text(readme,encoding='utf-8')
accept='# Shuttle HUD acceptance — release 142\n\n'+changes+'\n'+table+'\n\nThe fixed landing checks remain 195–205 KEAS, touchdown 1,500–3,500 ft beyond the displaced threshold, sink below 5 ft/s, cross-track within 10 m, gear locked for at least five seconds, final flare 30–80 ft, a brief inner-path transition, native AGL below 0.75 m after contact and a controlled stop. Failed repeats remain failed; they are not hidden by accepted results.\n\n'+limits+'\n\nThe predeclared display contract is `ACCEPTANCE_SPEC.md`; verified native cards and raw traces are retained in `Output/shuttle-symbology-20260910`. The final binary was built with all three C++ test suites passing. The report distinguishes current lifecycle evidence from historical checks and failed setup sessions.\n'
SUP.joinpath('ACCEPTANCE.md').write_text(accept,encoding='utf-8')
life=SUP.joinpath('LIFECYCLE.md').read_text(encoding='utf-8')
marker='## Symbology refinement, release 142'
if marker in life:life=life[:life.index(marker)]
life+='\n\n'+marker+'\n\nThe pure presentation state adds phase sequencing, contact latches, timers and per-phase declutter. It resets on replay crossings, backwards simulation time and large repositioning. Stale contact during a low airborne reload clears above 50 ft, or above 5 ft within the first three simulation seconds; ordinary low rebounds retain the ground format. This changes presentation only. Startup logging now reports the runtime version dynamically.\n\nThe build-140 native cards verify the five-second fade, ATT REF, manual/automatic declutter, gear timing, CSS/AUTO distinction, power, dimming, off-axis clipping, view restoration and replay entry/exit. Flown telemetry records raw contact release during rebounds while displayed WOW stays latched. The only subsequent renderer-independent change is the low-start reset correction in release 142. Final build tests cover that correction.\n\nSeveral setup sessions stalled in loading/graphics, including the later reload after build-140 flights. They are retained under `Log-*startup*` and `Log-session-140-reload-stall.txt`. The latter needed identity-checked termination after a normal quit request did not complete. Their cause is unproven. They are not clean-exit evidence. The final release readback, SDK enable/disable and clean-exit status are recorded separately in `release-reload-142.json`, `sdk-lifecycle-142.json` and `clean-exit-142.json`.\n'
SUP.joinpath('LIFECYCLE.md').write_text(life,encoding='utf-8')
R.joinpath('RESULTS.md').write_text(accept,encoding='utf-8')

cards=[
 ('verified-140-hac','Banked HAC','Initialized card: fixed flight director, rotating pitch references, Nz and limited guidance.'),
 ('verified-140-fade-mid','Prefinal transition','Initialized transition: square flight director moves toward the velocity vector over five seconds.'),
 ('verified-140-capture','Capture','Initialized card: CAPT, altitude tape and separate guidance.'),
 ('flight-206-ogs','Outer glide slope','Observed during flight 206; paused for the unaltered screenshot.'),
 ('flight-206-flare-preview','Flare preview','Observed during flight 206: second index pair approaches the OGS pair.'),
 ('flight-206-preflare','Preflare','Observed FLARE state during flight 206.'),
 ('flight-206-inner','Inner approach','Observed low approach, digital speed/height and nominal flare reference.'),
 ('flight-206-css-final-flare','CSS final flare','Observed FNLFL: VV remains; diamond and gamma indices clear.'),
 ('flight-206-main-contact','Main-wheel contact','Observed contact: speed relocates, attitude returns, deceleration scale appears.'),
 ('flight-206-nose-contact','Nose-wheel contact','Observed contact: G groundspeed, pitch references clear.'),
 ('verified-140-auto-final-flare','AUTO final flare','Initialized native AP/servo mode: guidance remains; this is a display card, not a Shuttle AUTO landing.'),
 ('verified-140-air-declutter-3','Boresight only','Initialized manual declutter 3.'),
 ('verified-140-native-offaxis','Off-axis eye','Initialized 6 cm lateral eye shift: native combiner clips the collimated symbology.'),
 ('verified-140-fullscreen','Full-screen view','Same body-referenced symbology with camera projection.'),
 ('release-142-flash-0','Gear warning on','Installed release: GEAR and the limited diamond are visible; the VV carries its limiting X.'),
 ('release-142-flash-1','Gear warning off','Same initialized state, with simulation time advanced into the other half of the flash cycle.'),
 ('release-142-speedbrake-flash-0','Speedbrake discrepancy','Installed release: the actual pointer clears during the flash cycle while the command arrow remains.'),
 ('release-142-speedbrake-flash-1','Actual pointer returns','Same native speedbrake discrepancy, above the reconstructed 20-degree threshold.'),
]
gallery=''
for name,title,caption in cards:
 if not (R/(name+'.png')).exists():continue
 s=load(name+'.json')['state'];tele=f"Phase {s['fsim_hud/display_phase']} · {s['fsim_hud/main_wheel_height_ft']:.0f} ft · {s['fsim_hud/equivalent_airspeed_kt']:.1f} KEAS · flags {s['fsim_hud/display_flags']}"
 gallery+=f'<figure><a href="{name}.png"><img loading="lazy" src="{name}.png" alt="{html.escape(title)}"></a><figcaption><h3>{title}</h3><p>{caption}</p><a href="{name}.json">{tele} ↗ telemetry</a></figcaption></figure>'
flightrows=''.join(f"<tr><td>{d['trial']} / {d['build']}</td><td>{d['mass_lb']:,.0f}</td><td>{d['touchdown']['eas']:.2f}</td><td>{d['touchdown']['along_m']:.1f}</td><td>{d['touchdown']['sink_fps']:.2f}</td><td>{d['rollout']['max_agl_m_first10']:.3f}</td><td><a href='analysis-{d['trial']}.json'>{'Pass' if d['accepted'] else 'Rebound limit missed'}</a></td></tr>" for d in flights)
status='Final lifecycle checks pending.'
if (R/'clean-exit-142.json').exists():status='Final release reloaded, live SDK disable/re-enable verified, and dedicated simulator exited normally. See the linked readbacks and log audit.'
page='''<!doctype html><html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Shuttle HUD · Reference and native verification</title><style>
*{box-sizing:border-box}body{margin:0;background:#101816;color:#e2ebe4;font:17px/1.55 system-ui,sans-serif}main{max-width:1240px;margin:auto;padding:52px 28px}a{color:#9ce3ac}h1{font-size:clamp(36px,5vw,64px);line-height:1.05;letter-spacing:-.04em;max-width:900px}h2{margin-top:58px;font-size:30px}h3{margin:0;font-size:21px}p{max-width:960px}header small{letter-spacing:.18em;text-transform:uppercase;color:#9ce3ac}header p{font-size:20px;color:#b7c8bf}.gallery{display:grid;grid-template-columns:1fr 1fr;gap:26px}figure{margin:0;background:#192621;border:1px solid #34483d;border-radius:10px;overflow:hidden}img{display:block;width:100%;height:auto}figcaption{padding:20px}figcaption p{margin:9px 0;color:#b8c7c0;font-size:15px}figcaption a{font-size:13px}.scroll{overflow:auto}table{border-collapse:collapse;width:100%;font-size:15px}td,th{text-align:left;padding:13px;border-bottom:1px solid #34483d;white-space:nowrap}th{color:#9ce3ac}.note{border-left:3px solid #dcc282;padding:10px 20px;background:#28291f}li{margin:9px 0}code{color:#aee5c2}footer{margin-top:60px;font-size:14px;color:#a2b9ab}@media(max-width:800px){.gallery{grid-template-columns:1fr}main{padding:30px 16px}}@media print{body{background:white;color:black}a{color:#164b2c}figure{background:white;break-inside:avoid}.gallery{display:block}figure{margin-bottom:20px}}
</style><main><header><small>Native X-Plane 12 · Release 142 · 10 September 2026</small><h1>Shuttle HUD, from acquisition to rollout.</h1><p>Reference-driven symbols, timed cues and contact transitions, checked in the simulator against NASA documentation, F-SIM explanations and recorded Shuttle approaches.</p></header>'''
page+='<h2>What changed</h2>'+''.join('<p>'+html.escape(p)+'</p>' for p in changes.strip().split('\n\n'))
page+='<p><a href="RESEARCH.md">Source comparison and decisions</a> · <a href="ACCEPTANCE_SPEC.md">Predeclared display contract</a> · <a href="RESULTS.md">Detailed acceptance and limits</a></p>'
page+='<h2>Native phase gallery</h2><p>Original X-Plane screenshots. Flight 206 uses build 142 and records an observed heavy landing; the other cards deliberately initialize display states. Cards with the verified-140 prefix use the same renderer before the low-start reset and version-log corrections. The final flight retains an XPME missing-library notice outside the unobscured HUD area. Click an image to inspect full resolution.</p><div class="gallery">'+gallery+'</div>'
page+='<h2>Landing regression ledger</h2><p>Native physics with an external validation pilot using normal controls. Distances are beyond the displaced threshold; speed is KEAS. Every measured run is retained. No landing calibration was changed.</p><div class="scroll"><table><thead><tr><th>Flight / build</th><th>Mass, lb</th><th>TD KEAS</th><th>TD m</th><th>Sink ft/s</th><th>Max AGL m</th><th>Result</th></tr></thead><tbody>'+flightrows+'</tbody></table></div>'
page+='<p class="note">The 0.750 m rebound limit is narrowly repeat-sensitive in the light model. Build-140 flights 202 and 203 fail that check; 204 passes. Final-build light flight 207 also narrowly misses it at 0.755 m. Every light run meets the speed, touchdown location, sink, gear, flare and stopping checks. The rollout display remains latched during the measured rebounds.</p>'
page+='<h2>Verification and lifecycle</h2><p>'+status+'</p><ul><li>Three C++ test suites: numeric boundaries, projection, guidance, timed cues, declutter, contact and reset transitions.</li><li>Native cards: five-second FD transition, ATT REF cage, CSS/AUTO cue behavior, gear timer, declutter, replay, power, dimming, full-screen and off-axis view.</li><li>Flown contact traces: raw WOW release during rebound does not return the display to airborne format.</li></ul><p><a href="final-cards-140.json">Native view/reset cards</a> · <a href="extended-140.json">Modes, gear and declutter</a> · <a href="flight-display-checks.json">Contact latch evidence</a> · <a href="release-reload-142.json">Installed release readback</a> · <a href="sdk-lifecycle-142.json">SDK enable/disable</a> · <a href="clean-exit-142.json">Final shutdown</a> · <a href="audit-final.json">Installation audit</a></p>'
page+='<p>Failed setup and reload sessions remain in their own logs. The long build-140 session stalled on a later aircraft/plugin reload and required a bounded, identity-checked termination; it is not clean-shutdown evidence. Its cause is unproven. Failed initialization cards with the <code>card-</code> prefix are excluded from the gallery. A build-141 setup stalled before rendering; flight 205 did not reach a valid flight state and is excluded from performance results.</p>'
page+='<h2>Source fidelity and limits</h2>'+''.join('<p>'+html.escape(p)+'</p>' for p in limits.split('\n\n'))
page+='''<p>Primary definition: <a href="https://www.ibiblio.org/apollo/Shuttle/Crew%20Training/Flight%20Procedures%20App%20Land%20Roll.pdf">JSC-23266 Rev B, §2.12 and §5.3.6</a>. Simulator explanation: <a href="https://fsim.com/help/ios/hud.html">F-SIM HUD brief</a> and <a href="https://fsim.com/help/ios/landing.html">landing tutorial</a>. Visual corroboration: <a href="https://www.youtube.com/watch?v=JBk6lCikqkQ">STS-125-labelled HUD footage</a> and <a href="https://www.youtube.com/watch?v=jgPR8R28WCo">STS-108 approach footage</a>. These video labels identify the posted recordings; measurements come from simulator telemetry.</p><footer>Local simulation recreation. Original vector lettering; no F-SIM application code or artwork. Not real-flight guidance. Baseline 136, source hashes, exact builds, initialized cards and failed attempts are retained alongside this report.</footer></main></html>'''
R.joinpath('REPORT.html').write_text(page,encoding='utf-8')
print('Wrote source documentation and native evidence report.')
