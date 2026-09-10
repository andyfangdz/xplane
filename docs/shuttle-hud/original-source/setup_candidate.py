from pathlib import Path
import shutil,json,hashlib
# Historical development scaffold. Never overwrite an installed release.
root=Path(r'D:\X-Plane 12')
source=root/'Aircraft/OrgForum/Space Shuttle-FX-V12'
candidate=root/'Aircraft/Testing/Space Shuttle F-SIM HUD'
out=root/'Output/shuttle-hud-20260909'
if candidate.exists(): raise RuntimeError('Candidate already exists; preserve it')
if (root/'Aircraft/OrgForum/Space Shuttle F-SIM HUD').exists(): raise RuntimeError('Release already installed; preserve it')
hashes={p.relative_to(source).as_posix():hashlib.sha256(p.read_bytes()).hexdigest() for p in source.rglob('*') if p.is_file()}
(out/'original-hashes.json').write_text(json.dumps(hashes,indent=2))
shutil.copytree(source,candidate,ignore=lambda d,n:[f for f in n if f.endswith('.acf') and f!='Orbiter_Glider.acf'])
acf=candidate/'Orbiter_Glider.acf'
b=acf.read_bytes().replace(b'P acf/_name Space Shuttle Glider Runway Start',b'P acf/_name Space Shuttle - F-SIM HUD')
acf.write_bytes(b)
plugin=candidate/'plugins/ShuttleHUD'
(plugin/'64').mkdir(parents=True)
rows=[]
for apt,airport in [(root/'Custom Scenery/KEDW Edwards AFB XP12/Earth nav data/apt.dat','KEDW'),(root/'Custom Scenery/KTTS - Classic Shuttle Landing Facility/Earth nav data/apt.dat','KTTS')]:
    selected=False;elev=0
    for line in apt.read_text(errors='replace').splitlines():
        p=line.split()
        if not p:continue
        if p[0] in ['1','16','17']:
            selected=p[4]==airport;elev=float(p[1])*.3048
        if selected and p[0]=='100':
            width=float(p[1])
            for a,b in [(8,17),(17,8)]:
                if airport=='KEDW' and p[a] not in ['22L','04R']:continue
                if airport=='KTTS' and p[a] not in ['15','33']:continue
                rows.append(f'{airport}-{p[a]},{p[a+1]},{p[a+2]},{p[b+1]},{p[b+2]},{width},{p[a+3]},{elev}')
(plugin/'runways.csv').write_text('\n'.join(rows)+'\n')
(out/'runways.csv').write_text('\n'.join(rows)+'\n')
print(candidate);print('\n'.join(rows))
