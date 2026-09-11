"""Regenerate checked-in Rust comparison fixtures from the archived C++ source.

Only this optional verification tool needs Zig. The shipping plugin is all Rust.
"""
from pathlib import Path
import argparse,csv,hashlib,json,subprocess

P=Path(__file__).resolve().parents[1]
R=P.parents[1]
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--zig',default='zig')
args=parser.parse_args()
out=P/'tests/fixtures';out.mkdir(parents=True,exist_ok=True)
work=R/'target/shuttle-reference';work.mkdir(parents=True,exist_ok=True)
reference=P/'reference/cpp'
cpp=(reference/'shuttle_hud.cpp').read_text(encoding='utf-8')

def compile_run(name,source,*arguments):
    file=work/(name+'.cpp');file.write_text(source,encoding='utf-8')
    exe=work/(name+'.exe')
    subprocess.run([args.zig,'c++','-std=c++17','-O2','-I',str(reference),str(file),'-o',str(exe)],check=True)
    result=subprocess.run([str(exe),*map(str,arguments)],check=True,capture_output=True)
    (out/(name+'.csv')).write_bytes(result.stdout)

common='''#include "landing_guidance.hpp"
#include "hud_presentation.hpp"
#include <iostream>
#include <iomanip>
#include <fstream>
#include <sstream>
#include <vector>
#include <array>
#include <map>
#include <cstdio>
using namespace shud;
std::vector<double> row(const std::string&s){std::vector<double> v;std::istringstream in(s);std::string field;while(std::getline(in,field,','))v.push_back(std::stod(field));return v;}
'''
compile_run('path-reference',common+'''int main(){std::cout<<std::setprecision(17);std::vector<double> points;for(int x=-8500;x<1000;x+=13)points.push_back(x);for(double x:{LandingPath::circleStart,LandingPath::expStart})for(double d:{-1e-5,0.,1e-5})points.push_back(x+d);for(double x:points){auto p=LandingPath::at(x);std::cout<<x<<','<<p.height<<','<<p.slope<<','<<p.curvature<<','<<p.segment<<'\\n';}}
''')
inputs=[]
traces=R/'docs/shuttle-hud/reports/shuttle-symbology-20260910'
for flight in [206,207]:
    previous=None
    for source in csv.DictReader((traces/f'trace-{flight}.csv').open()):
        t={k:float(v) for k,v in source.items()}
        reset=previous is None or abs(t['height_ft']-previous['height_ft'])>500 or abs(t['along_m']-previous['along_m'])>500
        inputs.append([int(reset),t['time'],t['along_m'],t['height_ft']/3.280839895,t['groundspeed_kt']/1.943844492,t['vy'],t['eas'],t['mass']*2.204622622,t['main_wow'],t['nose_wow'],t['bank'],t['cross_m']*3.280839895,t['gear0'],t['gear1'],t['gear2'],int(t['phase']>=3)])
        previous=t
with (work/'model-inputs.csv').open('w',newline='') as f:csv.writer(f).writerows(inputs)
compile_run('model-reference',common+'''int main(int,char**argv){std::cout<<std::setprecision(17);std::ifstream file(argv[1]);std::string line;LandingGuidance g;HudPresentation h;while(std::getline(file,line)){auto v=row(line);if(v[0]){g.reset();h.reset();}GuidanceInput i;i.time=v[1];i.along=v[2];i.height=v[3];i.groundspeed=v[4];i.vy=v[5];i.eas=v[6];i.massLb=v[7];i.mainWow=v[8]!=0;g.update(i);HudInput d;d.time=v[1];d.along=v[2];d.heightFt=v[3]*ft;d.groundspeed=v[4];d.eas=v[6];d.main=v[8]!=0;d.nose=v[9]!=0;d.bank=v[10];d.crossFt=v[11];d.gear={v[12],v[13],v[14]};d.finalFlare=v[15]!=0;d.pathErrorFt=d.heightFt-LandingPath::at(d.along).height*ft;d.gammaError=deg(std::atan2(i.vy,std::max(1.,i.groundspeed)))+20;d.stopDistance=14500/ft-d.along;h.update(d);for(double x:v)std::cout<<x<<',';std::cout<<g.speedbrake<<','<<g.gear<<','<<g.targetHeight<<','<<g.gamma<<','<<g.flareHeight<<','<<g.touchTime<<','<<g.retract3000<<','<<g.adjust500<<','<<g.finalFlare<<','<<g.phase<<','<<int(h.phase)<<','<<h.main<<','<<h.nose<<','<<h.fade<<','<<h.gearLockTime<<','<<h.captureSeconds<<','<<h.deceleration<<','<<h.requiredDeceleration<<','<<h.gearCue<<'\\n';}}
''',work/'model-inputs.csv')

# The vector helpers and the entire symbol-generation body are extracted directly
# from release 142, not independently reimplemented as a second Rust oracle.
helpers=cpp[cpp.index('struct Segment'):cpp.index('int draw(')]
helpers='\n'.join(line for line in helpers.splitlines() if not line.startswith('void strokeVertices'))
body=cpp[cpp.index(' const Point boresight='):cpp.index(' // Render the under-stroke')]
start=body.index(' XPLMSetGraphicsState(');stop=body.index(' if(!display.main&&level==0)')
body=body[:start]+' for(auto& group:segments)group.clear();segmentGroup=0;\n'+body[stop:]
globals='''
double hudLeft=-.2320574215,hudRight=.2320574215,hudTop=.1583844403,hudBottom=-.3057304027;
float heightFt,radarHeightFt,equivalent,groundVelocity,groundTrack,verticalVelocity,commandGamma,commandHeight,commandSpeedbrake;
bool onGlass;int horizontalCage,vvLimited,guidanceLimited,level;
struct Runway{std::string name;double lat,lon,endLat,endLon,width,displaced,elev,heading,length,un,ue;};
HudPresentation display;double clockValue,nzValue,actualBrake;
double val(const char*name,double fallback=0){std::string s(name);if(s=="sim/time/total_running_time_sec")return clockValue;if(s=="sim/flightmodel/forces/g_nrml")return nzValue;if(s=="sim/flightmodel2/controls/speedbrake_ratio"||s=="sim/cockpit2/controls/speedbrake_ratio")return actualBrake;return fallback;}
'''
scene_source=common+globals+helpers+'''
int main(int,char**argv){std::cout<<std::setprecision(17);std::ifstream file(argv[1]);std::string line;while(std::getline(file,line)){auto q=row(line);int id=int(q[0]);onGlass=q[1]!=0;level=int(q[2]);display=HudPresentation{};display.phase=HudPhase(int(q[3]));display.main=q[4]!=0;display.nose=q[5]!=0;display.automatic=q[6]!=0;display.fade=q[7];display.gearCue=int(q[8]);display.deceleration=q[9];display.requiredDeceleration=q[10];double heading=q[11],pitch=q[12],roll=q[13],along=q[14],cross=q[15],alt=q[16];heightFt=q[17];radarHeightFt=q[18];bool useRadar=q[19]!=0;equivalent=q[20];groundVelocity=q[21];groundTrack=q[22];verticalVelocity=q[23];commandGamma=q[24];commandHeight=q[25];commandSpeedbrake=q[26];actualBrake=q[27];horizontalCage=int(q[28]);clockValue=q[29];nzValue=q[30];
Runway r{};r.heading=238.11574214786276;r.un=-.528204999290666;r.ue=-.8491168816625587;r.elev=694.69;r.length=4570;r.displaced=542;
double wheel=heightFt/ft,gs=groundVelocity*kt;View v{};v.heading=heading;v.pitch=pitch;v.roll=roll;v.fx=960/(hudRight-hudLeft);v.fy=960/(hudTop-hudBottom);v.center={480-hudLeft*v.fx,100+hudTop*v.fy};
'''+body+'''
for(int group=0;group<2;++group)for(std::size_t k=0;k<segments[group].size();++k){auto s=segments[group][k];std::cout<<id<<','<<group<<','<<k<<','<<s.a.x<<','<<s.a.y<<','<<s.b.x<<','<<s.b.y;bool clipped=clipSegment(s,482,102,1438,group==0?835:1005);std::cout<<','<<clipped;if(clipped)std::cout<<','<<s.a.x<<','<<s.a.y<<','<<s.b.x<<','<<s.b.y;std::cout<<'\\n';}
}}
'''
# id,panel,level,phase,main,nose,auto,fade,gear cue,decel,required decel,
# heading,pitch,roll,along,cross,altitude,height,radar,use radar,eas,gs,track,vy,
# command gamma,height,speedbrake,actual speedbrake,caged,clock,Nz.
cases=[]
base=[0,1,0,1,0,0,0,0,0,0,0,238.11574214786276,-5,44,-20000,0,8000,24000,23800,0,300,230,240,-45,-20,6000,.45,.45,0,10.1,.7]
variants=[{}, {3:2,7:2.5,12:-8,13:25,17:17900}, {3:2,7:5,12:-8,13:10,17:17000},
 {3:3,7:5,12:-10,13:0,14:-9500,17:8500}, {3:4,7:5,2:1,12:-12,13:0,14:-7000,17:6000},
 {3:4,7:5,2:2,12:-10,13:0,14:-6000,17:3400}, {3:5,7:5,2:2,12:4,13:0,14:-3500,17:1900},
 {3:5,7:5,2:2,12:12,13:0,14:-500,17:150,18:145,19:1,23:-4,24:-2},
 {3:6,7:5,2:2,12:12,13:0,14:100,17:30,18:32,19:1,20:230,23:-2,24:-1},
 {3:6,7:5,2:2,6:1,12:12,13:0,14:100,17:30,18:32,19:1,20:230,23:-2,24:-1},
 {3:6,7:5,4:1,2:0,9:.15,10:.10,12:10,13:0,14:800,17:0,20:200,21:103,23:0},
 {3:6,7:5,4:1,5:1,2:0,9:.3,10:.2,12:0,13:0,14:1800,17:0,20:110,21:55,23:0},
 {2:3}, {3:6,4:1,2:1}, {3:2,7:5,28:1}, {3:5,7:5,2:2,8:1,17:150},
 {3:5,7:5,2:2,8:1,17:150,29:10.7}, {3:5,7:5,2:2,8:2,17:290},
 {3:5,7:5,2:2,8:3,17:250}, {27:1,26:0,29:10.7}, {30:2.5,29:10.7}, {30:2.5,29:10.1},
 {1:0,3:4,7:5,2:1,12:-12,13:0,14:-7000,17:6000}, {1:0,3:1,13:-44}]
for id,overrides in enumerate(variants):
    case=base.copy();case[0]=id
    for key,value in overrides.items():case[key]=value
    cases.append(case)
with (out/'scene-inputs.csv').open('w',newline='') as f:csv.writer(f).writerows(cases)
scene_source=scene_source.replace('std::string line;while(std::getline(file,line)){auto q=row(line);','std::string input_line;while(std::getline(file,input_line)){auto q=row(input_line);')
compile_run('scene-reference',scene_source,out/'scene-inputs.csv')
manifest={'source_version':142,'reference':'reference/cpp','scene_cases':len(cases),'model_frames':len(inputs),'files':{f.name:hashlib.sha256(f.read_bytes()).hexdigest() for f in out.glob('*.csv')},'source_hashes':{f.name:hashlib.sha256(f.read_bytes()).hexdigest() for f in reference.glob('*') if f.suffix in ('.cpp','.hpp')}}
(out/'PROVENANCE.json').write_text(json.dumps(manifest,indent=2)+'\n',encoding='utf-8')
print(json.dumps({k:v for k,v in manifest.items() if k not in ('files','source_hashes')}))
