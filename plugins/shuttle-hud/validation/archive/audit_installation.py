"""Read-only source and derivative audit, usable before and after final install."""
from pathlib import Path
import hashlib,json,re,sys
from PIL import Image
R=Path(r'D:\X-Plane 12');RUN=Path(__file__).parent
SRC=R/'Aircraft/OrgForum/Space Shuttle-FX-V12';DST=R/'Aircraft/OrgForum/Space Shuttle F-SIM HUD'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
original=json.loads((R/'Output/shuttle-hud-20260909/original-hashes.json').read_text())
changed=[];missing=[]
for rel,digest in original.items():
 p=SRC/rel
 if not p.is_file():missing.append(rel)
 elif sha(p)!=digest:changed.append(rel)
def props(p):return {m[1]:m[2] for m in re.finditer(r'^P (\S+) (.*)$',p.read_text(),re.M)}
a=props(SRC/'Orbiter_Glider.acf');b=props(DST/'Orbiter_Glider.acf')
diff={k:{'source':a.get(k),'derivative':b.get(k)} for k in a.keys()|b.keys() if a.get(k)!=b.get(k)}
allowed={'acf/_name','acf/_cockpit_type','acf/_elev1_dn','acf/_hud_region_index','_obja/18/_obj_flags','_obja/18/_v10_att_file_stl','_obja/0/_v10_att_file_stl','_obja/35/_v10_att_file_stl'}
allowed|={f'acf/_pe_xyz/{i}' for i in range(3)}
allowed|={'acf/_use_cus_gear_damping'}|{f'_gear/{i}/_damp' for i in range(3)}
allowed|={f'acf/_hud_fov_{k}_rat' for k in ['left','right','top','bottom']}
allowed|={f'acf/_hud_panel_{k}_px' for k in ['left','right','top','bottom']}
derivative_files={str(p.relative_to(DST)).replace('\\','/'):sha(p) for p in DST.rglob('*') if p.is_file()}
extra=sorted(set(derivative_files)-set(original))
changed_derivative=sorted(k for k in original if k in derivative_files and original[k]!=derivative_files[k])
source_panel=Image.open(SRC/'COCKPIT/-PANELS-/panel.png').convert('RGBA')
new_panel=Image.open(DST/'COCKPIT_3D/-PANELS-/panel.png').convert('RGBA')
panel_preserved=(new_panel.size==(4096,2048) and new_panel.crop((0,0,2048,2048)).tobytes()==source_panel.tobytes())
def vertices(p):return [[float(v) for v in line.split()[1:]] for line in p.read_text().splitlines() if line.startswith('VT')]
old_v=vertices(SRC/'Orbiter_Cockpit.obj');new_v=vertices(DST/'Orbiter_FSim_Cockpit.obj')
cockpit_preserved=len(old_v)==len(new_v) and all(a[:6]==b[:6] and a[7:]==b[7:] and abs(a[6]*.5-b[6])<1e-9 for a,b in zip(old_v,new_v))
glass_geometry_preserved=vertices(SRC/'Objects/Shuttle_Transp.obj')==vertices(DST/'Objects/Shuttle_FSim_Transp.obj')
result={'source_file_count':len(original),'source_missing':missing,'source_changed':changed,
 'source_verified':not missing and not changed,'acf_changes':dict(sorted(diff.items())),
 'original_atlas_pixels_preserved':panel_preserved,'cockpit_geometry_and_instrument_uv_mapping_preserved':cockpit_preserved,'glass_geometry_preserved':glass_geometry_preserved,
 'unexpected_acf_changes':sorted(set(diff)-allowed),'derivative_changed_original_paths':changed_derivative,
 'derivative_added_paths':extra,'source_init_unchanged':sha(SRC/'plugins/xlua/scripts/Shuttle_Init/Shuttle_Init.lua')==sha(DST/'plugins/xlua/scripts/Shuttle_Init/Shuttle_Init.lua'),
 'no_validation_controller_in_release':not any('Final_Demo' in k or 'SceneryProbe' in k for k in derivative_files),
 'compiled_binary_sha256':sha(R/'Output/shuttle-hud-20260909/win.xpl'),
 'installed_binary_sha256':sha(DST/'plugins/ShuttleHUD/64/win.xpl'),
 'scenery_ini_sha256':sha(R/'Custom Scenery/scenery_packs.ini')}
label=sys.argv[1] if len(sys.argv)>1 else 'current'
(RUN/f'audit-{label}.json').write_text(json.dumps(result,indent=2))
print(json.dumps({k:v for k,v in result.items() if k not in ['derivative_added_paths','acf_changes']},indent=2))
assert result['source_verified'] and not result['unexpected_acf_changes']
assert panel_preserved and cockpit_preserved and glass_geometry_preserved

assert result['source_init_unchanged'] and result['no_validation_controller_in_release']
assert result['compiled_binary_sha256']==result['installed_binary_sha256']
assert (R/'Custom Scenery/scenery_packs.ini').read_bytes()==(RUN/'scenery_packs.ini.original').read_bytes()
baseline=json.loads((RUN/'baseline-hashes.json').read_text())
allowed={'plugins/ShuttleHUD/64/win.xpl','HUD_ACCEPTANCE.md','HUD_LIFECYCLE.md','HUD_README.md','START_HERE.md'}
delta=sorted(k.replace(chr(92),'/') for k in baseline if sha(DST/k)!=baseline[k]);assert not set(delta)-allowed,delta
result['changed_since_136']=delta
(RUN/f'audit-{label}.json').write_text(json.dumps(result,indent=2))
print('PASS: original 430 files, baseline ACF/geometry/atlas, release binary, no helper, exact scenery')
