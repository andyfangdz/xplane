// Frozen v1.9 flight-loop oracle. No translated controller logic lives here.
// Compile with ap_roll_controller_adapter.cpp; no real X-Plane SDK is needed.
#include <array>
#include <fstream>
#include "sr20g6_chandelle_video_controller.cpp"
template<class T> void put(std::ofstream& out,const T& value){out.write(reinterpret_cast<const char*>(&value),sizeof(value));}
int main(int argc,char**argv){
 if(argc!=2)return 2;
 std::ofstream out(argv[1],std::ios::binary);out.write("XPTAXIS1",8);std::uint32_t count=12000;put(out,count);
 char n[256],sig[256],description[256];if(!XPluginStart(n,sig,description))return 3;XPluginEnable();
 const int slots[]={0,1,2,3,4,5,6,7,8,20,21,22,23,31};
 for(std::uint32_t frame=0;frame<count;frame++){
  int cycle=frame%1000;
  std::array<int,8> flags={frame%777!=0,frame%887!=0,cycle!=0,int((frame/1000)%4),frame%421==0||cycle==20,frame%613==0,cycle>900?6:0,cycle==21?1:0};
  float t=float(frame)*.013f;
  float dt=std::array<float,5>{.002f,.008f,.016f,.033f,.05f}[frame%5];
  if(frame%809==0)dt=.001f;if(frame%811==0)dt=.1f;
  std::array<float,12> sample={25.f*std::sin(t*.73f),12.f*std::sin(t*.39f),float(frame%3600)*.1f,
   .7f*std::cos(t*.73f),.3f*std::cos(t*.39f),.4f*std::sin(t*.19f),8.f*std::sin(t*.37f),
   30.f+float(frame%1300)*.08f,25.f+float(frame%1300)*.05f,dt,-500.f+float(frame%900),cycle>80?0.f:.35f};
  std::array<double,3> geo={40.875+double(frame)*1e-7,-74.278-double(frame)*2e-7,50.0+double(frame%3000)*.1};
  std::array<float,14> targets={42.f*std::sin(t*.23f),.72f,15.f*std::cos(t*.27f),.95f,.45f,
   60.f,2.f,15.f,frame%2000<1000?1.f:-1.f,150.f,20.f,180.f,.3f,209.5f};
  for(auto v:flags)put(out,v);for(auto v:sample)put(out,v);for(auto v:geo)put(out,v);for(auto v:targets)put(out,v);
  enabled=flags[0];match=flags[1];
  if(armed!=flags[2])write_int(reinterpret_cast<void*>(4),flags[2]);
  if(mode!=flags[3])write_int(reinterpret_cast<void*>(7),flags[3]);
  for(int i=0;i<14;i++)write_float(reinterpret_cast<void*>(std::intptr_t(slots[i])),targets[i]);
  XPLMDataRef scalar_refs[]={bank_ref,pitch_ref,heading_ref,p_ref,q_ref,r_ref,beta_ref,ias_ref,tas_ref,frame_ref,vvi_ref,throttle_ref};
  for(int i=0;i<12;i++)static_cast<FakeRef*>(scalar_refs[i])->value=sample[i];
  static_cast<FakeRef*>(latitude_ref)->value=geo[0];static_cast<FakeRef*>(longitude_ref)->value=geo[1];static_cast<FakeRef*>(elevation_ref)->value=geo[2];
  XPLMSetDatai(paused_ref,flags[4]);XPLMSetDatai(replay_ref,flags[5]);XPLMSetDatai(gear_ground_ref,flags[6]);
  XPLMDataRef override_refs[]={override_roll_ref,override_pitch_ref,override_yaw_ref};
  for(int i=0;i<3;i++)XPLMSetDatai(override_refs[i],!!(flags[7]&(1<<i)));
  callback(0,0,0,nullptr);
  for(int i=0;i<14;i++)put(out,read_int(reinterpret_cast<void*>(std::intptr_t(i))));
  for(int i=0;i<32;i++)put(out,read_float(reinterpret_cast<void*>(std::intptr_t(i))));
  for(int i=0;i<3;i++)put(out,read_double(reinterpret_cast<void*>(std::intptr_t(i))));
  put(out,owns_roll);put(out,owns_pitch);put(out,owns_yaw);
 }
 return out.good()?0:4;
}
