#pragma once
#include "hud_math.hpp"
#include <array>

namespace shud {
// JSC-23266 Rev B 2.12; phase acquisition is a local geometry estimate because
// this aircraft does not publish Shuttle GPC/TAEM phase words.
enum class HudPhase { Acq, Hdg, Prfnl, Capt, Ogs, Flare, Fnlfl };
inline const char* phaseLabel(HudPhase p){
    switch(p){case HudPhase::Acq:return "ACQ";case HudPhase::Hdg:return "HDG";
    case HudPhase::Prfnl:return "PRFNL";case HudPhase::Capt:return "CAPT";
    case HudPhase::Ogs:return "OGS";case HudPhase::Flare:return "FLARE";
    case HudPhase::Fnlfl:return "FNLFL";}return "";
}
inline int digitalHeight(double h){
    h=std::clamp(h,0.,32767.);
    const double step=h>1000?200:h>400?100:h>50?10:1;
    return int(std::floor(h/step)*step);
}
inline double altitudeStep(double h){return h>100000?10000:h>1000?1000:h>500?100:50;}
inline int indicatedSpeed(double s){return int(std::clamp(s,0.,500.));}
struct HudInput {
    double time=0,heightFt=0,eas=0,groundspeed=0,headingError=0,crossFt=0;
    double pathErrorFt=0,gammaError=0,bank=0,stopDistance=0,along=0;
    std::array<double,3> gear{};
    bool main=false,nose=false,finalFlare=false,automatic=false,replay=false,runwayVisible=true;
};
struct HudPresentation {
    HudPhase phase=HudPhase::Acq;
    bool main=false,nose=false,replay=false,automatic=false;
    double lastTime=-1,lastHeight=0,lastAlong=0,lastSpeed=0,fade=0,gearLockTime=-1,captureSeconds=0,deceleration=0,requiredDeceleration=0;
    int groundDeclutter=0,gearCue=0; // 0 blank, 1 GEAR, 2 GR, 3 GR-DN
    void reset(){*this=HudPresentation{};}
    void update(const HudInput&i){
        // Time discontinuities/replay crossings and an airborne reposition after
        // landing start a new display sequence. Ordinary rebounds retain WOW.
        if(lastTime>=0&&(i.time<lastTime||i.replay!=replay||std::abs(i.heightFt-lastHeight)>500||std::abs(i.along-lastAlong)>500||
            (main&&!i.main&&(i.heightFt>50||(i.time<3&&i.heightFt>5)))))reset();
        const bool first=lastTime<0;
        const double dt=first?0:std::clamp(i.time-lastTime,0.,.2);
        replay=i.replay;automatic=i.automatic;
        if(!main&&(i.main||i.nose)){main=true;groundDeclutter=0;}
        nose=nose||(main&&i.nose);
        const bool prefinal=std::abs(i.headingError)<35&&i.heightFt<18000;
        if(!main){
            if(i.finalFlare)phase=HudPhase::Fnlfl;
            else if(i.heightFt<=2000)phase=HudPhase::Flare;
            else if(phase<HudPhase::Capt){
                phase=prefinal?HudPhase::Prfnl:std::abs(i.bank)>10?HudPhase::Hdg:HudPhase::Acq;
                if(i.heightFt<=5000||(prefinal&&i.heightFt<10000&&std::abs(i.pathErrorFt)<1000&&
                    std::abs(i.crossFt)<1000&&std::abs(i.gammaError)<4&&i.eas>=288&&i.eas<=312))phase=HudPhase::Capt;
            }
            if(phase==HudPhase::Capt){
                captureSeconds=std::abs(i.gammaError)<2?captureSeconds+dt:0;
                if((std::abs(i.pathErrorFt)<50&&std::abs(i.gammaError)<2)||captureSeconds>=4)phase=HudPhase::Ogs;
            }
        }
        fade=advanceFade(fade,lastTime,i.time,prefinal||phase>=HudPhase::Capt);
        const bool locked=std::all_of(i.gear.begin(),i.gear.end(),[](double g){return g>=.99;});
        const bool up=std::all_of(i.gear.begin(),i.gear.end(),[](double g){return g<=.01;});
        if(locked){if(gearLockTime<0)gearLockTime=i.time;}else gearLockTime=-1;
        gearCue=main?0:locked?(i.time-gearLockTime<5?3:0):!up?2:(i.heightFt<300&&i.eas<300?1:0);
        if(main){
            if(dt>0){double a=std::clamp((lastSpeed-i.groundspeed)/(dt*9.80665),-.4,.8);
                deceleration+=(a-deceleration)*(1-std::exp(-dt/.5));}
            // Reconstructed indication: stop 1,000 ft before the selected runway end.
            requiredDeceleration=i.groundspeed*i.groundspeed/(2*std::max(1.,i.stopDistance)*9.80665);
        }
        lastTime=i.time;lastHeight=i.heightFt;lastAlong=i.along;lastSpeed=i.groundspeed;
    }
    int level(int mode,double height,bool visible)const{
        return main?(groundDeclutter==2?3:groundDeclutter):mode<0?clutter(height,visible,mode):std::clamp(mode,0,3);
    }
    bool guidanceVisible()const{return !main&&(phase!=HudPhase::Fnlfl||automatic);}
    bool nzVisible()const{return !main&&phase<HudPhase::Prfnl;}
    bool attitudeVisible(int level)const{return !nose&&(main?level==0:level<2);}
    bool horizonVisible(int level)const{return !nose&&level<3&&(!main||level==0);}
};
}
