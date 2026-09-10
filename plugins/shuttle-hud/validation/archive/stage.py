from pathlib import Path
import shutil,hashlib,json
R=Path(__file__).parent;ROOT=R.parents[1];S=ROOT/'Support/Shuttle-HUD';D=ROOT/'Aircraft/Testing/Shuttle Symbology Trial'
assert not D.exists(),'Trial already exists'
shutil.copytree(R/'baseline-136/aircraft',D)
shutil.copy2(R/'build-137/win.xpl',D/'plugins/ShuttleHUD/64/win.xpl')
pilot=D/'plugins/xlua/scripts/Final_Demo/Final_Demo.lua';pilot.parent.mkdir(parents=True)
t=(S/'ValidationController.lua').read_text();assert 'Output/shuttle-flare-20260910' in t
pilot.write_text(t.replace('Output/shuttle-flare-20260910','Output/shuttle-symbology-20260910'))
t=(ROOT/'Output/shuttle-flare-20260910/fly_trial.py').read_text().replace('Shuttle Correction Trial','Shuttle Symbology Trial')
(R/'fly_trial.py').write_text(t)
for n in ['landing_guidance.hpp','ValidationController.lua','hud-optics.txt']:
 assert (S/n).read_bytes()==(R/'baseline-136/support'/n).read_bytes(),n
(R/'staged-hashes.json').write_text(json.dumps({str(p.relative_to(D)):hashlib.sha256(p.read_bytes()).hexdigest() for p in D.rglob('*') if p.is_file()},indent=2))
print('Isolated trial created; landing guidance, validation controls and optics preserved.')
