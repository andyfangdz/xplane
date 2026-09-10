"""Native OBJ8/DSF ball-bar lights; JSC-23266 Rev B, figures 3.5-1/2.

No airport or library is replaced. The single-object origin samples the runway
centerline at bar station, so all lamps share the manual's elevation datum.
"""
from pathlib import Path
import argparse, csv, math, json, subprocess, hashlib
from PIL import Image

HERE=Path(__file__).resolve().parent
RUN=HERE/'../../target/shuttle-landing-aids-build'
OUT=HERE/'../../target/Shuttle Landing Aids'
RUNWAYS=HERE/'runways.csv'
TOOL=Path('DSFTool.exe')
FT=.3048

def build():
    (OUT/'objects').mkdir(parents=True,exist_ok=True)
    tex=Image.new('RGB',(16,16),(64,66,68));tex.save(OUT/'objects/fixture.png')
    vt=[];idx=[];lights=[]
    def box(cx,cy,cz,sx,sy,sz,tilt=0):
        a=math.radians(tilt);ca=math.cos(a);sa=math.sin(a)
        def rot(x,y,z):return (x,y*ca-z*sa,y*sa+z*ca)
        for normal,corners in [
            ((0,0,1),[(-1,-1,1),(1,-1,1),(1,1,1),(-1,1,1)]),
            ((0,0,-1),[(1,-1,-1),(-1,-1,-1),(-1,1,-1),(1,1,-1)]),
            ((1,0,0),[(1,-1,1),(1,-1,-1),(1,1,-1),(1,1,1)]),
            ((-1,0,0),[(-1,-1,-1),(-1,-1,1),(-1,1,1),(-1,1,-1)]),
            ((0,1,0),[(-1,1,1),(1,1,1),(1,1,-1),(-1,1,-1)]),
            ((0,-1,0),[(-1,-1,-1),(1,-1,-1),(1,-1,1),(-1,-1,1)])]:
            start=len(vt);n=rot(*normal)
            for x,y,z in corners:
                x,y,z=rot(x*sx/2,y*sy/2,z*sz/2)
                vt.append((cx+x,cy+y,cz+z,*n,.5,.5))
            idx.extend(start+k for k in [0,1,2,0,2,3])
    def assembly(x,y,z,count,color):
        # PAR-56 lamps, 10.5-inch centers: JSC-23266 figure 3.5-2.
        box(x,(y-8)/2,z,.12,y+8,.12)
        box(x,y-.14,z,count*.2667,.06,.10)
        for j in range(count):
            lx=x+(j-(count-1)/2)*.2667
            box(lx,y-.10*math.sin(math.radians(4)),z-.10*math.cos(math.radians(4)),.21,.21,.20,-4)
            lights.append({'x':lx,'y':y,'z':z,'rgb':color})
    for k in range(6):assembly(-(200+15*k)*FT,2*FT,0,4,(1,0,0))
    assembly(-200*FT,15*FT,500*FT,3,(1,1,1))
    obj=['A','800','OBJ','TEXTURE fixture.png',f'POINT_COUNTS {len(vt)} 0 0 {len(idx)}']
    obj+=['VT '+' '.join(f'{v:.7f}' for v in row) for row in vt]
    obj+=['IDX '+str(i) for i in idx]
    obj+=['ATTR_LOD 0 6500','ATTR_no_blend','TRIS 0 '+str(len(idx))]
    for light in lights:
        x,y,z=light['x'],light['y'],light['z'];r,g,b=light['rgb']
        obj.append(f'LIGHT_PARAM spot_params_bb_day_pm {x:.7f} {y:.7f} {z:.7f} {r} {g} {b} 50000cd 0 0.0697565 0.9975641 0')
    (OUT/'objects/ball_bar.obj').write_text('\n'.join(obj)+'\n',encoding='ascii')
    placements=[];tiles={}
    for row in csv.reader(RUNWAYS.open()):
        name=row[0];lat,lon,elat,elon,width,displaced,elev=map(float,row[1:])
        north=(elat-lat)*111120;east=(elon-lon)*111120*math.cos(math.radians((lat+elat)/2))
        length=math.hypot(north,east);un=north/length;ue=east/length
        heading=math.degrees(math.atan2(east,north))%360;distance=displaced+2200*FT
        plat=lat+distance*un/111120;plon=lon+distance*ue/(111120*math.cos(math.radians(lat)))
        placement={'runway':name,'latitude':plat,'longitude':plon,'heading':heading,
                   'bar_station_ft':2200,'ball_station_ft':1700,'datum':'native terrain at runway centerline, bar station'}
        placements.append(placement);tiles.setdefault((math.floor(plat),math.floor(plon)),[]).append(placement)
    for (lat,lon),items in tiles.items():
        folder=OUT/'Earth nav data'/f'{math.floor(lat/10)*10:+03d}{math.floor(lon/10)*10:+04d}'
        folder.mkdir(parents=True,exist_ok=True)
        stem=f'{lat:+03d}{lon:+04d}';p=RUN/(stem+'-landing-aids.txt')
        lines=['A','800','DSF2TEXT','PROPERTY sim/overlay 1',f'PROPERTY sim/west {lon}',f'PROPERTY sim/east {lon+1}',f'PROPERTY sim/south {lat}',f'PROPERTY sim/north {lat+1}','PROPERTY sim/creation_agent Shuttle landing aids generator','PROPERTY sim/require_object 0/0','OBJECT_DEF objects/ball_bar.obj']
        lines+=[f"OBJECT 0 {i['longitude']:.10f} {i['latitude']:.10f} {i['heading']:.7f}" for i in items]
        p.write_text('\n'.join(lines)+'\n',encoding='ascii')
        dsf=folder/(stem+'.dsf');subprocess.run([str(TOOL),'--text2dsf',str(p),str(dsf)],check=True)
        back=RUN/(stem+'-landing-aids-roundtrip.txt');subprocess.run([str(TOOL),'--dsf2text',str(dsf),str(back)],check=True)
        assert sum(s.startswith('OBJECT ') for s in back.read_text().splitlines())==len(items)
    manifest={'source':'JSC-23266 Rev B, section 3.5, figures 3.5-1/2',
              'photometry':'Local calibration: 50000 cd per lamp, native day-visible directional billboard, aimed 4 degrees upward. Not measured Shuttle lamp photometry.',
              'placements':placements,'lamps':lights,'red_count':24,'white_count':3}
    (OUT/'README.md').write_text('# Shuttle ball/bar landing lights\n\nNative scenery for KEDW 04R/22L and KTTS 15/33. Manual dimensions: JSC-23266 Rev B, figures 3.5-1/2. Fixtures and brightness are visual approximations. Lamps face the approach at 4 degrees upward. The object origin is the runway centerline at bar station so lamp elevations share that datum. The overlay requires these essential approach aids at every scenery object-density setting.\n\nTo remove: disable only `Custom Scenery/Shuttle Landing Aids/` in scenery_packs.ini, or move this folder out of Custom Scenery with X-Plane closed. Existing airports are unchanged.\n')
    manifest['files']={str(p.relative_to(OUT)):hashlib.sha256(p.read_bytes()).hexdigest() for p in OUT.rglob('*') if p.is_file()}
    (RUN/'landing-aids-manifest.json').write_text(json.dumps(manifest,indent=2))
    print(json.dumps({'placements':placements,'red':24,'white':3},indent=2))

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--dsftool',type=Path,required=True)
    parser.add_argument('--runways',type=Path,default=RUNWAYS)
    parser.add_argument('--output',type=Path,default=OUT)
    parser.add_argument('--work',type=Path,default=RUN)
    args=parser.parse_args()
    TOOL=args.dsftool.resolve(strict=True); RUNWAYS=args.runways.resolve(strict=True)
    OUT=args.output.resolve(); RUN=args.work.resolve()
    RUN.mkdir(parents=True,exist_ok=True)
    build()
