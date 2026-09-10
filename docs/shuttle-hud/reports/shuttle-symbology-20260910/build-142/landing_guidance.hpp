#pragma once
#include "hud_math.hpp"

namespace shud {
// JSC-23266 §5.3.6.8: 40% diameter after 1.5 s, disreef after 5 s.
// Area follows diameter squared; the 0.5 s disreef smoothing is a local model.
inline double chuteAreaRatio(double seconds){
 auto smooth=[](double v){v=std::clamp(v,0.,1.);return v*v*(3-2*v);};
 if(seconds<1.5)return .001+.159*smooth(seconds/1.5);
 return .16+.84*smooth((seconds-5)/.5);
}
// Nominal lightweight geometry reconstructed from JSC-23266 Rev B, Appendix B.3
// and USA005512 section 4. It is a C2 geometric reconstruction, not Shuttle GPC
// source code. Coordinates: metres from displaced threshold; positive downrange.
struct PathPoint { double height, slope, curvature; int segment; };
struct LandingPath {
    static constexpr double outerAim=-7500/ft, innerAim=1000/ft;
    static constexpr double circleStart=-12170.711613072857/ft;
    static constexpr double expStart=-4562/ft;
    static constexpr double radius=26069.178277669853/ft;
    static constexpr double expHeight=14.99818857362294/ft;
    static constexpr double expLength=624.112267429558/ft;
    static PathPoint at(double x) {
        const double mo=std::tan(rad(20)), mi=std::tan(rad(1.5));
        if(x<=circleStart)return {(outerAim-x)*mo,-mo,0,1};
        if(x<expStart){
            const double xc=circleStart+radius*std::sin(rad(20));
            const double yc=1700/ft+radius*std::cos(rad(20));
            const double dx=x-xc,z=std::sqrt(radius*radius-dx*dx);
            return {yc-z,dx/z,radius*radius/(z*z*z),2};
        }
        const double e=expHeight*std::exp(-(x-expStart)/expLength);
        return {(innerAim-x)*mi+e,-mi-e/expLength,e/(expLength*expLength),3};
    }
};
struct GuidanceInput {
    double time=0,along=0,height=0,groundspeed=0,vy=0,eas=0,massLb=184000;
    bool mainWow=false;
};
struct LandingGuidance {
    double lastTime=-1,speedbrake=0,gear=0,targetHeight=0,gamma=-20;
    double flareHeight=50/ft,touchTime=-1;
    bool retract3000=false,adjust500=false,finalFlare=false;
    int phase=1;
    void reset(){*this=LandingGuidance{};}
    void update(const GuidanceInput&i){
        if(lastTime<0||i.time<lastTime)reset();
        const double dt=lastTime<0?0:std::clamp(i.time-lastTime,0.,.2);
        lastTime=i.time;
        auto p=LandingPath::at(i.along);
        const double heavy=std::clamp((i.massLb-184000.)/42040.,0.,1.);
        // Local calibration: intercept slightly below the nominal IGS instead
        // of spending an extended period tracking it before the final flare.
        // The smooth offset preserves the preflare join and its initial slope.
        const double u=std::clamp((400-p.height*ft)/300.,0.,1.);
        const double offsetFeet=28-8*heavy;
        const double offset=offsetFeet/ft*u*u*u*(10-15*u+6*u*u);
        p.height-=offset;p.slope*=1+(offsetFeet/300)*30*u*u*(1-u)*(1-u);
        targetHeight=p.height;
        const double gs=std::max(35.,i.groundspeed);
        // Follow the tangent at the current station. The validation pilot's
        // rate feedforward handles actuator response without shifting the path.
        double vcmd=gs*p.slope+std::clamp(.18*(p.height-i.height),-12.,12.);
        phase=i.height>2000/ft?1:2;
        // Handbook final flare is a sink-dependent 30--80 ft trigger. Four
        // seconds of current sink provides lead for the real actuator response.
        if(!finalFlare)flareHeight=std::clamp(-i.vy*4.2,30/ft,80/ft);
        if(i.height<=flareHeight&&i.along>-2500/ft)finalFlare=true;
        if(finalFlare){
            phase=3;
            const double upperSink=1.5+.1*heavy;
            vcmd=-.9144-(upperSink-.9144)*std::clamp(i.height/(15/ft),0.,1.);
        }
        gamma=std::clamp(deg(std::atan2(vcmd,gs)),-24.,2.);
        if(i.height<=300/ft)gear=1;
        // X-Plane speedbrake ratio is a local aerodynamic control. These
        // retraction settings must be calibrated in flight; no continuous speed
        // target is imposed below 3,000 ft. The two event latches are deliberate.
        if(!retract3000&&i.height>3000/ft){
            double cmd=std::clamp(.45+.035*(i.eas-300),0.,.78);
            speedbrake+=std::clamp(cmd-speedbrake,-.5*dt,.5*dt);
        }else if(!retract3000){
            retract3000=true;speedbrake=.105+.145*heavy;
        }
        if(!adjust500&&i.height<=500/ft){
            adjust500=true;speedbrake=.105+.145*heavy;
        }
        if(i.mainWow){phase=4;speedbrake=gear=1;if(touchTime<0)touchTime=i.time;}
    }
};
}
