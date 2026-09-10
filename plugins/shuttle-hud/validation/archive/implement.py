from pathlib import Path
R=Path(__file__).resolve().parents[2];S=R/'Support/Shuttle-HUD'
p=S/'shuttle_hud.cpp';s=p.read_text()
def swap(a,b):
 global s
 assert a in s,a[:100]
 s=s.replace(a,b)
swap('#include "landing_guidance.hpp"','#include "landing_guidance.hpp"\n#include "hud_presentation.hpp"')
swap('version=136','version=137')
swap('double fade=0,lastTime=-1,radarGround=0;','double radarGround=0;\nHudPresentation display;\nint horizontalCage=0,displayPhase=0,displayMain=0,displayNose=0,gearCue=0,displayFlags=0,controlAuto=0;\nfloat displayAltitude=0,deceleration=0,decelerationCommand=0;')
start=s.index(' auto a=p(0,-r.width/2)');end=s.index('\nvoid tapes(',start)
s=s[:start]+''' // The real symbolic runway is 300 ft wide, independent of pavement width.
 const double half=150/ft,length=std::min(15000/ft,r.length-r.displaced);
 auto a=p(0,-half),b=p(0,half),c=p(length,half),d=p(length,-half);
 for(auto edge:std::array<std::pair<Projected,Projected>,4>{{{a,b},{b,c},{c,d},{d,a}}})
  if(!edge.first.limited&&!edge.second.limited)line(edge.first.p,edge.second.p);
 auto inner=p(LandingPath::innerAim,0),outer=p(LandingPath::outerAim,0);
 if(!inner.limited)circle(inner.p,7);if(!outer.limited)circle(outer.p,7);
 if(!inner.limited&&!outer.limited)line(inner.p,outer.p);
}'''+s[end:]
swap('void tapes(double speed,double altitude,double error){double cy=535,l=590,r=1330;','void tapes(double speed,double altitude,double error){double cy=100+hudTop*960/(hudTop-hudBottom)+std::tan(rad(5))*960/(hudTop-hudBottom),l=590,r=1330;')
swap('double y=speedTapeY(speed,n);','double y=cy+(n-speed)*18;')
swap('double step=altitude>2000?1000:100,spacing=180;','double step=altitudeStep(altitude),spacing=150;')
swap('step==1000?num(a/1000)+(n==low?"K":""):num(a)','altitude>1000?num(a/1000)+"K":num(a)')
swap('void ladder(const View&v){','void ladder(const View&v,bool numbered){')
swap('if(std::abs(deg0-v.pitch)>45)continue;double span=deg0==0?12:7,gap=2;','if((!numbered&&deg0!=0)||std::abs(deg0-v.pitch)>45)continue;double span=deg0==0?12:7,gap=2;')
start=s.index(' mainWow=(arr(',s.index('int draw('));end=s.index(' View v{};',start)
s=s[:start]+''' double wheel=heightFt/ft;double gs=groundVelocity*kt;
 bool useRadar=radarValid&&radarHeightFt<5000;
'''+s[end:]
start=s.index(' if(panel){v.heading=heading;');end=s.index(' XPLMSetGraphicsState',start)
s=s[:start]+''' const View camera=v;
 v.heading=heading;v.pitch=pitch;v.roll=roll;v.fx=960/(hudRight-hudLeft);v.fy=960/(hudTop-hudBottom);v.center={480-hudLeft*v.fx,100+hudTop*v.fy};
 // Both views use the same aircraft-fixed optical layout. The full-screen
 // renderer projects the completed body-ray segments into the current camera.
 auto cameraPoint=[&](Point p){
  double x=(p.x-v.center.x)/v.fx,y=-(p.y-v.center.y)/v.fy;
  double a=x*std::cos(rad(roll))+y*std::sin(rad(roll));
  double b=-x*std::sin(rad(roll))+y*std::cos(rad(roll));
  double f=std::cos(rad(pitch))-b*std::sin(rad(pitch));
  double u=std::sin(rad(pitch))+b*std::cos(rad(pitch));
  return camera.project(heading+deg(std::atan2(a,f)),deg(std::atan2(u,std::hypot(a,f))));
 };
 const Point boresight=v.center,fixed{v.center.x,v.center.y+v.fy*std::tan(rad(5))};
 double track=gs>2?groundTrack:heading,gamma=gs>2?deg(std::atan2(verticalVelocity,groundVelocity)):0;
 auto vv=constrain(v.project(track,gamma),710,150,1210,825);vvLimited=vv.limited?1:0;
 double now=val("sim/time/total_running_time_sec"),blend=display.fade/5;
 Point flight{horizontalCage?fixed.x:fixed.x+(vv.p.x-fixed.x)*blend,fixed.y+(vv.p.y-fixed.y)*blend};
 double commandTrack=r.heading+std::clamp(-cross*.012,-20.,20.);
 auto guide=constrain(v.project(commandTrack,commandGamma),698,138,1222,837);guidanceLimited=guide.limited?1:0;
'''+s[end:]
start=s.index(' if(level==0)runwaySymbol');end=s.index(' // Render the under-stroke',start)
s=s[:start]+''' if(!display.main&&level==0)runwaySymbol(v,r,along,cross,alt);
 if(display.horizonVisible(level))ladder(v,display.attitudeVisible(level));
 if(!display.main&&level<2)tapes(equivalent,heightFt,(wheel-commandHeight)*ft);
 segmentGroup=1;
 line({boresight.x-11,boresight.y},{boresight.x+11,boresight.y});line({boresight.x,boresight.y-11},{boresight.x,boresight.y+11});
 if(level<3){
 if(!display.main){
  if(display.guidanceVisible()){
   if(display.phase>=HudPhase::Prfnl){
    if(heightFt>2000){auto cue=v.project(track,-20);triangles({flight.x,cue.p.y},76,roll);}
    if(heightFt<=3500){
     // Open-loop altitude profile, separate from error-correcting guidance.
     double reference=-20;
     if(heightFt>2000)reference-=12*(heightFt-2000)/1500;
     else{
      double lo=LandingPath::circleStart,hi=LandingPath::innerAim;
      for(int k=0;k<32;++k){double mid=(lo+hi)*.5;if(LandingPath::at(mid).height>heightFt/ft)lo=mid;else hi=mid;}
      reference=deg(std::atan(LandingPath::at((lo+hi)*.5).slope));
     }
     auto cue=v.project(track,reference);triangles({flight.x,cue.p.y},76,roll);
    }
   }
   if(!guide.limited||std::fmod(now,1)<.5)poly({{guide.p.x,guide.p.y-15},{guide.p.x+15,guide.p.y},{guide.p.x,guide.p.y+15},{guide.p.x-15,guide.p.y}});
  }
  if(blend<1||horizontalCage)poly({{flight.x-11,flight.y-11},{flight.x+11,flight.y-11},{flight.x+11,flight.y+11},{flight.x-11,flight.y+11}});else circle(flight,11);
  line({flight.x-44,flight.y},{flight.x-11,flight.y});line({flight.x+11,flight.y},{flight.x+44,flight.y});line({flight.x,flight.y-28},{flight.x,flight.y-11});
  if(vv.limited&&blend>=1&&!horizontalCage){line({flight.x-7,flight.y-7},{flight.x+7,flight.y+7});line({flight.x-7,flight.y+7},{flight.x+7,flight.y-7});}
 }
 if(level==2||display.main||heightFt<=1000){
  Point numbers=display.main?boresight:flight;
  text(numbers.x-73,numbers.y-62,(display.nose?"G ":"")+std::to_string(indicatedSpeed(display.nose?gs:equivalent)),30,2);
 }
 if(!display.main)text(flight.x+73,flight.y-62,std::to_string(digitalHeight(useRadar?radarHeightFt:heightFt))+(useRadar?" R":""),30);
 double nz=val("sim/flightmodel/forces/g_nrml",1);
 if(display.nzVisible()&&(nz<=2||std::fmod(now,1)<.5))text(flight.x-74,flight.y+32,num(nz,1)+"G",22,2);
 if(!display.main){
  text(690,912,phaseLabel(display.phase),26);text(690,970,display.automatic?"AUTO":"CSS",20);
  if(display.gearCue&&(display.gearCue!=1||std::fmod(now,1)<.5))text(690,390,display.gearCue==1?"GEAR":display.gearCue==2?"GR":"GR-DN",22);
 }
 double sx=1000,sy=923,len=200;line({sx,sy},{sx+len,sy});
 for(int i=0;i<5;++i){double tick=(i==0||i==4)?13:5;line({sx+i*len/4,sy-tick},{sx+i*len/4,sy+tick});}
 double actual=std::clamp(val("sim/flightmodel2/controls/speedbrake_ratio",val("sim/cockpit2/controls/speedbrake_ratio")),0.,1.);
 double px=sx+len*actual,cx=sx+len*commandSpeedbrake;
 if(std::abs(actual-commandSpeedbrake)<=20/98.6||std::fmod(now,1)<.5)poly({{px,sy-3},{px-9,sy-21},{px+9,sy-21}});
 line({cx,sy+3},{cx,sy+27});line({cx,sy+3},{cx-9,sy+15});line({cx,sy+3},{cx+9,sy+15});
 if(display.main){
  const double x=1320,top=530,length=220;
  line({x,top},{x,top+length});for(int k=0;k<5;++k)line({x-6,top+k*length/4},{x+6,top+k*length/4});
  double y=top+length*(1-std::clamp(display.deceleration/.4,0.,1.));
  poly({{x-3,y},{x-21,y-9},{x-21,y+9}});
  double c=top+length*(1-std::clamp(display.requiredDeceleration/.4,0.,1.));
  line({x+3,c},{x+27,c});line({x+3,c},{x+15,c-9});line({x+3,c},{x+15,c+9});
 }
 }
'''+s[end:]
swap('if(!panel)glScissor(vp[0]+int((.5-480/logicalW)*vp[2]),vp[1]+int((1080-bottom)/1080*vp[3]),int(960/logicalW*vp[2]),int((bottom-top)/1080*vp[3]));','if(!panel)glScissor(vp[0],vp[1],vp[2],vp[3]);(void)top;(void)bottom;')
swap('if(panel&&!clipSegment(s,482,102,1438,group==0?835:1005))continue;strokeVertices','if(!clipSegment(s,482,102,1438,group==0?835:1005))continue;if(!panel){auto a=cameraPoint(s.a),b=cameraPoint(s.b);if(a.limited||b.limited)continue;s={a.p,b.p};}strokeVertices')
swap('mode=std::clamp(mode,-1,2);','mode=std::clamp(mode,-1,3);horizontalCage=horizontalCage?1:0;')
swap('if(c==commands[2])mode=mode==2?-1:mode+1;','if(c==commands[2]){if(display.main)display.groundDeclutter=(display.groundDeclutter+1)%3;else mode=mode<0?0:(mode+1)%4;}if(c==commands[6]){mode=-1;display.groundDeclutter=0;}if(c==commands[7])horizontalCage=!horizontalCage;')
swap('if(i>=0&&i<6)','if(i>=0&&i<8)')
start=s.index('float loop(');end=s.index('\nvoid cleanup()',start)
s=s[:start]+'''void updateDisplay(){
 if(!match||!pluginEnabled||runways.empty())return;
 auto&r=runways[std::clamp(runwayIndex,0,int(runways.size())-1)];
 radarHeightFt=float(std::max(0.,val("sim/flightmodel/position/elevation")-radarGround+mainWheelOffset(val("sim/flightmodel/position/theta"),val("sim/flightmodel/position/phi")))*ft);
 HudInput i;i.time=val("sim/time/total_flight_time_sec");i.heightFt=heightFt;i.eas=equivalent;i.groundspeed=groundVelocity;
 i.headingError=wrap(groundTrack-r.heading);i.crossFt=crossOut*ft;
 i.pathErrorFt=heightFt-LandingPath::at(alongOut).height*ft;
 i.gammaError=deg(std::atan2(verticalVelocity,std::max(1.f,groundVelocity)))+20;
 i.bank=val("sim/flightmodel/position/phi");i.stopDistance=r.length-r.displaced-alongOut-1000/ft;
 for(int k=0;k<3;++k)i.gear[k]=arr("sim/flightmodel2/gear/deploy_ratio",k);
 i.main=mainWow;i.nose=noseWow;i.finalFlare=guidance.finalFlare;i.replay=val("sim/time/is_in_replay")!=0;
 i.automatic=val("sim/cockpit2/autopilot/servos_on")!=0&&val("sim/cockpit/autopilot/autopilot_mode")==2;
 i.runwayVisible=val("sim/weather/visibility_reported_m",50000)>std::hypot(alongOut,crossOut);
 display.update(i);level=display.level(mode,heightFt,i.runwayVisible);
 displayPhase=int(display.phase);displayMain=display.main;displayNose=display.nose;gearCue=display.gearCue;controlAuto=display.automatic;
 vectorBlend=float(display.fade/5);displayAltitude=float(digitalHeight(radarValid&&radarHeightFt<5000?radarHeightFt:heightFt));
 deceleration=float(display.deceleration);decelerationCommand=float(display.requiredDeceleration);
 displayFlags=level==3?1:1|(!display.main?2:0)|(display.guidanceVisible()?4:0)|(display.horizonVisible(level)?8:0)|
  (display.attitudeVisible(level)?16:0)|(!display.main&&level<2?32:0)|(display.main?64:0)|
  (display.nzVisible()?128:0)|(!display.main&&level==0?256:0);
}
float loop(float,float,int,void*){checkMatch();updateKinematics();updateGuidance();updateChute();static double nextRadar=0;double clock=val("sim/time/total_running_time_sec");if(clock>=nextRadar){radar();nextRadar=clock+.2;}updateDisplay();if(ownView&&(!match||!enabled||!pluginEnabled||int(val("sim/graphics/view/view_type"))!=1024))restoreView();return -1.f;}'''+s[end:]
swap(' rf("fsim_hud/velocity_vector_blend",&vectorBlend);',''' rf("fsim_hud/velocity_vector_blend",&vectorBlend);
 ri("fsim_hud/att_ref_caged",&horizontalCage,true);ri("fsim_hud/display_phase",&displayPhase);ri("fsim_hud/display_main_wow",&displayMain);ri("fsim_hud/display_nose_wow",&displayNose);
 ri("fsim_hud/gear_cue",&gearCue);ri("fsim_hud/display_flags",&displayFlags);ri("fsim_hud/control_auto",&controlAuto);
 rf("fsim_hud/display_altitude_ft",&displayAltitude);rf("fsim_hud/deceleration_g",&deceleration);rf("fsim_hud/deceleration_command_g",&decelerationCommand);''')
swap('"fsim_hud/fullscreen"};','"fsim_hud/fullscreen","fsim_hud/declutter_auto","fsim_hud/att_ref"};')
swap('"Declutter Auto / 0 / 1 / 2"','"Cycle manual HUD declutter"')
swap('"Show full-screen shuttle HUD"};','"Show full-screen shuttle HUD","Use F-SIM automatic declutter","Toggle ATT REF horizontal cage"};')
swap('for(int i=0;i<6;++i){auto c=i==4','for(int i=0;i<8;++i){auto c=i==4')
swap('for(int i:{0,1,2,3,5})','for(int i:{0,1,2,3,5,6,7})')
swap('[ShuttleHUD] v127 registered; native HUD, earth-relative velocity, shared guidance and staged drag chute. No position or force overrides.','[ShuttleHUD] v137 registered; manual-based HUD presentation, native optics, shared guidance and staged drag chute. No position or force overrides.')
swap('refs.clear();fade=0;lastTime=-1;guidance.reset();','refs.clear();display.reset();guidance.reset();')
p.write_text(s)
b=S/'build.ps1';t=b.read_text();t=t.replace("$ErrorActionPreference = 'Stop'","param([string]$OutputPath = '')\n$ErrorActionPreference = 'Stop'")
t=t.replace("$outDir = Join-Path $taskRoot 'Output\\shuttle-hud-20260909'","$outDir = if ($OutputPath) { $OutputPath } else { Join-Path $taskRoot 'Output\\shuttle-hud-20260909' }\nNew-Item -ItemType Directory -Path $outDir -Force | Out-Null")
where='& $compiler c++ -target'
t=t.replace(where,"& $compiler c++ -std=c++17 -O2 -Wall -Wextra -Werror (Join-Path $PSScriptRoot 'test_presentation.cpp') -o (Join-Path $outDir 'test_presentation.exe')\nif ($LASTEXITCODE) { throw 'HUD presentation test build failed' }\n& (Join-Path $outDir 'test_presentation.exe')\nif ($LASTEXITCODE) { throw 'HUD presentation test failed' }\n"+where)
t=t.replace("(Join-Path $PSScriptRoot 'hud_math.hpp'),","(Join-Path $PSScriptRoot 'hud_math.hpp'), (Join-Path $PSScriptRoot 'hud_presentation.hpp'),")
b.write_text(t)
print('Presentation renderer integrated, version 137; landing guidance unchanged.')
