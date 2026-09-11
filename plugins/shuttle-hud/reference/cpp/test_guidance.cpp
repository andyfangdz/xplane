#include "landing_guidance.hpp"
#undef NDEBUG
#include <cassert>
#include <iostream>
int main(){
 using namespace shud;
 assert(chuteAreaRatio(0)==.001&&std::abs(chuteAreaRatio(1.5)-.16)<1e-9&&chuteAreaRatio(4)==.16&&chuteAreaRatio(6)==1);
 // Geometric joins may not introduce a height, angle or acceleration impulse.
 for(double x:{LandingPath::circleStart,LandingPath::expStart}){
  auto a=LandingPath::at(x-1e-5),b=LandingPath::at(x+1e-5);
  assert(std::abs(a.height-b.height)<1e-4);
  assert(std::abs(a.slope-b.slope)<1e-7);
  if(x==LandingPath::expStart)assert(std::abs(a.curvature-b.curvature)<1e-9);
 }
 assert(std::abs(LandingPath::at(LandingPath::circleStart).height*ft-1700)<.001);
 assert(std::abs(deg(std::atan(LandingPath::at(-5000).slope))+20)<1e-8);
 assert(std::abs(LandingPath::at(0).height*ft-26.2)<.02);
 assert(std::abs(deg(std::atan(LandingPath::at(0).slope))+1.5)<.001);
 double lastH=1e9;
 for(double x=-7000;x<100;x+=1){auto p=LandingPath::at(x);assert(std::isfinite(p.height)&&p.height<lastH&&p.slope<0);lastH=p.height;}
 LandingGuidance g;GuidanceInput i;i.time=1;i.height=4000/ft;i.along=-6500;i.eas=300;i.groundspeed=165;i.vy=-55;g.update(i);
 i.time=2;i.height=2900/ft;g.update(i);const double sb3000=g.speedbrake;
 i.time=3;i.height=1500/ft;i.eas=315;g.update(i);assert(g.speedbrake==sb3000);
 i.time=4;i.height=490/ft;i.eas=275;g.update(i);const double sb500=g.speedbrake;
 i.time=5;i.height=250/ft;i.eas=230;g.update(i);assert(g.speedbrake==sb500&&g.gear==1);
 i.time=6;i.along=-200;i.height=35/ft;i.vy=-3;g.update(i);assert(g.phase==3);
 i.time=7;i.mainWow=true;g.update(i);assert(g.phase==4&&g.speedbrake==1);
 i.time=0;i.mainWow=false;i.height=4000/ft;i.along=-6500;g.update(i);assert(!g.finalFlare&&!g.retract3000&&g.gear==0);
 std::cout<<"Landing geometry, event latches and reset tests passed\n";
}
