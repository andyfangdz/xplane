// Original, asset-free SDK fixture for xphost lifecycle, dataref and command tests.
#include "XPLMDataAccess.h"
#include "XPLMPlugin.h"
#include "XPLMProcessing.h"
#include "XPLMUtilities.h"
#include "XPLMDisplay.h"
#include "XPLMGraphics.h"
#include "XPLMNavigation.h"
#include "XPLMScenery.h"
#include "XPLMWeather.h"
#include <windows.h>
#include <algorithm>
#include <cstring>
#include <cstdlib>
#include <cmath>
#include <string>
#include <vector>

namespace {
void FaultAt(const char* stage){const char* requested=std::getenv("XPHOST_FIXTURE_FAULT");if(requested&&std::strcmp(requested,stage)==0)RaiseException(0xE0425048,0,0,nullptr);}
int ready=0,frames=0,begins=0,continues=0,ends=0,consume=0,consume_after=0;
int chain_before=0,chain_second=0,chain_after=0,chain_after_second=0,messages=0;
float value=1.25f;
float array_values[4]={1.f,2.f,3.f,4.f};
std::string bytes="fixture";
std::vector<XPLMDataRef> refs;
XPLMCommandRef pulse=nullptr,chain=nullptr,pause_on=nullptr;
int contract_failures=0,contract_done=0,contract_loops=0,contract_gap=0,contract_first_frame=0,nav_at_start=0;
int profile_engines_at_start=0;
XPLMFlightLoopID contract_loop=nullptr;
XPLMDataRef self_ref=nullptr;

void Check(const char* name,bool ok){if(!ok){++contract_failures;XPLMDebugString((std::string("contract FAIL: ")+name+"\n").c_str());}}
float ReentrantGet(void*){return XPLMGetDataf(XPLMFindDataRef("xphost/test/value"));}
float BusRatio(void*){return XPLMGetAvionicsBusVoltsRatio(nullptr);}
float UnregisterGet(void*){XPLMUnregisterDataAccessor(self_ref);return 7.f;}
XPLMDataRef FloatRef(const char* name,XPLMGetDataf_f getter){return XPLMRegisterDataAccessor(name,xplmType_Float,0,nullptr,nullptr,getter,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr);}
float ContractLoop(float,float,int frame,void*){
 ++contract_loops;
 if(contract_loops==1){contract_first_frame=frame;XPLMScheduleFlightLoop(contract_loop,-3.f,1);}
 else{contract_gap=frame-contract_first_frame;XPLMDestroyFlightLoop(contract_loop);}
 return -1.f;
}
int Contracts(XPLMCommandRef,XPLMCommandPhase phase,void*){
 if(phase!=xplm_CommandBegin)return 1;
 auto invalid=reinterpret_cast<XPLMDataRef>(static_cast<uintptr_t>(0x7fffffff));
 Check("invalid handle",!XPLMIsDataRefGood(invalid)&&XPLMGetDataf(invalid)==0);
 auto old=FloatRef("xphost/test/temporary",ReentrantGet);
 Check("reentrant accessor",old&&XPLMGetDataf(old)==value);
 XPLMUnregisterDataAccessor(old);
 auto replacement=FloatRef("xphost/test/temporary",ReentrantGet);
 Check("stale handle",replacement&&replacement!=old&&!XPLMIsDataRefGood(old)&&XPLMGetDataf(old)==0);
 XPLMUnregisterDataAccessor(replacement);
 self_ref=FloatRef("xphost/test/self_unregister",UnregisterGet);
 Check("self unregister",XPLMGetDataf(self_ref)==7.f&&!XPLMIsDataRefGood(self_ref));
 auto empty=XPLMRegisterDataAccessor("xphost/test/no_callbacks",xplmType_FloatArray,1,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr);
 Check("empty accessor scalar",empty&&XPLMGetDataf(empty)==0.f);XPLMSetDataf(empty,1.f);XPLMUnregisterDataAccessor(empty);
 int ints[4]={};auto array=XPLMFindDataRef("xphost/test/array");
 Check("array conversion",XPLMGetDatavi(array,ints,0,4)==4&&ints[0]==1&&ints[3]==4);
 Check("array scalar conversion",XPLMGetDatad(array)==1.);
 float single=0;Check("scalar array conversion",XPLMGetDatavf(XPLMFindDataRef("xphost/test/value"),&single,0,1)==1&&single==value);
 auto ro=XPLMFindDataRef("sim/test/read_only");Check("catalog readonly metadata",ro&&!XPLMCanWriteDataRef(ro));XPLMSetDataf(ro,4.5f);Check("sim fixture override",XPLMGetDataf(ro)==4.5f);
 double lat=0,lon=0,alt=0,x=0,y=0,z=0;XPLMLocalToWorld(123.,45.,-678.,&lat,&lon,&alt);XPLMWorldToLocal(lat,lon,alt,&x,&y,&z);
 Check("coordinate roundtrip",std::abs(x-123.)<1e-6&&std::abs(y-45.)<1e-6&&std::abs(z+678.)<1e-6);
 XPLMWeatherInfo_t weather{};weather.structSize=sizeof weather;
 Check("weather",XPLMGetWeatherAtLocation(0,0,1000,&weather)&&std::abs(weather.temperature_alt-8.5f)<0.001f&&weather.pressure_alt>89000&&weather.pressure_alt<91000);
 XPLMCreateWindow_t window{};window.structSize=sizeof window;window.left=1;window.top=20;window.right=30;window.bottom=4;window.visible=1;
 auto w=XPLMCreateWindowEx(&window);int l=0,t=0,r=0,b=0;XPLMGetWindowGeometry(w,&l,&t,&r,&b);Check("window state",w&&l==1&&t==20&&r==30&&b==4&&XPLMGetWindowIsVisible(w));
 Check("window initially unfocused",!XPLMHasKeyboardFocus(w)&&!XPLMHasKeyboardFocus(nullptr));
 XPLMTakeKeyboardFocus(w);Check("take keyboard focus",XPLMHasKeyboardFocus(w));
 XPLMTakeKeyboardFocus(reinterpret_cast<XPLMWindowID>(static_cast<uintptr_t>(0x7fffffff)));Check("invalid focus preserves owner",XPLMHasKeyboardFocus(w));
 XPLMTakeKeyboardFocus(nullptr);Check("release keyboard focus",!XPLMHasKeyboardFocus(w));
 XPLMTakeKeyboardFocus(w);XPLMDestroyWindow(w);Check("destroy clears keyboard focus",!XPLMHasKeyboardFocus(w));
 auto nav=XPLMGetFirstNavAid();Check("nav before plugin start",nav_at_start==1&&nav!=XPLM_NAV_NOT_FOUND);
 XPLMNavType kind=0;float height=0;int frequency=0;char ident[32]{};XPLMGetNavAidInfo(nav,&kind,nullptr,nullptr,&height,&frequency,nullptr,ident,nullptr,nullptr);
 Check("nav types and units",kind==xplm_Nav_VOR&&frequency==11680&&std::abs(height-30.48f)<0.001f&&std::strcmp(ident,"TEST")==0);
 XPLMSetFMSFlightPlanEntryInfo(xplm_Fpl_Pilot_Primary,0,nav,1000);XPLMSetDirectToFMSFlightPlanEntry(xplm_Fpl_Pilot_Primary,0);
 Check("FMS destination",XPLMCountFMSFlightPlanEntries(xplm_Fpl_Pilot_Primary)==1&&XPLMGetGPSDestination()==nav);XPLMClearFMSFlightPlanEntry(xplm_Fpl_Pilot_Primary,0);
 XPLMCreateFlightLoop_t loop{};loop.structSize=sizeof loop;loop.phase=xplm_FlightLoop_Phase_AfterFlightModel;loop.callbackFunc=ContractLoop;contract_loop=XPLMCreateFlightLoop(&loop);XPLMScheduleFlightLoop(contract_loop,-1.f,1);
 contract_done=1;return 1;
}

int GetInt(void* ref){return *static_cast<int*>(ref);}
void SetInt(void* ref,int v){*static_cast<int*>(ref)=v;}
float GetFloat(void*){FaultAt("read");return value;}
void SetFloat(void*,float v){FaultAt("write");value=v;}
int GetArray(void*,float* out,int offset,int count){
 if(!out)return 4;
 if(offset<0||offset>=4||count<=0)return 0;
 const int n=std::min(count,4-offset);std::copy_n(array_values+offset,n,out);return n;
}
void SetArray(void*,float* in,int offset,int count){
 if(!in||offset<0||offset>=4||count<=0)return;
 std::copy_n(in,std::min(count,4-offset),array_values+offset);
}
int GetBytes(void*,void* out,int offset,int count){
 if(!out)return static_cast<int>(bytes.size());
 if(offset<0||offset>=static_cast<int>(bytes.size())||count<=0)return 0;
 const int n=std::min(count,static_cast<int>(bytes.size())-offset);std::memcpy(out,bytes.data()+offset,n);return n;
}
void SetBytes(void*,void* in,int offset,int count){
 if(!in||offset<0||count<0||offset>4096||count>4096-offset)return;
 bytes.resize(static_cast<std::size_t>(offset+count));
 if(count)std::memcpy(bytes.data()+offset,in,static_cast<std::size_t>(count));
}
void IntRef(const char* name,int* target,bool writable=false){
 refs.push_back(XPLMRegisterDataAccessor(name,xplmType_Int,writable?1:0,
  GetInt,writable?SetInt:nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,target,target));
}
float Tick(float,float,int,void*){FaultAt("loop");++frames;return -1.f;}
int Pulse(XPLMCommandRef,XPLMCommandPhase phase,void*){
 FaultAt("command");
 if(phase==xplm_CommandBegin)++begins;
 if(phase==xplm_CommandContinue)++continues;
 if(phase==xplm_CommandEnd)++ends;
 return 1;
}
int First(XPLMCommandRef,XPLMCommandPhase phase,void*){if(phase==xplm_CommandBegin)++chain_before;return consume?0:1;}
int Second(XPLMCommandRef,XPLMCommandPhase phase,void*){if(phase==xplm_CommandBegin)++chain_second;return 1;}
int After(XPLMCommandRef,XPLMCommandPhase phase,void*){if(phase==xplm_CommandBegin)++chain_after;return consume_after?0:1;}
int AfterSecond(XPLMCommandRef,XPLMCommandPhase phase,void*){if(phase==xplm_CommandBegin)++chain_after_second;return 1;}
void ChainHandlers(XPLMCommandRef command,bool add){
 if(add){
  XPLMRegisterCommandHandler(command,First,1,nullptr);
  XPLMRegisterCommandHandler(command,Second,1,nullptr);
  XPLMRegisterCommandHandler(command,After,0,nullptr);
  XPLMRegisterCommandHandler(command,AfterSecond,0,nullptr);
 }else{
  XPLMUnregisterCommandHandler(command,First,1,nullptr);
  XPLMUnregisterCommandHandler(command,Second,1,nullptr);
  XPLMUnregisterCommandHandler(command,After,0,nullptr);
  XPLMUnregisterCommandHandler(command,AfterSecond,0,nullptr);
 }
}
}

PLUGIN_API int XPluginStart(char* name,char* signature,char* description){
 FaultAt("start");
 nav_at_start=XPLMGetFirstNavAid()!=XPLM_NAV_NOT_FOUND;
 profile_engines_at_start=XPLMGetDatai(XPLMFindDataRef("sim/aircraft/engine/acf_num_engines"));
 std::strcpy(name,"xphost test fixture");std::strcpy(signature,"org.xphost.test.fixture");
 std::strcpy(description,"Original asset-free SDK test fixture");
 IntRef("xphost/test/ready",&ready);IntRef("xphost/test/frames",&frames);
 IntRef("xphost/test/profile_engines_at_start",&profile_engines_at_start);
 refs.push_back(FloatRef("xphost/test/bus_ratio",BusRatio));
 IntRef("xphost/test/contract_failures",&contract_failures);IntRef("xphost/test/contract_done",&contract_done);IntRef("xphost/test/contract_loops",&contract_loops);IntRef("xphost/test/contract_gap",&contract_gap);
 IntRef("xphost/test/command_begin",&begins);IntRef("xphost/test/command_continue",&continues);IntRef("xphost/test/command_end",&ends);
 IntRef("xphost/test/consume",&consume,true);IntRef("xphost/test/consume_after",&consume_after,true);
 IntRef("xphost/test/chain_before",&chain_before);IntRef("xphost/test/chain_second",&chain_second);
 IntRef("xphost/test/chain_after",&chain_after);IntRef("xphost/test/chain_after_second",&chain_after_second);IntRef("xphost/test/messages",&messages);
 refs.push_back(XPLMRegisterDataAccessor("xphost/test/value",xplmType_Float,1,nullptr,nullptr,GetFloat,SetFloat,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr));
 refs.push_back(XPLMRegisterDataAccessor("xphost/test/array",xplmType_FloatArray,1,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,GetArray,SetArray,nullptr,nullptr,nullptr,nullptr));
 refs.push_back(XPLMRegisterDataAccessor("xphost/test/bytes",xplmType_Data,1,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,nullptr,GetBytes,SetBytes,nullptr,nullptr));
 if(std::any_of(refs.begin(),refs.end(),[](XPLMDataRef ref){return !ref;}))return 0;
 pulse=XPLMCreateCommand("xphost/test/pulse","Count command phases");
 chain=XPLMCreateCommand("xphost/test/chain","Test SDK command propagation");
 pause_on=XPLMFindCommand("sim/operation/pause_on");
 XPLMRegisterCommandHandler(pulse,Pulse,1,nullptr);ChainHandlers(chain,true);ChainHandlers(pause_on,true);
 XPLMRegisterCommandHandler(XPLMCreateCommand("xphost/test/contracts","Cross-language SDK contracts"),Contracts,1,nullptr);
 XPLMRegisterFlightLoopCallback(Tick,-1.f,nullptr);
 XPLMDebugString("fixture: start\n");return 1;
}
PLUGIN_API int XPluginEnable(){FaultAt("enable");ready=1;XPLMDebugString("fixture: enable\n");return 1;}
PLUGIN_API void XPluginDisable(){FaultAt("disable");ready=0;XPLMDebugString("fixture: disable\n");}
PLUGIN_API void XPluginStop(){
 FaultAt("stop");
 XPLMUnregisterFlightLoopCallback(Tick,nullptr);XPLMUnregisterCommandHandler(pulse,Pulse,1,nullptr);
 ChainHandlers(chain,false);ChainHandlers(pause_on,false);
 for(auto ref:refs)XPLMUnregisterDataAccessor(ref);
 XPLMDebugString("fixture: stop\n");
}
PLUGIN_API void XPluginReceiveMessage(XPLMPluginID,int,void*){FaultAt("message");++messages;}
