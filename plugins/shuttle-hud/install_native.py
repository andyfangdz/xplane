"""Regenerate release 142 from a separately obtained original aircraft.

The source is read-only. An existing derivative is never overwritten. Incomplete
work stays under Output, outside X-Plane's flyable-aircraft scanner.
"""
from pathlib import Path
from PIL import Image
import argparse, hashlib, json, re, shutil, uuid

HERE=Path(__file__).resolve().parent

def transform(SOURCE, DEST, binary):
    opt={}
    for line in (HERE/'hud-optics.txt').read_text().splitlines():
        if line and not line.startswith('#'):
            k,v=line.split();opt[k]=float(v)
    s=(SOURCE/'Orbiter_Glider.acf').read_text()
    def prop(k,v):
        nonlocal s
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
    (plugin/'64').mkdir(parents=True,exist_ok=True)
    shutil.copy2(HERE/'hud-optics.txt',plugin/'hud-optics.txt')
    shutil.copy2(binary,plugin/'64/win.xpl')

    return plugin

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--xplane',type=Path,required=True,help='X-Plane installation directory; close X-Plane first')
    parser.add_argument('--source',type=Path,help='Original Space Shuttle-FX-V12 folder; defaults to Aircraft/OrgForum')
    parser.add_argument('--binary',type=Path,default=HERE/'../../target/shuttle-hud/win.xpl')
    parser.add_argument('--runways',type=Path,default=HERE/'runways.csv')
    parser.add_argument('--dry-run',action='store_true',help='Validate inputs and report paths without writing')
    parser.add_argument('--stage-only',action='store_true',help='Generate under Output for review; do not place in Aircraft')
    args=parser.parse_args()
    root=args.xplane.resolve(strict=True)
    source=(args.source or root/'Aircraft/OrgForum/Space Shuttle-FX-V12').resolve(strict=True)
    destination=(root/'Aircraft/OrgForum/Space Shuttle F-SIM HUD').resolve()
    binary=args.binary.resolve(strict=True)
    runways=args.runways.resolve(strict=True)
    if not (root/'X-Plane.exe').is_file():
        raise RuntimeError('The selected directory is not a Windows X-Plane installation')
    if source==destination or destination.is_relative_to(source):
        raise RuntimeError('Source and derivative must be separate directories')
    if destination.exists() and not args.stage_only:
        raise FileExistsError('Derivative already exists; preserve it. Use --stage-only to generate a separate review copy.')
    for name, expected in json.loads((HERE/'source-requirements.json').read_text(encoding='utf-8')).items():
        if hashlib.sha256((source/name).read_bytes()).hexdigest()!=expected:
            raise RuntimeError('Unsupported source aircraft revision: '+name)
    for row in runways.read_text(encoding='utf-8').splitlines():
        fields=row.split(',')
        if len(fields)!=8:
            raise ValueError('Runway CSV requires a name and seven numeric fields')
        [float(value) for value in fields[1:]]
    plan={'source':str(source),'destination':str(destination),'binary':str(binary),'stage_only':args.stage_only}
    if args.dry_run:
        print(json.dumps(plan,indent=2)); return
    staging_parent=(root/'Output/ShuttleHUD-install').resolve()
    if not staging_parent.is_relative_to(root/'Output'):
        raise RuntimeError('Staging directory escapes X-Plane Output')
    staging=staging_parent/('stage-'+uuid.uuid4().hex)
    staging_parent.mkdir(parents=True,exist_ok=True)
    shutil.copytree(source,staging,ignore=lambda directory,names:[name for name in names if name.lower().endswith('.acf') and name!='Orbiter_Glider.acf'])
    plugin=transform(source,staging,binary)
    shutil.copy2(runways,plugin/'runways.csv')
    for name in ['README.md','ACCEPTANCE.md','LIFECYCLE.md','VIDEO_REFERENCE.md']:
        shutil.copy2(HERE/name,staging/('HUD_'+name))
    (staging/'START_HERE.md').write_text('Select Space Shuttle - F-SIM HUD, then press Shift+W.\n\nSource and reports: https://github.com/andyfangdz/xplane/tree/main/plugins/shuttle-hud\n',encoding='utf-8')
    manifest={file.relative_to(staging).as_posix():hashlib.sha256(file.read_bytes()).hexdigest() for file in staging.rglob('*') if file.is_file()}
    (staging/'HUD_INSTALL_MANIFEST.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf-8')
    if not args.stage_only:
        # Both verified absolute paths belong to this installation. Rename only
        # this newly generated directory; never replace an existing aircraft.
        if not destination.is_relative_to(root/'Aircraft') or destination.exists():
            raise RuntimeError('Destination no longer permits a new installation')
        destination.parent.mkdir(parents=True,exist_ok=True)
        staging.rename(destination)
    print(json.dumps({**plan,'created':str(staging if args.stage_only else destination),'files':len(manifest)},indent=2))

if __name__=='__main__':
    main()
