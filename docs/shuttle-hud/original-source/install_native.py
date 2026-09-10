"""Install the native HUD into the existing derivative, with the aircraft unloaded.

The original aircraft is read-only. Requires Pillow from the bundled runtime.
"""
from pathlib import Path
from PIL import Image
import re,shutil,json,hashlib
ROOT=Path(r'D:\X-Plane 12');HERE=Path(__file__).parent
SOURCE=ROOT/'Aircraft/OrgForum/Space Shuttle-FX-V12'
DEST=ROOT/'Aircraft/OrgForum/Space Shuttle F-SIM HUD'
assert SOURCE.is_dir() and DEST.is_dir() and SOURCE!=DEST
opt={}
for line in (HERE/'hud-optics.txt').read_text().splitlines():
    if line and not line.startswith('#'):
        k,v=line.split();opt[k]=float(v)
s=(SOURCE/'Orbiter_Glider.acf').read_text()
def prop(k,v):
    global s
    s,n=re.subn(r'^P '+re.escape(k)+r' [^\n]+',f'P {k} {v}',s,flags=re.M)
    assert n==1,k
prop('acf/_name','Space Shuttle - F-SIM HUD')
for i,k in enumerate(['eye_x','eye_y','eye_z']):prop(f'acf/_pe_xyz/{i}',f'{opt[k]/.3048:.9f}')
for k in ['left','right','top','bottom']:prop(f'acf/_hud_fov_{k}_rat',f'{opt[k]:.9f}')
prop('acf/_hud_region_index',0)
for k,v in {'left':opt['panel_left'],'right':opt['panel_left']+opt['panel_size'],'bottom':opt['panel_bottom'],'top':opt['panel_bottom']+opt['panel_size']}.items():prop(f'acf/_hud_panel_{k}_px',f'{v:.9f}')
prop('acf/_cockpit_type',2)
prop('acf/_elev1_dn','18.000000000')
# Calibrated native linear strut damping for this lightweight landing case.
# Stored source coefficients are otherwise ignored while automatic damping is on.
prop('acf/_use_cus_gear_damping',1)
for i in [0,1,2]:
    key=f'_gear/{i}/_damp'
    original=float(re.search(r'^P '+re.escape(key)+r' ([^\n]+)',s,re.M)[1])
    prop(key,f'{original*5:.9f}')
for i in [0,35]:prop(f'_obja/{i}/_v10_att_file_stl','../Orbiter_FSim_Cockpit.obj')
prop('_obja/18/_obj_flags',8194)
prop('_obja/18/_v10_att_file_stl','Shuttle_FSim_Transp.obj')
panel=s.split('PANEL_2D_BEGIN\n',1)[1].split('PANEL_2D_END',1)[0]
assert 'PANEL_3D_BEGIN\nPANEL_3D_END' in s
s=s.replace('PANEL_3D_BEGIN\nPANEL_3D_END','PANEL_3D_BEGIN\n'+panel+'PANEL_3D_END')
(DEST/'Orbiter_Glider.acf').write_text(s)
# The extra atlas space is separate from every original instrument. The cockpit
# UVs are rescaled horizontally so all existing instrument pixel mappings remain.
shutil.copytree(SOURCE/'COCKPIT',DEST/'COCKPIT_3D',dirs_exist_ok=True)
im=Image.open(SOURCE/'COCKPIT/-PANELS-/panel.png').convert('RGBA')
assert im.size==(2048,2048)
atlas=Image.new('RGBA',(4096,2048),(0,0,0,0));atlas.paste(im,(0,0))
atlas.save(DEST/'COCKPIT_3D/-PANELS-/panel.png')
s=(SOURCE/'Orbiter_Cockpit.obj').read_text()
s,n=re.subn(r'^TRIS\s+1182\s+12\s*$', '# Removed obsolete flat HUD instrument faces; the real glass is the native combiner.',s,flags=re.M);assert n==1
s=s.replace('POINT_COUNTS','GLOBAL_cockpit_lit\nPOINT_COUNTS',1)
lines=[]
for line in s.splitlines():
    if line.startswith('VT'):
        a=line.split();a[7]=f'{float(a[7])*.5:.9f}';line=' '.join(a)
    lines.append(line)
(DEST/'Orbiter_FSim_Cockpit.obj').write_text('\n'.join(lines)+'\n')
s=(SOURCE/'Objects/Shuttle_Transp.obj').read_text()
s=s.replace('TRIS\t0 4902','TRIS 0 4620\nATTR_hud_glass\nTRIS 4620 6\nATTR_hud_reset\nTRIS 4626 6\nATTR_hud_glass\nTRIS 4632 6\nATTR_hud_reset\nTRIS 4638 264')
assert s.count('ATTR_hud_glass')==2
(DEST/'Objects/Shuttle_FSim_Transp.obj').write_text(s)
plugin=DEST/'plugins/ShuttleHUD'
shutil.copy2(HERE/'hud-optics.txt',plugin/'hud-optics.txt')
shutil.copy2(ROOT/'Output/shuttle-hud-20260909/win.xpl',plugin/'64/win.xpl')
manifest={str(p.relative_to(DEST)):hashlib.sha256(p.read_bytes()).hexdigest() for p in [DEST/'Orbiter_Glider.acf',DEST/'Orbiter_FSim_Cockpit.obj',DEST/'Objects/Shuttle_FSim_Transp.obj',DEST/'COCKPIT_3D/-PANELS-/panel.png',plugin/'hud-optics.txt',plugin/'64/win.xpl']}
(ROOT/'Output/shuttle-flare-20260910/install-current.json').write_text(json.dumps(manifest,indent=2))
print('Installed native HUD on existing glass; original panel mapping retained')
