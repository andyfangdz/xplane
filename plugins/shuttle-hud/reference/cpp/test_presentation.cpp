#include "hud_presentation.hpp"
#include <cassert>
#include <iostream>
using namespace shud;
int main(){
    assert(digitalHeight(50.9)==50&&digitalHeight(59.9)==50&&digitalHeight(400)==400);
    assert(digitalHeight(499)==400&&digitalHeight(1000)==1000&&digitalHeight(1199)==1000);
    assert(digitalHeight(1200)==1200&&digitalHeight(-1)==0);
    assert(altitudeStep(500)==50&&altitudeStep(500.1)==100&&altitudeStep(1000)==100);
    assert(altitudeStep(1000.1)==1000&&altitudeStep(100000)==1000&&altitudeStep(100001)==10000);
    assert(indicatedSpeed(199.9)==199&&indicatedSpeed(501)==500);
    HudPresentation h;HudInput i;i.heightFt=20000;i.time=10;i.bank=40;i.groundspeed=150;
    h.update(i);assert(h.phase==HudPhase::Hdg&&h.nzVisible()&&h.fade==0);
    i.heightFt=19900;i.bank=0;h.update(i);i.heightFt=17800;h.update(i);
    h.reset();i.time=10;i.heightFt=18100;h.update(i);i.heightFt=17900;
    for(int n=1;n<=51;++n){i.time=10+n*.1;h.update(i);}assert(h.phase==HudPhase::Prfnl&&h.fade==5&&!h.nzVisible());
    i.heightFt=9000;i.eas=300;i.pathErrorFt=300;i.gammaError=1;h.update(i);assert(h.phase==HudPhase::Capt);
    for(int n=0;n<41;++n){i.time+=.1;h.update(i);}assert(h.phase==HudPhase::Ogs);
    i.heightFt=1800;h.update(i);assert(h.phase==HudPhase::Flare);
    i.heightFt=100;i.eas=250;h.update(i);assert(h.gearCue==1);
    i.gear={1,.5,1};h.update(i);assert(h.gearCue==2);
    i.gear={1,1,1};h.update(i);assert(h.gearCue==3);
    i.time+=5;h.update(i);assert(h.gearCue==0);
    i.finalFlare=true;h.update(i);assert(h.phase==HudPhase::Fnlfl&&!h.guidanceVisible());
    i.automatic=true;h.update(i);assert(h.guidanceVisible());
    assert(h.level(3,100,true)==3&&h.level(-1,3900,true)==2);
    i.main=true;i.heightFt=0;h.update(i);assert(h.main&&!h.nose&&h.gearCue==0&&!h.guidanceVisible());
    assert(h.level(3,0,true)==0&&h.attitudeVisible(0)&&h.horizonVisible(0));
    i.main=false;i.heightFt=5;h.update(i);assert(h.main);
    i.nose=true;h.update(i);assert(h.nose&&!h.horizonVisible(0)&&!h.attitudeVisible(0));
    i.nose=false;h.update(i);assert(h.nose);
    h.groundDeclutter=2;assert(h.level(0,0,true)==3);
    i.time=1;i.heightFt=15000;i.finalFlare=false;h.update(i);assert(!h.main&&!h.nose&&h.fade==5);
    i.main=true;h.update(i);i.main=false;i.replay=true;h.update(i);assert(!h.main);
    i.main=true;i.heightFt=0;h.update(i);i.main=false;i.heightFt=150;i.along+=1000;h.update(i);assert(!h.main&&!h.nose);
    h.reset();i.time=.1;i.heightFt=0;i.main=true;i.nose=true;h.update(i);
    i.time=.8;i.heightFt=20;i.main=false;i.nose=false;h.update(i);assert(!h.main&&!h.nose);
    i.time=50;i.heightFt=0;i.main=true;h.update(i);i.time=50.1;i.main=false;i.heightFt=5;h.update(i);assert(h.main);
    i.time=51;i.heightFt=100;h.update(i);assert(!h.main&&!h.nose);
    std::cout<<"HUD presentation: phase, five-second timers, numeric boundaries, declutter, CSS/AUTO and contact/reset checks passed\n";
}
