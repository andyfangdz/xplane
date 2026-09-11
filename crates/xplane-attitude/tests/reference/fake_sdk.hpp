// Test-only SDK implementation. The original controller source is included unchanged.
#pragma once
#include <algorithm>
#include <cstring>
#include <map>
#include <string>
using XPLMDataRef=void*;
using XPLMFlightLoopID=void*;
using XPLMPluginID=int;
using XPLMFlightLoop_f=float(*)(float,float,int,void*);
struct XPLMCreateFlightLoop_t {int structSize;int phase;XPLMFlightLoop_f callbackFunc;void* refcon;};
constexpr int xplmType_Int=1,xplmType_Float=2,xplmType_Double=4;
constexpr int xplm_FlightLoop_Phase_BeforeFlightModel=0,XPLM_MSG_PLANE_LOADED=102;
#define PLUGIN_API
struct FakeRef {double value=0;};
inline std::map<std::string,FakeRef> fake_refs;
inline XPLMDataRef XPLMFindDataRef(const char* name){return &fake_refs[name];}
inline float XPLMGetDataf(XPLMDataRef r){return float(static_cast<FakeRef*>(r)->value);}
inline double XPLMGetDatad(XPLMDataRef r){return static_cast<FakeRef*>(r)->value;}
inline int XPLMGetDatai(XPLMDataRef r){return int(static_cast<FakeRef*>(r)->value);}
inline void XPLMSetDataf(XPLMDataRef r,float v){static_cast<FakeRef*>(r)->value=v;}
inline void XPLMSetDatai(XPLMDataRef r,int v){static_cast<FakeRef*>(r)->value=v;}
inline int XPLMGetDatavi(XPLMDataRef r,int* values,int,int count){
 int mask=XPLMGetDatai(r);for(int i=0;i<count;i++)values[i]=i<3?!!(mask&(1<<i)):0;return count;
}
inline int XPLMGetDatab(XPLMDataRef,void* out,int,int count){
 const char* path="Aircraft/X-Aviation/TorqueSim SR20/SR20.acf";
 std::strncpy(static_cast<char*>(out),path,count);return std::min(int(std::strlen(path)),count);
}
template<class... T> inline XPLMDataRef XPLMRegisterDataAccessor(T...){return nullptr;}
inline void XPLMUnregisterDataAccessor(XPLMDataRef){}
inline XPLMFlightLoopID XPLMCreateFlightLoop(XPLMCreateFlightLoop_t*){return reinterpret_cast<void*>(1);}
inline void XPLMDestroyFlightLoop(XPLMFlightLoopID){}
inline void XPLMScheduleFlightLoop(XPLMFlightLoopID,float,int){}
inline void XPLMDebugString(const char*){}
