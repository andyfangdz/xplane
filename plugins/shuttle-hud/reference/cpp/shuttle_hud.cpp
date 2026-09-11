#include "XPLMPlugin.h"
#include "XPLMDataAccess.h"
#include "XPLMDisplay.h"
#include "XPLMGraphics.h"
#include "XPLMUtilities.h"
#include "XPLMPlanes.h"
#include "XPLMScenery.h"
#include "XPLMProcessing.h"
#include "XPLMMenus.h"
#include "hud_math.hpp"
#include "landing_guidance.hpp"
#include "hud_presentation.hpp"
#include <windows.h>
#include <GL/gl.h>
#include <array>
#include <cstdio>
#include <cstring>
#include <filesystem>
#include <fstream>
#include <map>
#include <sstream>
#include <vector>

namespace {
using namespace shud;
std::map<std::string,XPLMDataRef> refs;
std::vector<XPLMDataRef> accessors;
std::vector<XPLMCommandRef> commands;
std::filesystem::path folder;
std::string expected;
int enabled=1,pluginEnabled=0,match=0,active=0,frames=0,mode=-1,level=0,runwayIndex=0,version=142;
bool windowRegistered=false,panelRegistered=false,loopRegistered=false;
int renderer=2,guidancePhase=1;
float commandHeight=0,commandGear=0,alongOut=0,crossOut=0;
LandingGuidance guidance;
int landingSystems=1,chuteActive=0;
float chuteRatio=1;
double chuteBase=0,chuteStart=-1;
float runwayGround=0;
float verticalVelocity=0,groundVelocity=0,groundTrack=0;
bool chuteOwned=false;
double hudLeft=-.2320574215,hudRight=.2320574215,hudTop=.1583844403,hudBottom=-.3057304027;
double panelLeft=2112,panelBottom=128,panelSize=640;
int panelFrames=0,cockpitActive=0;
int vvLimited=0,guidanceLimited=0,mainWow=0,noseWow=0;
float radarHeightFt=0,heightFt=0,equivalent=0,commandGamma=0,commandSpeedbrake=0,brightness=1,vectorBlend=0;
double radarGround=0;
HudPresentation display;
int horizontalCage=0,displayPhase=0,displayMain=0,displayNose=0,gearCue=0,displayFlags=0,controlAuto=0;
float displayAltitude=0,deceleration=0,decelerationCommand=0;
bool radarValid=false;
bool ownView=false,onGlass=false;
float oldShift=0,oldFov=65;
XPLMMenuID menu=nullptr;
int menuIndex=-1;
XPLMProbeRef probe=nullptr;
struct Runway {std::string name;double lat,lon,endLat,endLon,width,displaced,elev,heading,length,un,ue;};
std::vector<Runway> runways;
XPLMDataRef ref(const char*n){auto&r=refs[n];if(!r)r=XPLMFindDataRef(n);return r;}
double val(const char*n,double fallback=0){auto r=ref(n);if(!r)return fallback;auto t=XPLMGetDataRefTypes(r);if(t&xplmType_Double)return XPLMGetDatad(r);if(t&xplmType_Float)return XPLMGetDataf(r);if(t&xplmType_Int)return XPLMGetDatai(r);return fallback;}
double arr(const char*n,int i){auto r=ref(n);if(!r)return 0;float f=0;int v=0;if(XPLMGetDataRefTypes(r)&xplmType_IntArray){XPLMGetDatavi(r,&v,i,1);return v;}XPLMGetDatavf(r,&f,i,1);return f;}
void set(const char*n,float v){auto r=ref(n);if(r&&XPLMCanWriteDataRef(r))XPLMSetDataf(r,v);}
void checkMatch(){char name[256]={},path[2048]={};XPLMGetNthAircraftModel(0,name,path);match=matches(std::filesystem::path(path).lexically_normal().generic_string(),expected)?1:0;}
void restoreView(){if(!ownView)return;set("sim/graphics/view/field_of_view_vertical_ratio",oldShift);set("sim/graphics/view/field_of_view_deg",oldFov);ownView=false;}
void chooseRunway();void terrain();void updateKinematics();
void hudView(){checkMatch();if(!match||!pluginEnabled||!enabled)return;chooseRunway();terrain();if(!ownView){oldShift=float(val("sim/graphics/view/field_of_view_vertical_ratio"));oldFov=float(val("sim/graphics/view/field_of_view_deg",65));ownView=true;}
 XPLMCommandOnce(XPLMFindCommand("sim/view/forward_with_nothing"));set("sim/graphics/view/field_of_view_deg",65);set("sim/graphics/view/field_of_view_vertical_ratio",-.5f);
}
void cockpitView(){checkMatch();if(!match||!pluginEnabled||!enabled)return;restoreView();chooseRunway();terrain();XPLMCommandOnce(XPLMFindCommand("sim/view/3d_cockpit_cmnd_look"));}
void loadRunways(){std::ifstream in(folder/"runways.csv");std::string line;while(std::getline(in,line)){std::replace(line.begin(),line.end(),',',' ');std::istringstream s(line);Runway r{};if(!(s>>r.name>>r.lat>>r.lon>>r.endLat>>r.endLon>>r.width>>r.displaced>>r.elev))continue;
 double n=(r.endLat-r.lat)*111120,e=(r.endLon-r.lon)*111120*std::cos(rad((r.lat+r.endLat)*.5));r.length=std::hypot(n,e);if(r.length<1000)continue;r.un=n/r.length;r.ue=e/r.length;r.heading=std::fmod(deg(std::atan2(e,n))+360,360);runways.push_back(r);}}
void chooseRunway(){if(runways.empty())return;updateKinematics();double best=1e100,track=groundVelocity>5?groundTrack:val("sim/flightmodel/position/psi");for(std::size_t i=0;i<runways.size();++i){auto&r=runways[i];double n=(val("sim/flightmodel/position/latitude")-r.lat)*111120,e=(val("sim/flightmodel/position/longitude")-r.lon)*111120*std::cos(rad(r.lat));double score=std::hypot(n,e)+100*std::abs(wrap(track-r.heading));if(score<best){best=score;runwayIndex=int(i);}}
}
void updateKinematics(){
 if(!match)return;
 // Local OpenGL Y is vertical at the scenery origin, not at the aircraft.
 // Obtain the aircraft's earth-relative basis through the SDK transforms.
 using V=std::array<double,3>;V p{},u{},n{},e{},v{{val("sim/flightmodel/position/local_vx"),val("sim/flightmodel/position/local_vy"),val("sim/flightmodel/position/local_vz")}};
 double lat=val("sim/flightmodel/position/latitude"),lon=val("sim/flightmodel/position/longitude"),alt=val("sim/flightmodel/position/elevation");
 XPLMWorldToLocal(lat,lon,alt,&p[0],&p[1],&p[2]);
 XPLMWorldToLocal(lat,lon,alt+100,&u[0],&u[1],&u[2]);
 XPLMWorldToLocal(lat+.001,lon,alt,&n[0],&n[1],&n[2]);
 auto dot=[](const V&a,const V&b){return a[0]*b[0]+a[1]*b[1]+a[2]*b[2];};
 auto norm=[&](V&a){double d=std::sqrt(dot(a,a));if(d<1e-6)return false;for(auto&x:a)x/=d;return true;};
 for(int k=0;k<3;++k){u[k]-=p[k];n[k]-=p[k];}
 if(!norm(u))return;double verticalPart=dot(n,u);for(int k=0;k<3;++k)n[k]-=verticalPart*u[k];if(!norm(n))return;
 e={n[1]*u[2]-n[2]*u[1],n[2]*u[0]-n[0]*u[2],n[0]*u[1]-n[1]*u[0]};
 double north=dot(v,n),east=dot(v,e);verticalVelocity=float(dot(v,u));groundVelocity=float(std::hypot(north,east));groundTrack=float(std::fmod(deg(std::atan2(east,north))+360,360));
}
void terrain(){if(runways.empty()||!probe)return;auto&r=runways[std::clamp(runwayIndex,0,int(runways.size())-1)];double x,y,z,lat,lon,alt;
 // Reference the nominal touchdown zone, including the displaced threshold.
 // The physical runway end can be at a different height on a sloping mesh.
 double distance=r.displaced+2500/ft;
 XPLMWorldToLocal(r.lat+distance*r.un/111120,r.lon+distance*r.ue/(111120*std::cos(rad(r.lat))),r.elev+2000,&x,&y,&z);XPLMProbeInfo_t info{};info.structSize=sizeof(info);if(XPLMProbeTerrainXYZ(probe,float(x),float(y),float(z),&info)==xplm_ProbeHitTerrain){XPLMLocalToWorld(info.locationX,info.locationY,info.locationZ,&lat,&lon,&alt);r.elev=alt;runwayGround=alt;char msg[160];std::snprintf(msg,sizeof(msg),"[ShuttleHUD] %s touchdown-zone datum %.3f m MSL\n",r.name.c_str(),alt);XPLMDebugString(msg);}}
void radar(){radarValid=false;if(!match||!probe)return;double x,y,z,lat,lon,alt;XPLMWorldToLocal(val("sim/flightmodel/position/latitude"),val("sim/flightmodel/position/longitude"),val("sim/flightmodel/position/elevation")+1000,&x,&y,&z);XPLMProbeInfo_t info{};info.structSize=sizeof(info);if(XPLMProbeTerrainXYZ(probe,float(x),float(y),float(z),&info)==xplm_ProbeHitTerrain){XPLMLocalToWorld(info.locationX,info.locationY,info.locationZ,&lat,&lon,&alt);radarGround=alt;radarValid=true;}}
bool loadOptics(){
 std::ifstream in(folder/"hud-optics.txt");std::map<std::string,double> values;std::string line;
 while(std::getline(in,line)){std::istringstream s(line);std::string k;double v;if(s>>k>>v)values[k]=v;}
 for(const char*k:{"left","right","top","bottom","panel_left","panel_bottom","panel_size"})if(!values.count(k)||!std::isfinite(values[k]))return false;
 hudLeft=values["left"];hudRight=values["right"];hudTop=values["top"];hudBottom=values["bottom"];
 panelLeft=values["panel_left"];panelBottom=values["panel_bottom"];panelSize=values["panel_size"];
 return hudLeft<0&&hudRight>0&&hudBottom<hudTop&&panelLeft>=2048&&panelBottom>=0&&panelSize>=64&&panelLeft+panelSize<=4096&&panelBottom+panelSize<=2048;
}
double mainWheelOffset(double pitch,double roll){double y=arr("sim/aircraft/parts/acf_gear_ynodef",1)-arr("sim/aircraft/parts/acf_gear_leglen",1)-arr("sim/flightmodel2/gear/tire_radius_mtrs",1),z=arr("sim/aircraft/parts/acf_gear_znodef",1);return y*std::cos(rad(pitch))*std::cos(rad(roll))-z*std::sin(rad(pitch));}
void updateGuidance(){if(!match||runways.empty())return;auto&r=runways[std::clamp(runwayIndex,0,int(runways.size())-1)];double n=(val("sim/flightmodel/position/latitude")-r.lat)*111120,e=(val("sim/flightmodel/position/longitude")-r.lon)*111120*std::cos(rad(r.lat));alongOut=float(n*r.un+e*r.ue-r.displaced);crossOut=float(e*r.un-n*r.ue);
 GuidanceInput i;i.time=val("sim/time/total_flight_time_sec");i.along=alongOut;i.height=std::max(0.,val("sim/flightmodel/position/elevation")-r.elev+mainWheelOffset(val("sim/flightmodel/position/theta"),val("sim/flightmodel/position/phi")));i.groundspeed=groundVelocity;i.vy=verticalVelocity;i.eas=eas(val("sim/flightmodel/position/true_airspeed"),val("sim/weather/rho",1.225));i.massLb=val("sim/flightmodel/weight/m_total")*2.204622622;i.mainWow=arr("sim/flightmodel2/gear/on_ground",1)||arr("sim/flightmodel2/gear/on_ground",2);
 mainWow=i.mainWow?1:0;noseWow=arr("sim/flightmodel2/gear/on_ground",0)?1:0;if(mainWow)i.height=0;heightFt=float(i.height*ft);equivalent=float(i.eas);
 if(display.lastTime>=0&&(std::abs(i.height*ft-display.lastHeight)>500||std::abs(alongOut-display.lastAlong)>500)){guidance.reset();display.reset();}
 guidance.update(i);commandGamma=float(guidance.gamma);commandSpeedbrake=float(guidance.speedbrake);commandHeight=float(guidance.targetHeight);commandGear=float(guidance.gear);guidancePhase=guidance.phase;
}
void releaseChute(){if(chuteOwned&&match)set("sim/aircraft/specialcontrols/acf_chute_area",float(chuteBase));chuteOwned=false;chuteActive=0;chuteRatio=1;}
void updateChute(){
 if(!match||!pluginEnabled||!landingSystems||val("sim/time/is_in_replay")||val("sim/time/paused")){releaseChute();return;}
 if(!val("sim/cockpit/switches/parachute_on")){releaseChute();chuteStart=-1;return;}
 double now=val("sim/time/total_flight_time_sec");
 if(chuteStart<0){if(!mainWow)return;chuteStart=now;}
 if(now<chuteStart){releaseChute();chuteStart=-1;return;}
 if(!chuteOwned){chuteBase=val("sim/aircraft/specialcontrols/acf_chute_area");if(chuteBase<=0)return;chuteOwned=true;}
 chuteRatio=float(chuteAreaRatio(now-chuteStart));chuteActive=chuteRatio<1;
 set("sim/aircraft/specialcontrols/acf_chute_area",float(chuteBase*chuteRatio));
}
// Thin quad strokes remain reliable with X-Plane's Vulkan/OpenGL bridge.
struct Segment{Point a,b;};
bool clipSegment(Segment& s,double l,double t,double r,double b){
 double lo=0,hi=1,dx=s.b.x-s.a.x,dy=s.b.y-s.a.y;
 auto edge=[&](double p,double q){if(std::abs(p)<1e-12)return q>=0;double u=q/p;if(p<0)lo=std::max(lo,u);else hi=std::min(hi,u);return lo<=hi;};
 if(!edge(-dx,s.a.x-l)||!edge(dx,r-s.a.x)||!edge(-dy,s.a.y-t)||!edge(dy,b-s.a.y))return false;
 Point a=s.a;s.a={a.x+lo*dx,a.y+lo*dy};s.b={a.x+hi*dx,a.y+hi*dy};return true;
}
std::array<std::vector<Segment>,2> segments;
int segmentGroup=0;
void strokeVertices(Point a,Point b,double width){double l=std::hypot(b.x-a.x,b.y-a.y);if(l<1e-9)return;double x=-(b.y-a.y)*width/(2*l),y=(b.x-a.x)*width/(2*l);glVertex2d(a.x+x,a.y+y);glVertex2d(b.x+x,b.y+y);glVertex2d(b.x-x,b.y-y);glVertex2d(a.x-x,a.y-y);}
void line(Point a,Point b){if(std::isfinite(a.x)&&std::isfinite(a.y)&&std::isfinite(b.x)&&std::isfinite(b.y)&&segments[segmentGroup].size()<20000)segments[segmentGroup].push_back({a,b});}
void poly(std::initializer_list<Point> p){if(!p.size())return;auto prev=*(p.end()-1);for(auto q:p){line(prev,q);prev=q;}}
void circle(Point p,double r){Point prev{p.x+r,p.y};for(int i=1;i<=32;++i){double a=2*pi*i/32;Point q{p.x+r*std::cos(a),p.y+r*std::sin(a)};line(prev,q);prev=q;}}
void dash(Point a,Point b){for(int i=0;i<5;++i){double u=i/5.,v=(i+.6)/5.;line({a.x+(b.x-a.x)*u,a.y+(b.y-a.y)*u},{a.x+(b.x-a.x)*v,a.y+(b.y-a.y)*v});}}
// Original vector lettering: no F-SIM artwork or proprietary font is embedded.
const std::map<char,std::string> glyphs={
 {'0',"10 40 50 51 57 48 18 08 01 10"},{'1',"11 30 38|18 48"},{'2',"01 10 40 51 53 05 08 58"},{'3',"00 50 50 53 34 53 57 48 08"},
 {'4',"50 04 05 55|50 58"},{'5',"50 00 04 44 55 57 48 08"},{'6',"50 10 01 07 18 48 57 55 44 04"},{'7',"00 50 18"},{'8',"10 40 51 53 44 14 03 01 10|14 05 07 18 48 57 55 44"},{'9',"54 14 03 01 10 40 51 57 48 08"},
 {'A',"08 01 10 40 51 58|04 54"},{'B',"00 08 48 57 55 44 04|00 40 51 53 44"},{'C',"50 10 01 07 18 58"},{'D',"00 08 38 57 51 30 00"},{'E',"50 00 08 58|04 44"},{'F',"50 00 08|04 44"},{'G',"50 10 01 07 18 58 54 34"},{'H',"00 08|50 58|04 54"},{'I',"00 50|20 28|08 58"},{'J',"50 57 48 18 07"},{'K',"00 08|50 04 58"},{'L',"00 08 58"},{'M',"08 00 24 50 58"},{'N',"08 00 58 50"},{'O',"10 40 51 57 48 18 07 01 10"},{'P',"08 00 40 51 53 44 04"},{'Q',"10 40 51 56 48 18 07 01 10|35 58"},{'R',"08 00 40 51 53 44 04|24 58"},{'S',"50 10 01 03 14 44 55 57 48 08"},{'T',"00 50|20 28"},{'U',"00 07 18 48 57 50"},{'V',"00 28 50"},{'W',"00 18 24 48 50"},{'X',"00 58|50 08"},{'Y',"00 24 50|24 28"},{'Z',"00 50 08 58"},{'-',"04 54"},{'+',"04 54|20 28"},{'.',"28 28"},{'/',"08 50"}
};
void text(double x,double y,const std::string&s,double size=24,int align=0,double angle=0,Point pivot={}){if(onGlass)size*=1.4;double adv=size*.78;x-=s.size()*adv*(align*.5);for(char c:s){auto it=glyphs.find(c);if(c=='.'){circle(rotate({x+size*.2,y+size},pivot,angle),.8);x+=adv;continue;}if(it!=glyphs.end()){bool have=false;Point prev{};std::istringstream st(it->second);std::string token;while(st>>token){for(std::size_t j=0;j<token.size();){if(token[j]=='|'){have=false;++j;continue;}if(j+1>=token.size())break;Point p=rotate({x+(token[j]-'0')*size/8,y+(token[j+1]-'0')*size/8},pivot,angle);if(have)line(prev,p);prev=p;have=true;j+=2;}}}x+=adv;}}
std::string num(double v,int places=0){char b[64];std::snprintf(b,sizeof(b),"%.*f",places,v);return b;}
struct View {double fx,fy;Point center;double heading,pitch,roll;Projected project(double az,double el)const{return shud::project(az,el,heading,pitch,roll,fx,fy,center);}};
void triangles(Point c,double half,double roll){for(int sign:{-1,1}){Point tip{c.x+sign*half,c.y};poly({rotate(tip,c,-roll),rotate({tip.x+sign*16,tip.y-9},c,-roll),rotate({tip.x+sign*16,tip.y+9},c,-roll)});}}
void runwaySymbol(const View&v,const Runway&r,double along,double cross,double altitude){
 auto p=[&](double a,double c){double dn=(a-along)*r.un-(c-cross)*r.ue,de=(a-along)*r.ue+(c-cross)*r.un;return v.project(deg(std::atan2(de,dn)),deg(std::atan2(r.elev-altitude,std::hypot(dn,de))));};
 // The real symbolic runway is 300 ft wide, independent of pavement width.
 const double half=150/ft,length=std::min(15000/ft,r.length-r.displaced);
 const std::array<Point,4> corners{{{0,-half},{0,half},{length,half},{length,-half}}};
 for(int k=0;k<4;++k){
  Point start=corners[k],end=corners[(k+1)%4];auto a=p(start.x,start.y),b=p(end.x,end.y);
  if(a.limited&&b.limited)continue;
  // Clip a crossing edge at the forward projection plane before 2D FOV clipping.
  if(a.limited||b.limited){
   Point front=a.limited?end:start,back=a.limited?start:end;
   for(int n=0;n<32;++n){Point mid{(front.x+back.x)*.5,(front.y+back.y)*.5};if(p(mid.x,mid.y).limited)back=mid;else front=mid;}
   if(a.limited)a=p(front.x,front.y);else b=p(front.x,front.y);
  }
  line(a.p,b.p);
 }
 auto inner=p(LandingPath::innerAim,0),outer=p(LandingPath::outerAim,0);
 if(!inner.limited)circle(inner.p,7);if(!outer.limited)circle(outer.p,7);
 if(!inner.limited&&!outer.limited)line(inner.p,outer.p);
}
void tapes(double speed,double altitude,double error){double cy=100+hudTop*960/(hudTop-hudBottom)+std::tan(rad(5))*960/(hudTop-hudBottom),l=590,r=1330;
 for(int n=int(std::floor((speed-19)/5))*5;n<speed+19;n+=5){double y=cy+(n-speed)*18;if(n<0)continue;line({l-9,y},{l+(n%10?2:9),y});if(n%10==0)text(l+20,y-12,num(n),26);}
 poly({{l-10,cy-5},{l+10,cy-5},{l+10,cy+5},{l-10,cy+5}});
 double step=altitudeStep(altitude),spacing=150;int low=int(std::ceil((altitude-1.65*step)/step));for(int n=low;n<=std::floor((altitude+1.65*step)/step);++n){double a=n*step,y=cy+(altitude-a)*spacing/step;if(a<0)continue;line({r-9,y},{r+9,y});text(r-22,y-12,altitude>1000?num(a/1000)+"K":num(a),26,2);}
 poly({{r-10,cy-5},{r+10,cy-5},{r+10,cy+5},{r-10,cy+5}});double gsi=cy+std::clamp(error/std::max(50.,altitude*.06),-3.,3.)*55;poly({{r+34,gsi},{r+51,gsi-9},{r+51,gsi+9}});
}
void ladder(const View&v,bool numbered){for(int deg0=-85;deg0<=85;deg0+=5){if((!numbered&&deg0!=0)||std::abs(deg0-v.pitch)>45)continue;double span=deg0==0?8:5,gap=2;
 auto a=v.project(v.heading-span,deg0),b=v.project(v.heading-gap,deg0),c=v.project(v.heading+gap,deg0),d=v.project(v.heading+span,deg0);if(a.limited||b.limited||c.limited||d.limited)continue;
 if(deg0<0){dash(a.p,b.p);dash(c.p,d.p);}else{line(a.p,b.p);line(c.p,d.p);}if(!deg0)continue;
 auto e=v.project(v.heading-span,deg0+(deg0<0?.8:-.8)),f=v.project(v.heading+span,deg0+(deg0<0?.8:-.8));line(a.p,e.p);line(d.p,f.p);
 text(d.p.x+10,d.p.y+(deg0<0?-27:8),num(std::abs(deg0)),22,0,-v.roll,d.p);}}
int draw(XPLMDrawingPhase phase,int,void*){
 const bool panel=phase==xplm_Phase_Gauges;onGlass=panel;
 const int viewType=int(val("sim/graphics/view/view_type"));
 if(panel){
  if(!match)return 1;
  const int renderType=int(val("sim/graphics/view/panel_render_type"));
  if(renderType==0)return 1;
  // Reserve this region in both native panel layers; only the emissive pass
  // supplies the image for X-Plane's collimating compositor.
  XPLMSetGraphicsState(0,0,0,0,0,0,0);glPushAttrib(GL_ENABLE_BIT);glDisable(GL_SCISSOR_TEST);glDisable(GL_STENCIL_TEST);glDisable(GL_CULL_FACE);for(int i=0;i<6;++i)glDisable(GL_CLIP_PLANE0+i);glColor4f(0,0,0,0);
  double l=panelLeft,b=panelBottom;
  glBegin(GL_QUADS);glVertex2d(l,b);glVertex2d(l+panelSize,b);glVertex2d(l+panelSize,b+panelSize);glVertex2d(l,b+panelSize);glEnd();glPopAttrib();
  if(renderType!=2)return 1;
  if(!pluginEnabled||!enabled||val("sim/cockpit2/switches/HUD_on")==0||val("sim/cockpit2/electrical/HUD_brightness_ratio")<=0){cockpitActive=0;return 1;}
 }else{
  cockpitActive=pluginEnabled&&enabled&&match&&panelFrames>0&&viewType==1026&&val("sim/cockpit2/switches/HUD_on")!=0&&val("sim/cockpit2/electrical/HUD_brightness_ratio")>0;
  active=cockpitActive;
  if(viewType!=1024||val("sim/graphics/VR/enabled")!=0)return 1;
 }
 if(!pluginEnabled||!enabled||!match||runways.empty()||val("sim/cockpit2/switches/HUD_on")==0||val("sim/cockpit2/electrical/HUD_brightness_ratio")<=0)return 1;
 int w=0,h=0;XPLMGetScreenSize(&w,&h);if(w<=0||h<=0)return 1;double scale=h/1080.,logicalW=w/scale;double shift=(logicalW-1920)/2;
 auto&r=runways[std::clamp(runwayIndex,0,int(runways.size())-1)];double lat=val("sim/flightmodel/position/latitude"),lon=val("sim/flightmodel/position/longitude"),alt=val("sim/flightmodel/position/elevation");
 double n=(lat-r.lat)*111120,e=(lon-r.lon)*111120*std::cos(rad(r.lat)),along=n*r.un+e*r.ue-r.displaced,cross=e*r.un-n*r.ue;
 double pitch=val("sim/flightmodel/position/theta"),roll=val("sim/flightmodel/position/phi"),heading=val("sim/flightmodel/position/psi");
 double wheel=heightFt/ft;double gs=groundVelocity*kt;
 bool useRadar=radarValid&&radarHeightFt<5000;
 View v{};v.heading=val("sim/graphics/view/view_heading",heading);v.pitch=val("sim/graphics/view/view_pitch",pitch);v.roll=val("sim/graphics/view/view_roll",roll);
 float matrix[16]={};auto pr=ref("sim/graphics/view/projection_matrix_3d");if(pr)XPLMGetDatavf(pr,matrix,0,16);
 v.fx=(matrix[0]>.01?matrix[0]:1/std::tan(rad(val("sim/graphics/view/field_of_view_deg",65)/2)))*logicalW/2;v.fy=matrix[5]>.01?matrix[5]*540:v.fx;v.center={logicalW*(.5-.5*matrix[8])-shift,540*(1+matrix[9])};
 // Pixels encode rays in aircraft axes. X-Plane projects these at optical
 // distance and clips them to the real combiner separately for each eye.
 const View camera=v;
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
 XPLMSetGraphicsState(0,0,0,0,1,0,0);glPushAttrib(GL_CURRENT_BIT|GL_SCISSOR_BIT|GL_ENABLE_BIT);glDisable(GL_CULL_FACE);for(int i=0;i<6;++i)glDisable(GL_CLIP_PLANE0+i);glDisable(GL_ALPHA_TEST);glDisable(GL_STENCIL_TEST);GLint oldMode=GL_MODELVIEW;glGetIntegerv(GL_MATRIX_MODE,&oldMode);glMatrixMode(GL_PROJECTION);glPushMatrix();if(!panel){glLoadIdentity();glOrtho(0,w,0,h,-1,1);}glMatrixMode(GL_MODELVIEW);glPushMatrix();
 if(panel){glDisable(GL_SCISSOR_TEST);glTranslated(panelLeft,panelBottom+panelSize,0);glScaled(panelSize/960.,-panelSize/960.,1);glTranslated(-480,-100,0);}
 else{glLoadIdentity();glTranslated((w-1920*scale)/2,h,0);glScaled(scale,-scale,1);}
 for(auto&group:segments)group.clear();segmentGroup=0;
 // Only HUD field is clipped; letterforms retain the same aspect at all resolutions.
 if(!panel)glEnable(GL_SCISSOR_TEST);GLint vp[4]={};glGetIntegerv(GL_VIEWPORT,vp);auto clip=[&](double top,double bottom){if(!panel)glScissor(vp[0],vp[1],vp[2],vp[3]);(void)top;(void)bottom;};clip(100,835);
 if(!display.main&&level==0)runwaySymbol(v,r,along,cross,alt);
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
     if(heightFt>2000)reference=-20;
     else{
      double lo=LandingPath::circleStart,hi=LandingPath::innerAim;
      for(int k=0;k<32;++k){double mid=(lo+hi)*.5;if(LandingPath::at(mid).height>heightFt/ft)lo=mid;else hi=mid;}
      reference=deg(std::atan(LandingPath::at((lo+hi)*.5).slope));
     }
     auto cue=v.project(track,reference);
     if(heightFt>2000)cue.p.y+=(835-cue.p.y)*(heightFt-2000)/1500;
     triangles({flight.x,cue.p.y},76,roll);
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
 // Render the under-stroke first across the whole display, then the luminous
 // stroke. This avoids dark joints in the lettering and uses only four batches.
 // Translucent emission limits scenery occlusion at filtered edges.
 // The matching luminance gain preserves the bright central stroke.
 if(panel)glDisable(GL_BLEND);float alpha=brightness;
 for(int group=0;group<2;++group){clip(100,group==0?835:1005);for(int pass=0;pass<2;++pass){if(panel&&!pass)continue;if(pass)glColor4f(panel?.25f*brightness:.74f,panel?brightness:.84f,panel?.40f*brightness:.64f,alpha);else glColor4f(.025f,.045f,.015f,alpha*.32f);glBegin(GL_QUADS);for(auto s:segments[group]){if(!clipSegment(s,482,102,1438,group==0?835:1005))continue;if(!panel){auto a=cameraPoint(s.a),b=cameraPoint(s.b);if(a.limited||b.limited)continue;s={a.p,b.p};}strokeVertices(s.a,s.b,pass?(panel?4.6:2.1):3.2);}glEnd();}}
 glPopMatrix();glMatrixMode(GL_PROJECTION);glPopMatrix();glMatrixMode(oldMode);glPopAttrib();if(panel){++panelFrames;cockpitActive=viewType==1026;active=cockpitActive;}else active=1;++frames;return 1;
}
int getInt(void*p){return *static_cast<int*>(p);}void putInt(void*p,int v){*static_cast<int*>(p)=v;enabled=enabled?1:0;mode=std::clamp(mode,-1,3);horizontalCage=horizontalCage?1:0;if(!enabled)restoreView();}
float getFloat(void*p){return *static_cast<float*>(p);}void putFloat(void*p,float v){*static_cast<float*>(p)=std::clamp(v,.05f,1.f);}
void ri(const char*n,int*p,bool write=false){accessors.push_back(XPLMRegisterDataAccessor(n,xplmType_Int,write,getInt,write?putInt:nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,p,p));}
void rf(const char*n,float*p,bool write=false){accessors.push_back(XPLMRegisterDataAccessor(n,xplmType_Float,write,nullptr,nullptr,getFloat,write?putFloat:nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,p,p));}
int command(XPLMCommandRef c,XPLMCommandPhase phase,void*){if(phase!=xplm_CommandBegin)return 1;checkMatch();if(!match)return 1;
 if(c==commands[0]||c==commands[4])cockpitView();if(c==commands[5])hudView();if(c==commands[1]){enabled=!enabled;if(!enabled)restoreView();}if(c==commands[2]){if(display.main)display.groundDeclutter=(display.groundDeclutter+1)%3;else mode=mode<0?0:(mode+1)%4;}if(c==commands[6]){mode=-1;display.groundDeclutter=0;}if(c==commands[7])horizontalCage=!horizontalCage;if(c==commands[3]&&!runways.empty()){runwayIndex=(runwayIndex+1)%int(runways.size());terrain();}return c==commands[4]&&enabled?0:1;}
void menuAction(void*,void*item){auto i=reinterpret_cast<std::intptr_t>(item);if(i>=0&&i<8)XPLMCommandOnce(commands[std::size_t(i)]);}
void updateDisplay(){
 if(!match||!pluginEnabled||runways.empty())return;
 auto&r=runways[std::clamp(runwayIndex,0,int(runways.size())-1)];
 radarHeightFt=float(std::max(0.,val("sim/flightmodel/position/elevation")-radarGround+mainWheelOffset(val("sim/flightmodel/position/theta"),val("sim/flightmodel/position/phi")))*ft);
 HudInput i;i.time=val("sim/time/total_flight_time_sec");i.heightFt=heightFt;i.along=alongOut;i.eas=equivalent;i.groundspeed=groundVelocity;
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
float loop(float,float,int,void*){checkMatch();updateKinematics();updateGuidance();updateChute();static double nextRadar=0;double clock=val("sim/time/total_running_time_sec");if(clock>=nextRadar){radar();nextRadar=clock+.2;}updateDisplay();if(ownView&&(!match||!enabled||!pluginEnabled||int(val("sim/graphics/view/view_type"))!=1024))restoreView();return -1.f;}
void cleanup(){releaseChute();pluginEnabled=0;active=cockpitActive=0;restoreView();
 if(windowRegistered){XPLMUnregisterDrawCallback(draw,xplm_Phase_Window,0,nullptr);windowRegistered=false;}
 if(panelRegistered){XPLMUnregisterDrawCallback(draw,xplm_Phase_Gauges,0,nullptr);panelRegistered=false;}
 if(loopRegistered){XPLMUnregisterFlightLoopCallback(loop,nullptr);loopRegistered=false;}
 for(auto c:commands)XPLMUnregisterCommandHandler(c,command,1,nullptr);commands.clear();
 for(auto r:accessors)XPLMUnregisterDataAccessor(r);accessors.clear();
 if(probe){XPLMDestroyProbe(probe);probe=nullptr;}
 if(menu){XPLMDestroyMenu(menu);menu=nullptr;}
 if(menuIndex>=0){XPLMRemoveMenuItem(XPLMFindPluginsMenu(),menuIndex);menuIndex=-1;}
 XPLMDebugString("[ShuttleHUD] cleanup complete; native panel callback released.\n");
}
}
PLUGIN_API int XPluginStart(char*n,char*s,char*d){std::strcpy(n,"Shuttle F-SIM manual HUD");std::strcpy(s,"local.shuttle.fsim-manual-hud");std::strcpy(d,"Aircraft-local native HUD, lightweight landing guidance and staged drag chute.");
 char path[2048]={};XPLMGetPluginInfo(XPLMGetMyID(),nullptr,path,nullptr,nullptr);folder=std::filesystem::path(path).parent_path().parent_path();expected=(folder.parent_path().parent_path()/"Orbiter_Glider.acf").lexically_normal().generic_string();if(!loadOptics()){XPLMDebugString("[ShuttleHUD] Invalid or missing hud-optics.txt; refusing panel drawing.\n");return 0;}loadRunways();checkMatch();chooseRunway();probe=XPLMCreateProbe(xplm_ProbeY);terrain();
 ri("fsim_hud/version",&version);ri("fsim_hud/enabled",&enabled,true);ri("fsim_hud/aircraft_match",&match);ri("fsim_hud/active",&active);ri("fsim_hud/draw_frames",&frames);ri("fsim_hud/declutter_mode",&mode,true);ri("fsim_hud/declutter_level",&level);ri("fsim_hud/runway_index",&runwayIndex);ri("fsim_hud/velocity_limited",&vvLimited);ri("fsim_hud/guidance_limited",&guidanceLimited);ri("fsim_hud/main_wow",&mainWow);ri("fsim_hud/nose_wow",&noseWow);rf("fsim_hud/main_wheel_height_ft",&heightFt);rf("fsim_hud/radar_height_ft",&radarHeightFt);rf("fsim_hud/equivalent_airspeed_kt",&equivalent);rf("fsim_hud/gamma_command_deg",&commandGamma);rf("fsim_hud/speedbrake_command_ratio",&commandSpeedbrake);rf("fsim_hud/brightness",&brightness,true);
 ri("fsim_hud/cockpit_active",&cockpitActive);ri("fsim_hud/cockpit_draw_frames",&panelFrames);ri("fsim_hud/renderer",&renderer);
 ri("fsim_hud/landing_systems_enabled",&landingSystems,true);ri("fsim_hud/chute_reefing_active",&chuteActive);rf("fsim_hud/chute_area_ratio",&chuteRatio);rf("fsim_hud/runway_ground_elevation_m",&runwayGround);
 rf("fsim_hud/vertical_velocity_mps",&verticalVelocity);rf("fsim_hud/groundspeed_mps",&groundVelocity);rf("fsim_hud/ground_track_deg",&groundTrack);
 ri("fsim_hud/plugin_enabled",&pluginEnabled);ri("fsim_hud/guidance_phase",&guidancePhase);rf("fsim_hud/height_command_m",&commandHeight);rf("fsim_hud/gear_command",&commandGear);rf("fsim_hud/along_m",&alongOut);rf("fsim_hud/cross_m",&crossOut);
 rf("fsim_hud/velocity_vector_blend",&vectorBlend);
 ri("fsim_hud/att_ref_caged",&horizontalCage,true);ri("fsim_hud/display_phase",&displayPhase);ri("fsim_hud/display_main_wow",&displayMain);ri("fsim_hud/display_nose_wow",&displayNose);
 ri("fsim_hud/gear_cue",&gearCue);ri("fsim_hud/display_flags",&displayFlags);ri("fsim_hud/control_auto",&controlAuto);
 rf("fsim_hud/display_altitude_ft",&displayAltitude);rf("fsim_hud/deceleration_g",&deceleration);rf("fsim_hud/deceleration_command_g",&decelerationCommand);
 const char*names[]={"fsim_hud/view","fsim_hud/toggle","fsim_hud/declutter_cycle","fsim_hud/next_runway","sim/view/forward_with_hud","fsim_hud/fullscreen","fsim_hud/declutter_auto","fsim_hud/att_ref"};const char*desc[]={"Show shuttle cockpit HUD","Toggle shuttle HUD symbology","Cycle manual HUD declutter","Select next shuttle runway","","Show full-screen shuttle HUD","Use F-SIM automatic declutter","Toggle ATT REF horizontal cage"};
 for(int i=0;i<8;++i){auto c=i==4?XPLMFindCommand(names[i]):XPLMCreateCommand(names[i],desc[i]);commands.push_back(c);XPLMRegisterCommandHandler(c,command,1,nullptr);}
 menuIndex=XPLMAppendMenuItem(XPLMFindPluginsMenu(),"Shuttle HUD",nullptr,0);menu=XPLMCreateMenu("Shuttle HUD",XPLMFindPluginsMenu(),menuIndex,menuAction,nullptr);for(int i:{0,1,2,3,5,6,7})XPLMAppendMenuItem(menu,desc[i],reinterpret_cast<void*>(std::intptr_t(i)),0);
 windowRegistered=XPLMRegisterDrawCallback(draw,xplm_Phase_Window,0,nullptr)!=0;
 panelRegistered=XPLMRegisterDrawCallback(draw,xplm_Phase_Gauges,0,nullptr)!=0;
 if(!windowRegistered||!panelRegistered){cleanup();return 0;}
 XPLMRegisterFlightLoopCallback(loop,-1.f,nullptr);loopRegistered=true;char banner[220];std::snprintf(banner,sizeof(banner),"[ShuttleHUD] v%d registered; manual-based HUD presentation, native optics, shared guidance and staged drag chute. No position or force overrides.\n",version);XPLMDebugString(banner);return 1;}
PLUGIN_API void XPluginStop(){cleanup();XPLMDebugString("[ShuttleHUD] clean stop; presentation settings restored.\n");}
PLUGIN_API int XPluginEnable(){pluginEnabled=1;return 1;}
PLUGIN_API void XPluginDisable(){releaseChute();pluginEnabled=0;active=cockpitActive=0;restoreView();}
PLUGIN_API void XPluginReceiveMessage(XPLMPluginID,int message,void*){if(message==XPLM_MSG_PLANE_LOADED){refs.clear();display.reset();guidance.reset();checkMatch();chooseRunway();terrain();}}
