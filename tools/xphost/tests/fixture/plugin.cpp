// Original, asset-free SDK fixture for xphost lifecycle, dataref and command tests.
#include "XPLMDataAccess.h"
#include "XPLMPlugin.h"
#include "XPLMProcessing.h"
#include "XPLMUtilities.h"
#include <algorithm>
#include <cstring>
#include <string>
#include <vector>

namespace {
int ready=0,frames=0,begins=0,continues=0,ends=0,consume=0,consume_after=0;
int chain_before=0,chain_second=0,chain_after=0,chain_after_second=0,messages=0;
float value=1.25f;
float array_values[4]={1.f,2.f,3.f,4.f};
std::string bytes="fixture";
std::vector<XPLMDataRef> refs;
XPLMCommandRef pulse=nullptr,chain=nullptr,pause_on=nullptr;

int GetInt(void* ref){return *static_cast<int*>(ref);}
void SetInt(void* ref,int v){*static_cast<int*>(ref)=v;}
float GetFloat(void*){return value;}
void SetFloat(void*,float v){value=v;}
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
float Tick(float,float,int,void*){++frames;return -1.f;}
int Pulse(XPLMCommandRef,XPLMCommandPhase phase,void*){
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
 std::strcpy(name,"xphost test fixture");std::strcpy(signature,"org.xphost.test.fixture");
 std::strcpy(description,"Original asset-free SDK test fixture");
 IntRef("xphost/test/ready",&ready);IntRef("xphost/test/frames",&frames);
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
 XPLMRegisterFlightLoopCallback(Tick,-1.f,nullptr);
 XPLMDebugString("fixture: start\n");return 1;
}
PLUGIN_API int XPluginEnable(){ready=1;XPLMDebugString("fixture: enable\n");return 1;}
PLUGIN_API void XPluginDisable(){ready=0;XPLMDebugString("fixture: disable\n");}
PLUGIN_API void XPluginStop(){
 XPLMUnregisterFlightLoopCallback(Tick,nullptr);XPLMUnregisterCommandHandler(pulse,Pulse,1,nullptr);
 ChainHandlers(chain,false);ChainHandlers(pause_on,false);
 for(auto ref:refs)XPLMUnregisterDataAccessor(ref);
 XPLMDebugString("fixture: stop\n");
}
PLUGIN_API void XPluginReceiveMessage(XPLMPluginID,int,void*){++messages;}
