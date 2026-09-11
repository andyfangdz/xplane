#pragma once
#include <algorithm>
#include <cmath>
#include <string>
namespace shud {
constexpr double pi=3.14159265358979323846,ft=3.280839895,kt=1.943844492;
inline double rad(double d){return d*pi/180;}
inline double deg(double r){return r*180/pi;}
inline double wrap(double d){d=std::fmod(d+180,360);return (d<0?d+360:d)-180;}
struct Point{double x=0,y=0;};
struct Projected{Point p;bool limited=false;};
inline Point rotate(Point p,Point c,double d){double a=rad(d),x=p.x-c.x,y=p.y-c.y;return {c.x+x*std::cos(a)-y*std::sin(a),c.y+x*std::sin(a)+y*std::cos(a)};}
inline Projected project(double az,double el,double hdg,double pitch,double roll,double fx,double fy,Point center){
 double a=rad(wrap(az-hdg)),e=rad(el),p=rad(pitch),r=rad(roll);
 double f=std::cos(e)*std::cos(a)*std::cos(p)+std::sin(e)*std::sin(p);
 double x=std::cos(e)*std::sin(a),y=std::sin(e)*std::cos(p)-std::cos(e)*std::cos(a)*std::sin(p);
 return {{center.x+fx*(x*std::cos(r)-y*std::sin(r))/std::max(.01,f),center.y-fy*(x*std::sin(r)+y*std::cos(r))/std::max(.01,f)},f<=.01};
}
inline Projected constrain(Projected p,double left,double top,double right,double bottom){
 p.limited=p.limited||p.p.x<left||p.p.x>right||p.p.y<top||p.p.y>bottom;
 p.p.x=std::clamp(p.p.x,left,right);p.p.y=std::clamp(p.p.y,top,bottom);return p;
}
inline double eas(double tas,double rho){return std::max(0.,tas)*std::sqrt(std::max(0.,rho)/1.225)*kt;}
inline int clutter(double wheelFt,bool visible,int mode){return mode<0?(wheelFt<4000?2:wheelFt<10000&&visible?1:0):std::clamp(mode,0,2);}
inline double speedTapeY(double actual,double mark){return 535+(mark-actual)*18;}
inline std::string speedText(double equivalent,double ground,bool nose){return (nose?"G":"")+std::to_string(int(std::round(nose?ground:equivalent)));}
inline bool matches(const std::string&loaded,const std::string&expected){return !expected.empty()&&loaded==expected;}
inline double advanceFade(double fade,double previous,double now,bool prefinal){
 if(previous<0||now<previous)return prefinal?5.:0.;
 double dt=std::clamp(now-previous,0.,.2);
 return std::clamp(fade+(prefinal?dt:-dt),0.,5.);
}
}
