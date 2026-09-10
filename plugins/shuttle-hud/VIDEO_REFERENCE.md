> Historical visual-comparison record for release 115. Release 127 supersedes the implementation and acceptance statements below with native collimated optics, phase-based flare labels, shared NASA-based landing geometry and the new accepted cockpit recording. Read ACCEPTANCE.md and Output/shuttle-correction-20260910/REPORT.html for the current result.



# STS-125 HUD video comparison



User reference: [Exclusive: STS-125 Landing HUD Cockpit View, Crew Audio](https://www.youtube.com/watch?v=JBk6lCikqkQ), supplied and visually reviewed September 10, 2026. Review used the actual browser player, paused at the times below. No video assets are bundled with the aircraft.



| Time | Visible reference | Release 115 response |

| --- | --- | --- |

| 0:03 and 0:20 | Green airspeed/altitude tapes, boresight above the velocity vector, OGS/CSS below, opposed speedbrake pointers | Retain the existing topology; increase cockpit stroke width from 3.2 to 4.6 logical units and shift colour toward green |

| 0:41 | Digital speed and radar altitude flank the vector; a visible gap precedes R | Add the gap before R in cockpit digital altitude |

| 1:02 | Horizon segments remain visible in the decluttered display; FLARE appears below; gear cue is above the guidance area | Preserve horizon separately from the blanked numbered scales; show FLARE in the local preflare/inner-approach band; move the gear cue upward |

| 1:22 | FNLFL displayed near touchdown | Retain the existing below-50-foot final-flare label |

| 1:33 | Digital speed moves beside boresight; OGS/flare and CSS labels are absent | Retain native wheel-contact speed relocation; clear cockpit mode/CSS labels after main-wheel contact |

| 1:38 | G precedes the speed during rollout | Retain the existing nosewheel-contact groundspeed transition |



These are visual observations, not a recovered shuttle software specification. Camera exposure, blur, reflections and compression make exact colour and font measurement unreliable. The gear lettering is indistinct; this build keeps the readable GR-DN/GR-TR wording while moving its location. The FLARE band (50–1,800 feet) is a local approximation, not a measured transition threshold from the video. The separate full-screen display retains the earlier F-SIM-manual presentation. Release 115 additionally initializes an already-established final with a circular velocity vector when a new flight loads, while retaining the five-second transition for continuous flight into prefinal.



Acceptance cards for the update: successful warnings-as-errors build; existing math assertions; native cockpit day/night and power-off checks; a low approach card with declutter 2, visible horizon, gear and FLARE; return to the paused reference approach; source/aircraft/scenery integrity. This is a display update, with no new aerodynamics or flight-control writes in the HUD plugin.

## STS-125 flare follow-up





The additional reference is https://www.youtube.com/watch?v=JBk6lCikqkQ . Its readable HUD frames and approximate height/speed callouts informed the flare refinement. NASA records the mission landing weight as 226,040 lb. Research, manual section 3.5 ball/bar dimensions and the actual native comparison recording are under `Output/shuttle-flare-20260910`. Read `RESEARCH.md` for source links and measurement limitations.
