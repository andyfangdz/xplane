#pragma once
#include "parameters.generated.hpp"
#include <algorithm>
#include <cmath>
#include <string>
#include <sstream>
#include <iomanip>
#include <set>

namespace xpt {
constexpr double pi = 3.14159265358979323846;
inline double rad(double x) { return x*pi/180.; }
inline double deg(double x) { return x*180./pi; }
inline double clamp(double v,double lo,double hi) { return std::max(lo,std::min(v,hi)); }
inline double wrap(double a) { a=std::fmod(a+180.,360.); if(a<0)a+=360.; return a-180.; }
struct Config {
#define FIELD(name, value, low, high) double name=value;
 XPT_PARAMETERS(FIELD)
#undef FIELD
 bool parse(const std::string& text, std::string& error) {
  Config candidate; std::set<std::string> seen; std::istringstream input(text); std::string line;
  while(std::getline(input,line)) {
   if(!line.empty()&&line.back()=='\r')line.pop_back();
   if(line.empty())continue;
   const auto equals=line.find('='); if(equals==std::string::npos){error="missing equals";return false;}
   const auto key=line.substr(0,equals); if(!seen.insert(key).second){error="duplicate "+key;return false;}
   double value; try { std::size_t used; const auto number=line.substr(equals+1); value=std::stod(number,&used); if(used!=number.size())throw 1; }
   catch(...){error="invalid numeric value "+key;return false;}
   if(!std::isfinite(value)){error="non-finite "+key;return false;}
   bool known=false;
#define ASSIGN(name, def, low, high) if(key==#name){known=true;if(value<low||value>high){error="range " #name;return false;}candidate.name=value;}
   XPT_PARAMETERS(ASSIGN)
#undef ASSIGN
   if(!known){error="unknown "+key;return false;}
  }
  int count=0;
#define COUNT(name, def, low, high) ++count;
  XPT_PARAMETERS(COUNT)
#undef COUNT
  if(static_cast<int>(seen.size())!=count){error="incomplete configuration";return false;}
  if(candidate.run_token!=std::floor(candidate.run_token)){error="run_token must be an integer";return false;}
  if(candidate.threshold_lat==candidate.end_lat&&candidate.threshold_lon==candidate.end_lon){error="empty runway";return false;}
  if(candidate.capture_blend_start_deg<=candidate.capture_blend_full_deg){error="capture blend interval";return false;}
  if(candidate.deceleration_start_height_ft<=candidate.deceleration_end_height_ft||candidate.deceleration_end_height_ft<candidate.flare_height_ft||candidate.landing_entry_kias>candidate.final_kias){error="short-final deceleration interval";return false;}
  *this=candidate; return true;
 }
 std::string text()const {
  std::ostringstream out;out<<std::setprecision(17);
#define OUTPUT(name, def, low, high) out<<#name<<'='<<name<<'\n';
  XPT_PARAMETERS(OUTPUT)
#undef OUTPUT
  return out.str();
 }
};

enum Phase { Idle=0, Ready=1, Downwind=2, Delay=3, TurnBase=4, Base=5, TurnFinal=6, Final=7, Rollout=8, Complete=9, Aborted=10 };
enum Reason { None=0, Entry=1, LowAlignment=2, Envelope=3, Timeout=4, SupervisorLost=5, InvalidConfig=6,
              FrameGap=7, WindMismatch=8, MassMismatch=9, OverrideConflict=10, Cancelled=11, MissingDataref=12, TraceError=13 };
struct Sample {
 double t=0,x=0,y=0,h=0,ias=0,tas_fps=0,gs_fps=0,heading=0,track=0,bank=0,pitch=0,vvi=0,vy=0;
 double wind_kt=0,wind_dir=0,mass_lb=0,throttle=0; bool ground=false;
 double cross_velocity(double runway)const{return gs_fps*std::sin(rad(track-runway));}
};
struct Controller {
 Config c; Phase phase=Idle; Reason reason=None;
 double heading=0,pitch=4,bank=0,throttle=.35,flap=0,lead=0,desired=0,accel=0,wind_rate=0,wind_ff=0,pitch_rate=0;
 double start_t=0,cut_t=-1,roundout_t=-1,contact_t=-1,gate_s=0,dt=0,delay_s=0;
 double speed_integral=0,altitude_integral=0,throttle_trim=.35;
 double cross_accel=0,predicted_cross=0,actual_pitch_rate=0;
 Sample last{}; bool has_last=false; unsigned long long steps=0;
 explicit Controller(const Config& config=Config()):c(config){ reset(config); }
 void reset(const Config& config){
  c=config;phase=Ready;reason=None;pitch=c.initial_pitch_deg;bank=0;throttle=c.initial_throttle;flap=0;
  lead=desired=accel=wind_rate=wind_ff=pitch_rate=0;cut_t=roundout_t=contact_t=-1;gate_s=0;
  speed_integral=altitude_integral=0;throttle_trim=throttle;has_last=false;steps=0;dt=0;
  cross_accel=predicted_cross=actual_pitch_rate=0;
  const double n=(c.end_lat-c.threshold_lat)*60*6076.12;
  const double e=(c.end_lon-c.threshold_lon)*60*6076.12*std::cos(rad((c.threshold_lat+c.end_lat)*.5));
  heading=deg(std::atan2(e,n));if(heading<0)heading+=360;
  const double h=c.wind_speed_kt*std::cos(rad(c.wind_offset_deg)),cross=c.wind_speed_kt*std::sin(rad(c.wind_offset_deg));
  delay_s=std::max(0.,c.delay_base_s+c.delay_headwind_s_per_kt*h+
    c.delay_strong_headwind_s_per_kt*std::max(0.,h-c.flare_headwind_threshold_kt)+
    c.delay_tailwind_s_per_kt*std::max(0.,-h)+c.delay_crosswind_s_per_kt*cross+c.delay_crosswind_abs_s_per_kt*std::abs(cross));
 }
 void start(double time){phase=Downwind;start_t=time;}
 bool running()const{return phase>=Downwind&&phase<=Rollout;}
 void abort(Reason why){reason=why;phase=Aborted;throttle=0;bank=0;}
 double wind_heading(double track,const Sample&s,double tas_kt=0)const{
  const double tas=std::max(30.,tas_kt>0?tas_kt:s.tas_fps/1.68780986);
  return track+deg(std::asin(clamp(s.wind_kt*std::sin(rad(s.wind_dir-track))/tas,-.5,.5)));
 }
 double turn_lead(const Sample&s)const{
  const double ratio=s.tas_fps/1.68780986/std::max(50.,s.ias);
  const double v=(.4*s.ias+.6*(c.final_kias-1.5))*ratio*1.68780986;
  const double begin=wrap(s.heading-heading),end=wrap(wind_heading(heading,s,(c.final_kias-1.5)*ratio)-heading)-3;
  const double duration=std::max(0.,rad(wrap(end-begin))*v/(32.174*std::tan(rad(c.turn_mean_bank_deg))));
  const double radius=v*v/(32.174*std::tan(rad(c.turn_bank_deg)));
  const double wa=rad(s.wind_dir-heading),cross=s.wind_kt*std::sin(wa),head=s.wind_kt*std::cos(wa);
  return std::max(500.,radius*(std::cos(rad(end))-std::cos(rad(begin)))+c.turn_lead_extra_ft+
         c.turn_lead_headwind_ft_per_kt*head+c.turn_lead_crosswind_ft_per_kt*cross+
         c.turn_lead_crosswind_abs_ft_per_kt*std::abs(cross)+cross*1.68780986*duration);
 }
 double geometry_bank(const Sample&s,double cross_v){
  // Solve the remaining wind-drifted circular arc from the predicted state.
  // This corrects the radius throughout the turn, before centerline capture.
  const double look=c.lateral_lookahead_s;
  const double y=s.y+cross_v*look+.5*cross_accel*look*look;
  const double ratio=s.tas_fps/1.68780986/std::max(50.,s.ias);
  const double v=std::max(60.,(.4*s.ias+.6*(c.final_kias-1.5))*ratio*1.68780986);
  const double begin=rad(wrap(s.heading-heading))+32.174*std::tan(rad(s.bank))/std::max(60.,s.tas_fps)*look;
  const double end=rad(wrap(wind_heading(heading,s,v/1.68780986)-heading));
  const double cross=s.wind_kt*std::sin(rad(s.wind_dir-heading))*1.68780986;
  const double numerator=v*v*(std::cos(end)-std::cos(begin))+cross*v*std::max(0.,end-begin);
  return clamp(deg(std::atan2(std::max(0.,numerator),32.174*std::max(10.,y))),0,c.turn_capture_bank_max_deg);
 }
 double centerline_bank(const Sample&s,double cross_v){
  // Predict lateral state over the attitude response delay. The measured
  // acceleration includes bank response and changes in local wind.
  const double look=c.lateral_lookahead_s;
  predicted_cross=s.y+cross_v*look+.5*cross_accel*look*look;
  const double velocity=cross_v+cross_accel*look;
  return clamp(deg(std::atan2(-c.final_position_gain*predicted_cross-c.final_velocity_gain*velocity,32.174))+c.bank_bias_deg,
               s.h<50?-7:-20,s.h<50?7:20);
 }
 double final_speed_target(const Sample&s)const{
  if(phase!=Final)return c.final_kias;
  const double start=std::max(c.deceleration_end_height_ft+20.,c.deceleration_start_height_ft-
    c.deceleration_headwind_start_ft_per_kt*strong_headwind());
  const double fraction=clamp((start-s.h)/(start-c.deceleration_end_height_ft),0,1);
  const double blend=fraction*fraction*(3-2*fraction);
  const double tail=std::max(0.,-c.wind_speed_kt*std::cos(rad(c.wind_offset_deg)));
  const double cross=std::abs(c.wind_speed_kt*std::sin(rad(c.wind_offset_deg)));
  const double landing=std::max(65.,c.landing_entry_kias-c.landing_tailwind_kias_per_kt*tail-
    c.landing_crosswind_kias_per_kt*cross);
  return c.final_kias+c.final_strong_headwind_kias_per_kt*strong_headwind()+(landing-c.final_kias)*blend;
 }
 double strong_headwind()const{
  return std::max(0.,c.wind_speed_kt*std::cos(rad(c.wind_offset_deg))-c.flare_headwind_threshold_kt);
 }
 double roundout_rate_limit()const{
  // Make some of the existing wind-response authority available before the
  // measured near-ground wind loss. The overall command slew ceiling is fixed.
  const double cross=std::abs(c.wind_speed_kt*std::sin(rad(c.wind_offset_deg)));
  const double base=std::max(.3,c.flare_max_pitch_rate_deg_s-c.flare_crosswind_rate_reduction*cross);
  return base+std::min(c.flare_wind_positive_limit,
    c.flare_headwind_rate_gain*strong_headwind()+std::max(0.,wind_ff));
 }
 double roundout_pitch_limit()const{
  return std::min(12.,c.flare_max_pitch_deg+c.flare_headwind_pitch_gain*strong_headwind());
 }
 double approach_path_bias(const Sample&s)const{
  if(phase!=Final||roundout_t>=0||s.h<=c.flare_height_ft||s.h>=c.path_start_height_ft)return 0;
  // Empirical roundout travel for this aircraft/loading and lower roundout.
  const double head=s.wind_kt*std::cos(rad(s.wind_dir-heading));
  const double cross=s.wind_kt*std::sin(rad(s.wind_dir-heading));
  const double travel=c.path_flare_distance_calm_ft+c.path_flare_headwind_ft_per_kt*head+
                      c.path_flare_headwind_sq_ft_per_kt2*head*head+c.path_flare_crosswind_ft_per_kt*cross;
  const double along=std::max(80.,s.gs_fps*std::cos(rad(s.track-heading)));
  const double projected=s.x+(s.h-c.flare_height_ft)*along/clamp(-s.vy,10,20);
  const double error=projected-(c.path_target_touchdown_ft-travel);
  return clamp(-c.path_pitch_gain_deg_per_ft*error,-c.path_pitch_limit_deg,c.path_pitch_limit_deg);
 }
 void step(const Sample&s){
  if(!running())return;
  dt=has_last?s.t-last.t:0.;
  if(dt<0){abort(FrameGap);return;}
  if(dt==0){last=s;has_last=true;return;}
  if(dt>c.maximum_frame_dt_s){abort(FrameGap);return;}
  ++steps;
  if(s.t-start_t>c.timeout_sim_s){abort(Timeout);return;}
  const double cross_v=s.cross_velocity(heading);
  cross_accel+=dt/(c.lateral_accel_filter_s+dt)*((cross_v-last.cross_velocity(heading))/dt-cross_accel);
  actual_pitch_rate+=dt/(.06+dt)*((s.pitch-last.pitch)/dt-actual_pitch_rate);
  accel+=dt/(.4+dt)*((s.vy-last.vy)/dt-accel);
  const double head=s.wind_kt*std::cos(rad(s.wind_dir-s.heading));
  const double oldhead=last.wind_kt*std::cos(rad(last.wind_dir-last.heading));
  wind_rate+=dt/(.25+dt)*((head-oldhead)/dt-wind_rate);
  if(phase==Downwind){
   const bool stable=std::abs(s.ias-c.entry_kias)<=c.entry_speed_tolerance&&std::abs(s.h-c.entry_agl_ft)<=c.entry_height_tolerance&&
                     std::abs(s.y-c.entry_cross_ft)<=c.entry_cross_tolerance&&std::abs(s.bank)<=c.entry_bank_tolerance;
   gate_s=stable?gate_s+dt:0;
   const double ae=c.entry_agl_ft-s.h;
   altitude_integral=clamp(altitude_integral+c.altitude_integral_gain*ae*dt,-c.altitude_integral_limit,c.altitude_integral_limit);
   if(std::abs(s.vvi)<=150){
    throttle_trim=clamp(throttle_trim+c.throttle_integral_gain*(c.entry_kias-s.ias)*dt,c.throttle_min,c.throttle_max);
    throttle=clamp(throttle_trim+c.throttle_proportional_gain*(c.entry_kias-s.ias),c.throttle_min,c.throttle_max);
   }
   const double raw=clamp(c.level_pitch_deg+c.altitude_gain*ae+altitude_integral-c.vvi_gain*s.vvi,-7,8);
   pitch+=clamp(raw-pitch,-1.25*dt,1.25*dt);
   bank=clamp(.9*wrap(wind_heading(heading+180,s)-s.heading)-.003*(c.entry_cross_ft-s.y),-12,12);
   if(last.x>c.cut_along_ft&&s.x<=c.cut_along_ft){
    if(gate_s<c.entry_gate_s){abort(Entry);return;}
    cut_t=s.t;phase=Delay;pitch=s.pitch;throttle=0;
   }
   if(s.ground){abort(Entry);return;}
  }
  if(cut_t>=0){
   if(std::abs(s.mass_lb-c.mass_target_lb)>c.mass_tolerance_lb){abort(MassMismatch);return;}
   if(s.h>25&&(std::abs(s.wind_kt-c.wind_speed_kt)>c.wind_tolerance_kt||
      (c.wind_speed_kt>0&&std::abs(wrap(s.wind_dir-heading-c.wind_offset_deg))>c.wind_tolerance_deg))){abort(WindMismatch);return;}
   if(s.ground&&contact_t<0){contact_t=s.t;phase=Rollout;}
   if(phase==Rollout){
    throttle=0;bank=0;pitch+=clamp(2-pitch,-dt,dt);
    if(s.t-contact_t>=c.rollout_s)phase=Complete;
    last=s;return;
   }
   throttle=0;double target=c.turn_kias;lead=turn_lead(s);
   if(phase==Delay&&s.t-cut_t>=delay_s)phase=TurnBase;
   if(phase==TurnBase&&std::abs(wrap(wind_heading(heading-90,s)-s.heading))<5){phase=Base;flap=.5;}
   if(phase==Base&&s.y<=lead)phase=TurnFinal;
   if(phase==TurnFinal&&std::abs(wrap(wind_heading(heading,s)-s.heading))<3)phase=Final;
   if(phase==Delay)bank=clamp(.9*wrap(wind_heading(heading+180,s)-s.heading),-12,12);
   else if(phase==TurnBase)bank=c.turn_bank_deg;
   else if(phase==Base)bank=clamp(wrap(wind_heading(heading-90,s)-s.heading),-20,20);
   else if(phase==TurnFinal){
    const double error=wrap(wind_heading(heading,s)-s.heading);
    const double weight=clamp((c.capture_blend_start_deg-std::abs(error))/(c.capture_blend_start_deg-c.capture_blend_full_deg),0,1);
    const double blend=weight*weight*(3-2*weight);
    bank=(1-blend)*geometry_bank(s,cross_v)+blend*centerline_bank(s,cross_v);target=c.final_kias;
   }
   else {bank=centerline_bank(s,cross_v);target=final_speed_target(s);}
   if(phase>=Base&&flap<1&&(s.y<=3000||(phase==Final&&s.x>=-600)))flap=1;
   speed_integral=clamp(speed_integral+.025*(s.ias-target)*dt,-4,4);
   const double full_ff=-3.5-(target-75)/6,ff=flap<=.5?-9*flap:-4.5+(full_ff+4.5)*2*(flap-.5);
   const double raw=-.8+.35*(s.ias-target)+speed_integral+ff+approach_path_bias(s);
   if(phase==Final&&s.h<=c.flare_height_ft&&std::abs(s.y)<80&&roundout_t<0){roundout_t=s.t;}
   if(roundout_t>=0){
    const double va=c.flare_vertical_accel_fps2,ph=std::max(0.,s.h+s.vy*c.flare_lookahead_s);
    desired=-std::sqrt(c.flare_contact_sink_fps*c.flare_contact_sink_fps+2*va*ph);
    const double ades=ph>0?clamp(-va*(s.vy+c.flare_lookahead_s*accel)/std::max(1.,std::abs(desired)),0,3):0;
    wind_ff=clamp(-c.flare_wind_loss_gain*wind_rate,-c.flare_wind_negative_limit,c.flare_wind_positive_limit);
    pitch_rate=clamp(deg(ades/std::max(80.,s.tas_fps))+c.flare_velocity_gain*(desired-s.vy)-
                     c.flare_acceleration_gain*(accel-ades)+wind_ff,-.8,roundout_rate_limit());
    // Damping follows saturation, so an excessive physical pitch rate reduces
    // the command even when vertical-speed error requests maximum authority.
    pitch_rate=std::max(-.8,pitch_rate-c.flare_pitch_rate_feedback_gain*
                       std::max(0.,actual_pitch_rate-c.flare_pitch_rate_feedback_target));
    pitch=clamp(pitch+pitch_rate*dt,-5,roundout_pitch_limit());
   }else pitch+=clamp(raw-pitch,-1.25*dt,1.25*dt);
   if(s.h<60&&(s.x<-600||std::abs(s.y)>150)){abort(LowAlignment);return;}
   if(s.ias<c.minimum_ias_abort||std::abs(s.bank)>c.maximum_bank_abort){abort(Envelope);return;}
  }
  last=s;
 }
};
} // namespace xpt
