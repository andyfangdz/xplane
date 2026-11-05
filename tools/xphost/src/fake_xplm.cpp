// Fake XPLM_64.dll for xphost: a typed dataref store, command and flight-loop plumbing, flat
// terrain, ISA weather, a mini nav database and harmless UI stubs. X-Plane's flight model and
// rendering are absent by design; see README.md. Built with XPLM=1 so the SDK headers export.
#include "XPLMDefs.h"
#include "XPLMDataAccess.h"
#include "XPLMProcessing.h"
#include "XPLMUtilities.h"
#include "XPLMPlugin.h"
#include "XPLMPlanes.h"
#include "XPLMMenus.h"
#include "XPLMDisplay.h"
#include "XPLMGraphics.h"
#include "XPLMNavigation.h"
#include "XPLMScenery.h"
#include "XPLMWeather.h"
#include "XPLMSound.h"
#include "XPLMInstance.h"
#include "xphost_api.h"
#include <algorithm>
#include <array>
#include <cmath>
#include <cstdio>
#include <cstring>
#include <map>
#include <mutex>
#include <set>
#include <string>
#include <vector>
namespace {
std::recursive_mutex g_mu;
FILE* g_log=nullptr;std::set<std::string> g_said;std::string g_error;
bool g_faulted=false;long long g_frame=0;double g_now=0,g_dt=1./60.;
long long g_sim_r=0,g_sim_w=0,g_plug_r=0,g_plug_w=0,g_cmd_fired=0,g_loops_run=0;
std::string g_system_path,g_plugin_path,g_acf_path,g_acf_name,g_self_name,g_self_sig,g_self_desc;
int g_enabled=0;double g_lat0=0,g_lon0=0,g_elev0=0,g_magvar=0;
XpHostMessageFn g_msg_fn=nullptr;void* g_msg_ref=nullptr;XpHostCommandFn g_cmd_fn=nullptr;void* g_cmd_ref=nullptr;
void LogRaw(const char* s){if(g_log){std::fputs(s,g_log);std::fflush(g_log);}}
void LogLine(const std::string& s){LogRaw(("xphost: "+s+"\n").c_str());}
void LogOnce(const std::string& s){if(g_said.insert(s).second)LogLine(s);}
void PluginFault(const std::string& s){g_faulted=true;LogOnce(s);}
std::string Trim(const std::string& s){const auto a=s.find_first_not_of(" \t\r\n"),b=s.find_last_not_of(" \t\r\n");return a==std::string::npos?"":s.substr(a,b-a+1);}
// ---- dataref store ---------------------------------------------------------------------------
struct Entry {
 std::string name;int types=0;bool writable=false,owned=false,alive=true;
 double scalar=0;std::vector<float> fa;std::vector<int> ia;std::vector<char> bytes;
 XPLMGetDatai_f gi=nullptr;XPLMSetDatai_f si=nullptr;XPLMGetDataf_f gf=nullptr;XPLMSetDataf_f sf=nullptr;
 XPLMGetDatad_f gd=nullptr;XPLMSetDatad_f sd=nullptr;XPLMGetDatavi_f gvi=nullptr;XPLMSetDatavi_f svi=nullptr;
 XPLMGetDatavf_f gvf=nullptr;XPLMSetDatavf_f svf=nullptr;XPLMGetDatab_f gb=nullptr;XPLMSetDatab_f sb=nullptr;
 void* rref=nullptr;void* wref=nullptr;
};
std::map<std::string,Entry*> g_by_name;std::vector<Entry*> g_order;std::set<std::string> g_misses;
Entry* NewEntry(const std::string& name,int types,bool writable,bool owned){
 auto* e=new Entry();e->name=name;e->types=types;e->writable=writable;e->owned=owned;
 g_by_name[name]=e;g_order.push_back(e);return e;
}
Entry* FindEntry(const char* name,bool note_miss){
 if(!name)return nullptr;auto it=g_by_name.find(name);
 if(it==g_by_name.end()||(it->second->owned&&!it->second->alive)){if(note_miss)g_misses.insert(name);return nullptr;}
 return it->second;
}
Entry* AsEntry(XPLMDataRef r){return static_cast<Entry*>(r);}
// Owned accessor dispatch with XPLM-style conversion between int/float/double. No throws out.
double OwnedScalar(Entry* e)noexcept{
 try{
  if(e->gd)return e->gd(e->rref);if(e->gf)return e->gf(e->rref);if(e->gi)return e->gi(e->rref);
  if(e->gvf){float v=0;return e->gvf(e->rref,&v,0,1)?v:0.;}
  if(e->gvi){int v=0;return e->gvi(e->rref,&v,0,1)?v:0.;}
 }catch(...){PluginFault("dataref read fault in plugin accessor: "+e->name);}
 return 0.;
}
void OwnedSetScalar(Entry* e,double v)noexcept{
 try{
  if(e->sd){e->sd(e->wref,v);return;}if(e->sf){e->sf(e->wref,static_cast<float>(v));return;}
  if(e->si){e->si(e->wref,static_cast<int>(v));return;}
  if(e->svf){float f=static_cast<float>(v);e->svf(e->wref,&f,0,1);return;}
  if(e->svi){int i=static_cast<int>(v);e->svi(e->wref,&i,0,1);return;}
 }catch(...){PluginFault("dataref write fault in plugin accessor: "+e->name);}
}
int EntryLength(Entry* e)noexcept{
 if(e->owned){
  try{
   if(e->gvf)return e->gvf(e->rref,nullptr,0,0);
   if(e->gvi)return e->gvi(e->rref,nullptr,0,0);
   if(e->gb)return e->gb(e->rref,nullptr,0,0);
  }catch(...){PluginFault("dataref length fault in plugin accessor: "+e->name);}
  return (e->types&(xplmType_FloatArray|xplmType_IntArray|xplmType_Data))?0:1;
 }
 if(e->types&xplmType_FloatArray)return static_cast<int>(e->fa.size());
 if(e->types&xplmType_IntArray)return static_cast<int>(e->ia.size());
 if(e->types&xplmType_Data)return static_cast<int>(e->bytes.size());
 return 1;
}
double GetScalarE(Entry* e){if(!e)return 0;if(e->owned){++g_plug_r;return OwnedScalar(e);}++g_sim_r;return e->scalar;}
void SetScalarE(Entry* e,double v){if(!e)return;if(e->owned){++g_plug_w;OwnedSetScalar(e,v);return;}++g_sim_w;e->scalar=v;}
int GetFloatsE(Entry* e,float* out,int off,int count)noexcept{
 if(!e)return 0;
 if(e->owned){++g_plug_r;try{
   if(e->gvf)return e->gvf(e->rref,out,off,count);
   if(e->gvi){if(!out)return e->gvi(e->rref,nullptr,0,0);std::vector<int> t(static_cast<std::size_t>(std::max(count,0)));const int n=e->gvi(e->rref,t.data(),off,count);for(int i=0;i<n;++i)out[i]=static_cast<float>(t[static_cast<std::size_t>(i)]);return n;}
   if(!out)return 1;if(off==0&&count>0){out[0]=static_cast<float>(OwnedScalar(e));return 1;}return 0;
  }catch(...){PluginFault("dataref array read fault in plugin accessor: "+e->name);}return 0;}
 ++g_sim_r;const auto& a=e->fa;if(!out)return static_cast<int>(a.size());
 if(off<0||count<=0)return 0;const int n=std::min(count,std::max(0,static_cast<int>(a.size())-off));
 for(int i=0;i<n;++i)out[i]=a[static_cast<std::size_t>(off+i)];return n;
}
int GetIntsE(Entry* e,int* out,int off,int count)noexcept{
 if(!e)return 0;
 if(e->owned){++g_plug_r;try{
   if(e->gvi)return e->gvi(e->rref,out,off,count);
   if(e->gvf){if(!out)return e->gvf(e->rref,nullptr,0,0);std::vector<float> t(static_cast<std::size_t>(std::max(count,0)));const int n=e->gvf(e->rref,t.data(),off,count);for(int i=0;i<n;++i)out[i]=static_cast<int>(t[static_cast<std::size_t>(i)]);return n;}
   if(!out)return 1;if(off==0&&count>0){out[0]=static_cast<int>(OwnedScalar(e));return 1;}return 0;
  }catch(...){PluginFault("dataref array read fault in plugin accessor: "+e->name);}return 0;}
 ++g_sim_r;const auto& a=e->ia;if(!out)return static_cast<int>(a.size());
 if(off<0||count<=0)return 0;const int n=std::min(count,std::max(0,static_cast<int>(a.size())-off));
 for(int i=0;i<n;++i)out[i]=a[static_cast<std::size_t>(off+i)];return n;
}
void SetFloatsE(Entry* e,const float* in,int off,int count)noexcept{
 if(!e||!in||off<0||count<=0)return;
 if(e->owned){++g_plug_w;try{
   if(e->svf){e->svf(e->wref,const_cast<float*>(in),off,count);return;}
   if(e->svi){std::vector<int> t(static_cast<std::size_t>(count));for(int i=0;i<count;++i)t[static_cast<std::size_t>(i)]=static_cast<int>(in[i]);e->svi(e->wref,t.data(),off,count);return;}
   if(off==0)OwnedSetScalar(e,in[0]);
  }catch(...){PluginFault("dataref array write fault in plugin accessor: "+e->name);}return;}
 ++g_sim_w;if(static_cast<std::size_t>(off+count)>e->fa.size())e->fa.resize(static_cast<std::size_t>(off+count),0.f);
 for(int i=0;i<count;++i)e->fa[static_cast<std::size_t>(off+i)]=in[i];
}
void SetIntsE(Entry* e,const int* in,int off,int count)noexcept{
 if(!e||!in||off<0||count<=0)return;
 if(e->owned){++g_plug_w;try{
   if(e->svi){e->svi(e->wref,const_cast<int*>(in),off,count);return;}
   if(e->svf){std::vector<float> t(static_cast<std::size_t>(count));for(int i=0;i<count;++i)t[static_cast<std::size_t>(i)]=static_cast<float>(in[i]);e->svf(e->wref,t.data(),off,count);return;}
   if(off==0)OwnedSetScalar(e,in[0]);
  }catch(...){PluginFault("dataref array write fault in plugin accessor: "+e->name);}return;}
 ++g_sim_w;if(static_cast<std::size_t>(off+count)>e->ia.size())e->ia.resize(static_cast<std::size_t>(off+count),0);
 for(int i=0;i<count;++i)e->ia[static_cast<std::size_t>(off+i)]=in[i];
}
int GetBytesE(Entry* e,void* out,int off,int count)noexcept{
 if(!e)return 0;
 if(e->owned){++g_plug_r;try{if(e->gb)return e->gb(e->rref,out,off,count);}catch(...){PluginFault("dataref byte read fault in plugin accessor: "+e->name);}return 0;}
 ++g_sim_r;const auto& a=e->bytes;if(!out)return static_cast<int>(a.size());
 if(off<0||count<=0)return 0;const int n=std::min(count,std::max(0,static_cast<int>(a.size())-off));
 if(n>0)std::memcpy(out,a.data()+off,static_cast<std::size_t>(n));return n;
}
void SetBytesE(Entry* e,const void* in,int off,int count)noexcept{
 if(!e||!in||off<0||count<=0)return;
 if(e->owned){++g_plug_w;try{if(e->sb)e->sb(e->wref,const_cast<void*>(in),off,count);}catch(...){PluginFault("dataref byte write fault in plugin accessor: "+e->name);}return;}
 ++g_sim_w;if(static_cast<std::size_t>(off+count)>e->bytes.size())e->bytes.resize(static_cast<std::size_t>(off+count),0);
 std::memcpy(e->bytes.data()+off,in,static_cast<std::size_t>(count));
}
// Seeding helpers (entry created if DataRefs.txt lacked it).
Entry* Ensure(const char* name,int types){if(auto* e=FindEntry(name,false))return e;return NewEntry(name,types,true,false);}
void SeedD(const char* n,double v){auto* e=Ensure(n,xplmType_Double|xplmType_Float);e->scalar=v;}
void SeedF(const char* n,double v){auto* e=Ensure(n,xplmType_Float|xplmType_Double);e->scalar=v;}
void SeedI(const char* n,int v){auto* e=Ensure(n,xplmType_Int);e->scalar=v;}
void SeedFA(const char* n,int idx,float v){auto* e=Ensure(n,xplmType_FloatArray);if(static_cast<std::size_t>(idx)>=e->fa.size())e->fa.resize(static_cast<std::size_t>(idx+1),0.f);e->fa[static_cast<std::size_t>(idx)]=v;}
void SeedIA(const char* n,int idx,int v){auto* e=Ensure(n,xplmType_IntArray);if(static_cast<std::size_t>(idx)>=e->ia.size())e->ia.resize(static_cast<std::size_t>(idx+1),0);e->ia[static_cast<std::size_t>(idx)]=v;}
void SeedStr(const char* n,const char* v){auto* e=Ensure(n,xplmType_Data);const std::size_t len=std::strlen(v)+1;if(e->bytes.size()<len)e->bytes.resize(len,0);std::memcpy(e->bytes.data(),v,len);}
double IsaTemp(double h){return 15.-.0065*h;}
double IsaPressure(double h){return h<11000.?101325.*std::pow(1.-2.25577e-5*h,5.2559):22632.;}
// ---- commands ----------------------------------------------------------------------------------
struct Handler { XPLMCommandCallback_f fn;int before;void* ref; };
struct Cmd { std::string name,desc;std::vector<Handler> handlers;bool down=false; };
std::map<std::string,Cmd*> g_cmds;std::vector<Cmd*> g_cmd_order;
Cmd* NewCmd(const std::string& name,const std::string& desc){
 auto it=g_cmds.find(name);if(it!=g_cmds.end()){if(it->second->desc.empty())it->second->desc=desc;return it->second;}
 auto* c=new Cmd();c->name=name;c->desc=desc;g_cmds[name]=c;g_cmd_order.push_back(c);return c;
}
Cmd* AsCmd(XPLMCommandRef r){return static_cast<Cmd*>(r);}
int CallHandler(XPLMCommandCallback_f fn,XPLMCommandRef c,XPLMCommandPhase p,void* ref)noexcept{
 try{return fn(c,p,ref);}catch(...){PluginFault("command handler fault in plugin");return 0;}
}
void SetSimScalar(const char* name,double v){if(auto* e=FindEntry(name,false))e->scalar=v;}
double GetSimScalar(const char* name){auto* e=FindEntry(name,false);return e?e->scalar:0.;}
bool g_reset_pending=false;
void SimDefault(Cmd* c,int phase){
 if(phase!=xplm_CommandBegin)return;
 if(c->name=="sim/operation/pause_on")SetSimScalar("sim/time/paused",1);
 else if(c->name=="sim/operation/pause_off")SetSimScalar("sim/time/paused",0);
 else if(c->name=="sim/operation/pause_toggle")SetSimScalar("sim/time/paused",GetSimScalar("sim/time/paused")?0:1);
 else if(c->name=="sim/operation/reset_flight")g_reset_pending=true;
}
void Dispatch(Cmd* c,int phase){
 if(!c)return;++g_cmd_fired;
 if(g_cmd_fn)g_cmd_fn(c->name.c_str(),phase,g_now,g_cmd_ref);
 // XPLMCommandCallback_f: 1 continues processing, 0 consumes the command.
 for(const auto& h:c->handlers)if(h.before&&!CallHandler(h.fn,c,static_cast<XPLMCommandPhase>(phase),h.ref))return;
 SimDefault(c,phase);
 for(const auto& h:c->handlers)if(!h.before&&!CallHandler(h.fn,c,static_cast<XPLMCommandPhase>(phase),h.ref))return;
}
void FireOnce(Cmd* c){if(!c)return;Dispatch(c,xplm_CommandBegin);Dispatch(c,xplm_CommandEnd);
 if(g_reset_pending){g_reset_pending=false;if(g_msg_fn)g_msg_fn(XPLM_MSG_AIRPORT_LOADED,nullptr,g_msg_ref);}}
void FireBegin(Cmd* c){if(!c||c->down)return;c->down=true;Dispatch(c,xplm_CommandBegin);}
void FireEnd(Cmd* c){if(!c||!c->down)return;Dispatch(c,xplm_CommandEnd);c->down=false;
 if(g_reset_pending){g_reset_pending=false;if(g_msg_fn)g_msg_fn(XPLM_MSG_AIRPORT_LOADED,nullptr,g_msg_ref);}}
// ---- flight loops --------------------------------------------------------------------------------
struct Loop { XPLMFlightLoop_f cb=nullptr;void* ref=nullptr;int phase=0;bool dead=false,scheduled=false,called=false;
 double due_time=0,last_call=0;long long due_frame=-1; };
std::vector<Loop*> g_loops;
void ScheduleLoop(Loop* l,float interval){
 if(interval==0.f){l->scheduled=false;return;}
 l->scheduled=true;
 if(interval>0.f){l->due_time=g_now+interval;l->due_frame=-1;}
 else{l->due_frame=g_frame+std::max(1LL,static_cast<long long>(-interval));}
}
float CallLoopCb(XPLMFlightLoop_f cb,float a,float b,int c,void* r,unsigned* code){
 __try{return cb(a,b,c,r);}__except(EXCEPTION_EXECUTE_HANDLER){*code=GetExceptionCode();return 0.f;}
}
void DispatchLoops(int phase){
 const std::size_t n=g_loops.size();
 for(std::size_t i=0;i<n;++i){Loop* l=g_loops[i];
  if(!l||l->dead||!l->scheduled||l->phase!=phase)continue;
  const bool due=l->due_frame>=0?g_frame>=l->due_frame:g_now>=l->due_time-1e-9;
  if(!due)continue;
  const float since=static_cast<float>(l->called?g_now-l->last_call:g_dt);
  unsigned code=0;const float next=CallLoopCb(l->cb,since,static_cast<float>(g_dt),static_cast<int>(g_frame),l->ref,&code);
  ++g_loops_run;l->called=true;l->last_call=g_now;
  if(code){g_faulted=true;l->scheduled=false;char buf[96];std::snprintf(buf,sizeof buf,"flight loop fault (SEH 0x%08x); loop descheduled",code);LogOnce(buf);continue;}
  if(!l->dead)ScheduleLoop(l,next);
 }
}
// Host "flight model" refresh: derive geo datarefs from the local pose the plugin publishes.
double g_last_lx=1e30,g_last_ly=0,g_last_lz=0;
void LocalToWorldImpl(double x,double y,double z,double* lat,double* lon,double* alt);
void FmRefresh(){
 auto* ex=FindEntry("sim/flightmodel/position/local_x",false);
 auto* ey=FindEntry("sim/flightmodel/position/local_y",false);
 auto* ez=FindEntry("sim/flightmodel/position/local_z",false);
 if(!ex||!ey||!ez)return;
 const double x=ex->scalar,y=ey->scalar,z=ez->scalar;
 { // Cockpit indicators X-Plane derives from the flightmodel engine values the plugin publishes.
  auto* tac=FindEntry("sim/flightmodel/engine/ENGN_tacrad",false);
  auto* rpm=FindEntry("sim/cockpit2/engine/indicators/engine_speed_rpm",false);
  if(tac&&rpm&&!tac->fa.empty()&&!rpm->fa.empty())rpm->fa[0]=static_cast<float>(tac->fa[0]*9.54929658551372);
  auto* op=FindEntry("sim/flightmodel/engine/ENGN_oil_press_psi",false);
  auto* opi=FindEntry("sim/cockpit2/engine/indicators/oil_pressure_psi",false);
  if(op&&opi&&!op->fa.empty()&&!opi->fa.empty())opi->fa[0]=op->fa[0];
  auto* mp=FindEntry("sim/flightmodel/engine/ENGN_MPR",false);
  auto* mpi=FindEntry("sim/cockpit2/engine/indicators/MPR_in_hg",false);
  if(mp&&mpi&&!mp->fa.empty()&&!mpi->fa.empty())mpi->fa[0]=mp->fa[0];
 }
 if(x==g_last_lx&&y==g_last_ly&&z==g_last_lz)return;
 g_last_lx=x;g_last_ly=y;g_last_lz=z;
 double lat=0,lon=0,alt=0;LocalToWorldImpl(x,y,z,&lat,&lon,&alt);
 SetSimScalar("sim/flightmodel/position/latitude",lat);
 SetSimScalar("sim/flightmodel/position/longitude",lon);
 SetSimScalar("sim/flightmodel/position/elevation",alt);
 SetSimScalar("sim/flightmodel/position/y_agl",y);
 const double vx=GetSimScalar("sim/flightmodel/position/local_vx"),vz=GetSimScalar("sim/flightmodel/position/local_vz");
 SetSimScalar("sim/flightmodel/position/groundspeed",std::sqrt(vx*vx+vz*vz));
}
// ---- nav mini database ---------------------------------------------------------------------------
struct Nav { int type=0;float lat=0,lon=0,height=0,heading=0;int freq=0;std::string id,name,reg; };
std::vector<Nav> g_navs;
struct Plan { std::vector<XPLMNavRef> entries;std::vector<int> alts;int displayed=-1,direct=-1; };
std::array<Plan,6> g_plans;
Plan* GetPlan(XPLMNavFlightPlan p){const int i=static_cast<int>(p);return i>=0&&i<6?&g_plans[static_cast<std::size_t>(i)]:nullptr;}
// ---- file seeding ----------------------------------------------------------------------------------
std::vector<std::string> SplitTabs(const std::string& s){std::vector<std::string> o;std::size_t a=0;for(;;){const auto b=s.find('\t',a);if(b==std::string::npos){o.push_back(s.substr(a));break;}o.push_back(s.substr(a,b-a));a=b+1;}return o;}
int LoadDataRefs(const char* path){
 if(!path)return 0;FILE* f=nullptr;if(fopen_s(&f,path,"rb")||!f)return -1;
 std::string all;char buf[65536];std::size_t n;while((n=std::fread(buf,1,sizeof buf,f))>0)all.append(buf,n);std::fclose(f);
 int count=0;std::size_t a=0;
 while(a<all.size()){const auto b=all.find('\n',a);std::string line=Trim(all.substr(a,b==std::string::npos?std::string::npos:b-a));a=b==std::string::npos?all.size():b+1;
  if(line.empty()||line.find('/')==std::string::npos)continue;
  const auto parts=SplitTabs(line);if(parts.size()<3)continue;
  const std::string name=Trim(parts[0]),type=Trim(parts[1]);if(name.empty()||g_by_name.count(name))continue;
  const bool writable=Trim(parts[2])=="y";
  int len=1;std::string base=type;
  if(const auto lb=type.find('[');lb!=std::string::npos){base=type.substr(0,lb);len=std::max(1,std::atoi(type.c_str()+lb+1));}
  Entry* e=nullptr;
  if(base=="float"&&len==1)e=NewEntry(name,xplmType_Float|xplmType_Double,writable,false);
  else if(base=="double")e=NewEntry(name,xplmType_Double|xplmType_Float,writable,false);
  else if(base=="int"&&len==1)e=NewEntry(name,xplmType_Int,writable,false);
  else if(base=="float"){e=NewEntry(name,xplmType_FloatArray,writable,false);e->fa.assign(static_cast<std::size_t>(len),0.f);}
  else if(base=="int"){e=NewEntry(name,xplmType_IntArray,writable,false);e->ia.assign(static_cast<std::size_t>(len),0);}
  else if(base=="byte"){e=NewEntry(name,xplmType_Data,writable,false);e->bytes.assign(static_cast<std::size_t>(len),0);}
  else continue;
  ++count;
 }
 return count;
}
int LoadCommands(const char* path){
 if(!path)return 0;FILE* f=nullptr;if(fopen_s(&f,path,"rb")||!f)return -1;
 std::string all;char buf[65536];std::size_t n;while((n=std::fread(buf,1,sizeof buf,f))>0)all.append(buf,n);std::fclose(f);
 int count=0;std::size_t a=0;
 while(a<all.size()){const auto b=all.find('\n',a);std::string line=all.substr(a,b==std::string::npos?std::string::npos:b-a);a=b==std::string::npos?all.size():b+1;
  if(!line.empty()&&line.back()=='\r')line.pop_back();
  const auto sp=line.find_first_of(" \t");if(sp==std::string::npos)continue;
  const std::string name=line.substr(0,sp);if(name.find("sim/")!=0)continue;
  NewCmd(name,Trim(line.substr(sp)));++count;
 }
 return count;
}
void SeedParked(double psi_true){
 // Generic parked state at the configured origin: on the ground, engine off, ISA, battery off.
 SeedD("sim/flightmodel/position/latitude",g_lat0);SeedD("sim/flightmodel/position/longitude",g_lon0);
 SeedD("sim/flightmodel/position/elevation",g_elev0+1.);
 SeedD("sim/flightmodel/position/local_x",0.);SeedD("sim/flightmodel/position/local_y",1.);SeedD("sim/flightmodel/position/local_z",0.);
 SeedF("sim/flightmodel/position/y_agl",1.);SeedF("sim/flightmodel/position/groundspeed",0.);
 SeedF("sim/flightmodel/position/theta",0.);SeedF("sim/flightmodel/position/phi",0.);SeedF("sim/flightmodel/position/psi",psi_true);
 SeedF("sim/flightmodel/position/mag_psi",psi_true+g_magvar);
 for(const char* n:{"local_vx","local_vy","local_vz","local_ax","local_ay","local_az","P","Q","R","Prad","Qrad","Rrad","vh_ind","hpath","vpath","true_airspeed","groundspeed"})SeedF((std::string("sim/flightmodel/position/")+n).c_str(),0.);
 SeedI("sim/flightmodel/failures/onground_any",1);SeedI("sim/flightmodel/failures/onground_all",1);
 SeedI("sim/flightmodel/ground/surface_texture_type",3);  // asphalt
 const double t=IsaTemp(g_elev0),p=IsaPressure(g_elev0);
 SeedF("sim/weather/aircraft/temperature_ambient_deg_c",t);SeedF("sim/weather/aircraft/barometer_current_pas",p);
 SeedF("sim/weather/aircraft/visibility_reported_sm",20.);
 for(const char* n:{"wind_now_x_msc","wind_now_y_msc","wind_now_z_msc","wind_now_direction_degt","wind_now_speed_msc","precipitation_on_aircraft_ratio","rain_on_aircraft_ratio","snow_on_aircraft_ratio","hail_on_aircraft_ratio"})SeedF((std::string("sim/weather/aircraft/")+n).c_str(),0.);
 SeedF("sim/weather/region/runway_friction",0.);
 SeedI("sim/time/paused",0);SeedI("sim/time/is_in_replay",0);SeedI("sim/time/sim_speed",1);SeedI("sim/time/ground_speed",1);
 SeedF("sim/time/sim_speed_actual",1.);SeedF("sim/time/sim_speed_actual_ogl",1.);
 SeedF("sim/time/total_running_time_sec",0.);SeedF("sim/time/total_flight_time_sec",0.);
 SeedF("sim/time/local_time_sec",43200.);SeedF("sim/time/zulu_time_sec",43200.);
 SeedI("sim/time/local_date_days",302);SeedI("sim/time/zulu_date_days",302);
 SeedF("sim/operation/misc/frame_rate_period",g_dt);SeedF("sim/time/framerate_period",g_dt);
 SeedF("sim/flightmodel/weight/m_total",1200.);
 SeedStr("sim/aircraft/view/acf_tailnum","XPHOST");
 SeedI("sim/graphics/view/view_type",1026);SeedI("sim/graphics/view/view_is_external",0);
 SeedI("sim/graphics/VR/enabled",0);SeedI("sim/graphics/view/hide_yoke",0);
 SeedF("sim/graphics/scenery/sun_pitch_degrees",45.);SeedF("sim/graphics/view/field_of_view_deg",60.);
 for(int i=0;i<16;++i){SeedFA("sim/graphics/view/world_matrix",i,i%5==0?1.f:0.f);SeedFA("sim/graphics/view/projection_matrix",i,i%5==0?1.f:0.f);}
 SeedI("sim/operation/sound/sound_on",1);
}
void LocalToWorldImpl(double x,double y,double z,double* lat,double* lon,double* alt){
 constexpr double kR=6371000.,kDeg=3.141592653589793/180.;
 *lat=g_lat0-z/(kR*kDeg);*lon=g_lon0+x/(kR*kDeg*std::cos(g_lat0*kDeg));*alt=g_elev0+y;
}
void WorldToLocalImpl(double lat,double lon,double alt,double* x,double* y,double* z){
 constexpr double kR=6371000.,kDeg=3.141592653589793/180.;
 *x=(lon-g_lon0)*kR*kDeg*std::cos(g_lat0*kDeg);*y=alt-g_elev0;*z=-(lat-g_lat0)*kR*kDeg;
}
}  // namespace
// ---- XPLMDataAccess ---------------------------------------------------------------------------------
extern "C" {
XPLM_API XPLMDataRef XPLMFindDataRef(const char* name){std::lock_guard<std::recursive_mutex> g(g_mu);return FindEntry(name,true);}
XPLM_API int XPLMIsDataRefGood(XPLMDataRef r){std::lock_guard<std::recursive_mutex> g(g_mu);auto* e=AsEntry(r);return e&&e->alive?1:0;}
XPLM_API XPLMDataTypeID XPLMGetDataRefTypes(XPLMDataRef r){std::lock_guard<std::recursive_mutex> g(g_mu);auto* e=AsEntry(r);return e?static_cast<XPLMDataTypeID>(e->types):xplmType_Unknown;}
XPLM_API int XPLMCanWriteDataRef(XPLMDataRef r){std::lock_guard<std::recursive_mutex> g(g_mu);auto* e=AsEntry(r);return e&&e->writable?1:0;}
XPLM_API int XPLMGetDatai(XPLMDataRef r){std::lock_guard<std::recursive_mutex> g(g_mu);return static_cast<int>(GetScalarE(AsEntry(r)));}
XPLM_API float XPLMGetDataf(XPLMDataRef r){std::lock_guard<std::recursive_mutex> g(g_mu);return static_cast<float>(GetScalarE(AsEntry(r)));}
XPLM_API double XPLMGetDatad(XPLMDataRef r){std::lock_guard<std::recursive_mutex> g(g_mu);return GetScalarE(AsEntry(r));}
XPLM_API void XPLMSetDatai(XPLMDataRef r,int v){std::lock_guard<std::recursive_mutex> g(g_mu);SetScalarE(AsEntry(r),v);}
XPLM_API void XPLMSetDataf(XPLMDataRef r,float v){std::lock_guard<std::recursive_mutex> g(g_mu);SetScalarE(AsEntry(r),v);}
XPLM_API void XPLMSetDatad(XPLMDataRef r,double v){std::lock_guard<std::recursive_mutex> g(g_mu);SetScalarE(AsEntry(r),v);}
XPLM_API int XPLMGetDatavi(XPLMDataRef r,int* out,int off,int max){std::lock_guard<std::recursive_mutex> g(g_mu);return GetIntsE(AsEntry(r),out,off,max);}
XPLM_API int XPLMGetDatavf(XPLMDataRef r,float* out,int off,int max){std::lock_guard<std::recursive_mutex> g(g_mu);return GetFloatsE(AsEntry(r),out,off,max);}
XPLM_API int XPLMGetDatab(XPLMDataRef r,void* out,int off,int max){std::lock_guard<std::recursive_mutex> g(g_mu);return GetBytesE(AsEntry(r),out,off,max);}
XPLM_API void XPLMSetDatavi(XPLMDataRef r,int* in,int off,int count){std::lock_guard<std::recursive_mutex> g(g_mu);SetIntsE(AsEntry(r),in,off,count);}
XPLM_API void XPLMSetDatavf(XPLMDataRef r,float* in,int off,int count){std::lock_guard<std::recursive_mutex> g(g_mu);SetFloatsE(AsEntry(r),in,off,count);}
XPLM_API void XPLMSetDatab(XPLMDataRef r,void* in,int off,int count){std::lock_guard<std::recursive_mutex> g(g_mu);SetBytesE(AsEntry(r),in,off,count);}
XPLM_API XPLMDataRef XPLMRegisterDataAccessor(const char* name,XPLMDataTypeID types,int writable,XPLMGetDatai_f gi,XPLMSetDatai_f si,XPLMGetDataf_f gf,XPLMSetDataf_f sf,XPLMGetDatad_f gd,XPLMSetDatad_f sd,XPLMGetDatavi_f gvi,XPLMSetDatavi_f svi,XPLMGetDatavf_f gvf,XPLMSetDatavf_f svf,XPLMGetDatab_f gb,XPLMSetDatab_f sb,void* rref,void* wref){
 std::lock_guard<std::recursive_mutex> g(g_mu);if(!name||!*name)return nullptr;
 Entry* e=nullptr;auto it=g_by_name.find(name);
 if(it!=g_by_name.end()){if(!it->second->owned||it->second->alive)return nullptr;e=it->second;}
 else e=NewEntry(name,static_cast<int>(types),writable!=0,true);
 e->types=static_cast<int>(types);e->writable=writable!=0;e->owned=true;e->alive=true;
 e->gi=gi;e->si=si;e->gf=gf;e->sf=sf;e->gd=gd;e->sd=sd;e->gvi=gvi;e->svi=svi;e->gvf=gvf;e->svf=svf;e->gb=gb;e->sb=sb;e->rref=rref;e->wref=wref;
 return e;
}
XPLM_API void XPLMUnregisterDataAccessor(XPLMDataRef r){std::lock_guard<std::recursive_mutex> g(g_mu);auto* e=AsEntry(r);if(e&&e->owned){e->alive=false;e->gi=nullptr;e->si=nullptr;e->gf=nullptr;e->sf=nullptr;e->gd=nullptr;e->sd=nullptr;e->gvi=nullptr;e->svi=nullptr;e->gvf=nullptr;e->svf=nullptr;e->gb=nullptr;e->sb=nullptr;}}
// ---- XPLMProcessing -----------------------------------------------------------------------------------
XPLM_API float XPLMGetElapsedTime(){std::lock_guard<std::recursive_mutex> g(g_mu);return static_cast<float>(g_now);}
XPLM_API int XPLMGetCycleNumber(){std::lock_guard<std::recursive_mutex> g(g_mu);return static_cast<int>(g_frame);}
XPLM_API void XPLMRegisterFlightLoopCallback(XPLMFlightLoop_f cb,float interval,void* ref){
 std::lock_guard<std::recursive_mutex> g(g_mu);auto* l=new Loop();l->cb=cb;l->ref=ref;l->phase=xplm_FlightLoop_Phase_BeforeFlightModel;g_loops.push_back(l);ScheduleLoop(l,interval);
}
XPLM_API void XPLMUnregisterFlightLoopCallback(XPLMFlightLoop_f cb,void* ref){
 std::lock_guard<std::recursive_mutex> g(g_mu);for(auto* l:g_loops)if(l&&l->cb==cb&&l->ref==ref)l->dead=true;
}
XPLM_API XPLMFlightLoopID XPLMCreateFlightLoop(XPLMCreateFlightLoop_t* p){
 std::lock_guard<std::recursive_mutex> g(g_mu);if(!p||p->structSize<static_cast<int>(sizeof(int)))return nullptr;
 auto* l=new Loop();l->cb=p->callbackFunc;l->ref=p->refcon;l->phase=static_cast<int>(p->phase);g_loops.push_back(l);return static_cast<XPLMFlightLoopID>(l);
}
XPLM_API void XPLMDestroyFlightLoop(XPLMFlightLoopID id){std::lock_guard<std::recursive_mutex> g(g_mu);auto* l=static_cast<Loop*>(id);if(l){l->dead=true;l->scheduled=false;}}
XPLM_API void XPLMScheduleFlightLoop(XPLMFlightLoopID id,float interval,int){std::lock_guard<std::recursive_mutex> g(g_mu);auto* l=static_cast<Loop*>(id);if(l&&!l->dead)ScheduleLoop(l,interval);}
// ---- XPLMUtilities (commands, debug, paths) ------------------------------------------------------------
XPLM_API void XPLMDebugString(const char* s){
 std::lock_guard<std::recursive_mutex> g(g_mu);if(!s)return;LogRaw(s);
}
XPLM_API void XPLMGetSystemPath(char* out){std::lock_guard<std::recursive_mutex> g(g_mu);if(out)std::strncpy(out,g_system_path.c_str(),511),out[511]=0;}
XPLM_API XPLMCommandRef XPLMFindCommand(const char* name){std::lock_guard<std::recursive_mutex> g(g_mu);if(!name)return nullptr;return NewCmd(name,"");}
XPLM_API XPLMCommandRef XPLMCreateCommand(const char* name,const char* desc){std::lock_guard<std::recursive_mutex> g(g_mu);if(!name)return nullptr;return NewCmd(name,desc?desc:"");}
XPLM_API void XPLMRegisterCommandHandler(XPLMCommandRef c,XPLMCommandCallback_f fn,int before,void* ref){
 std::lock_guard<std::recursive_mutex> g(g_mu);auto* cmd=AsCmd(c);if(cmd&&fn)cmd->handlers.push_back({fn,before,ref});
}
XPLM_API void XPLMUnregisterCommandHandler(XPLMCommandRef c,XPLMCommandCallback_f fn,int before,void* ref){
 std::lock_guard<std::recursive_mutex> g(g_mu);auto* cmd=AsCmd(c);if(!cmd)return;
 cmd->handlers.erase(std::remove_if(cmd->handlers.begin(),cmd->handlers.end(),[&](const Handler& h){return h.fn==fn&&h.before==before&&h.ref==ref;}),cmd->handlers.end());
}
XPLM_API void XPLMCommandBegin(XPLMCommandRef c){std::lock_guard<std::recursive_mutex> g(g_mu);FireBegin(AsCmd(c));}
XPLM_API void XPLMCommandEnd(XPLMCommandRef c){std::lock_guard<std::recursive_mutex> g(g_mu);FireEnd(AsCmd(c));}
XPLM_API void XPLMCommandOnce(XPLMCommandRef c){std::lock_guard<std::recursive_mutex> g(g_mu);FireOnce(AsCmd(c));}
XPLM_API void XPLMGetVersions(int* xplm,int* xplane,XPLMHostApplicationID* host){if(xplm)*xplm=411;if(xplane)*xplane=12100;if(host)*host=xplm_Host_XPlane;}
// ---- XPLMPlugin ------------------------------------------------------------------------------------------
XPLM_API XPLMPluginID XPLMGetMyID(){return 1;}
XPLM_API void XPLMGetPluginInfo(XPLMPluginID id,char* name,char* path,char* sig,char* desc){
 std::lock_guard<std::recursive_mutex> g(g_mu);
 auto put=[](char* out,const std::string& v,std::size_t cap){if(out){std::strncpy(out,v.c_str(),cap-1);out[cap-1]=0;}};
 if(id==XPLM_PLUGIN_XPLANE){put(name,"X-Plane",256);put(path,"",512);put(sig,"xplanesdk.simulator",256);put(desc,"xphost fake simulator",256);return;}
 if(id!=1){put(name,"",256);put(path,"",512);put(sig,"",256);put(desc,"",256);return;}
 put(name,g_self_name,256);put(path,g_plugin_path,512);put(sig,g_self_sig,256);put(desc,g_self_desc,256);
}
XPLM_API XPLMPluginID XPLMFindPluginBySignature(const char* sig){
 std::lock_guard<std::recursive_mutex> g(g_mu);
 if(sig&&!g_self_sig.empty()&&g_self_sig==sig)return 1;
 if(sig&&std::strcmp(sig,"xplanesdk.simulator")==0)return XPLM_PLUGIN_XPLANE;
 return XPLM_NO_PLUGIN_ID;
}
XPLM_API int XPLMIsPluginEnabled(XPLMPluginID id){std::lock_guard<std::recursive_mutex> g(g_mu);return id==1?g_enabled:id==XPLM_PLUGIN_XPLANE?1:0;}
// ---- XPLMPlanes ---------------------------------------------------------------------------------------------
XPLM_API void XPLMGetNthAircraftModel(int index,char* file,char* path){
 std::lock_guard<std::recursive_mutex> g(g_mu);
 if(file){std::strncpy(file,index==0?g_acf_name.c_str():"",255);file[255]=0;}
 if(path){std::strncpy(path,index==0?g_acf_path.c_str():"",511);path[511]=0;}
}
// ---- XPLMMenus (stubs with real indices) ---------------------------------------------------------------------
struct Menu { std::string name;XPLMMenuHandler_f handler=nullptr;void* ref=nullptr;std::vector<std::string> items;bool dead=false; };
Menu* g_aircraft_menu=nullptr;Menu* g_plugins_menu=nullptr;
Menu* AsMenu(XPLMMenuID m){return static_cast<Menu*>(m);}
XPLM_API XPLMMenuID XPLMFindPluginsMenu(){std::lock_guard<std::recursive_mutex> g(g_mu);if(!g_plugins_menu)g_plugins_menu=new Menu{"Plugins"};return g_plugins_menu;}
XPLM_API XPLMMenuID XPLMFindAircraftMenu(){std::lock_guard<std::recursive_mutex> g(g_mu);if(!g_aircraft_menu)g_aircraft_menu=new Menu{"Aircraft"};return g_aircraft_menu;}
XPLM_API XPLMMenuID XPLMCreateMenu(const char* name,XPLMMenuID,int,XPLMMenuHandler_f h,void* ref){
 std::lock_guard<std::recursive_mutex> g(g_mu);auto* m=new Menu();m->name=name?name:"";m->handler=h;m->ref=ref;return m;
}
XPLM_API void XPLMDestroyMenu(XPLMMenuID id){std::lock_guard<std::recursive_mutex> g(g_mu);auto* m=AsMenu(id);if(m)m->dead=true;}
XPLM_API void XPLMClearAllMenuItems(XPLMMenuID id){std::lock_guard<std::recursive_mutex> g(g_mu);auto* m=AsMenu(id);if(m)m->items.clear();}
XPLM_API int XPLMAppendMenuItem(XPLMMenuID id,const char* name,void*,int){
 std::lock_guard<std::recursive_mutex> g(g_mu);auto* m=AsMenu(id);if(!m||m->dead)return -1;m->items.push_back(name?name:"");return static_cast<int>(m->items.size())-1;
}
XPLM_API int XPLMAppendMenuItemWithCommand(XPLMMenuID id,const char* name,XPLMCommandRef){
 std::lock_guard<std::recursive_mutex> g(g_mu);auto* m=AsMenu(id);if(!m||m->dead)return -1;m->items.push_back(name?name:"");return static_cast<int>(m->items.size())-1;
}
XPLM_API void XPLMAppendMenuSeparator(XPLMMenuID id){std::lock_guard<std::recursive_mutex> g(g_mu);auto* m=AsMenu(id);if(m)m->items.push_back("-");}
XPLM_API void XPLMRemoveMenuItem(XPLMMenuID id,int index){
 std::lock_guard<std::recursive_mutex> g(g_mu);auto* m=AsMenu(id);if(m&&index>=0&&index<static_cast<int>(m->items.size()))m->items.erase(m->items.begin()+index);
}
XPLM_API void XPLMSetMenuItemName(XPLMMenuID id,int index,const char* name,int){
 std::lock_guard<std::recursive_mutex> g(g_mu);auto* m=AsMenu(id);if(m&&index>=0&&index<static_cast<int>(m->items.size()))m->items[static_cast<std::size_t>(index)]=name?name:"";
}
XPLM_API void XPLMCheckMenuItem(XPLMMenuID,int,XPLMMenuCheck){}
XPLM_API void XPLMEnableMenuItem(XPLMMenuID,int,int){}
// ---- XPLMDisplay (windows, avionics devices, draw callbacks; registered, never called) -------------------------
struct Win { int l=0,t=0,r=0,b=0;bool visible=false;std::string title; };
Win* AsWin(XPLMWindowID w){return static_cast<Win*>(w);}
XPLM_API XPLMWindowID XPLMCreateWindowEx(XPLMCreateWindow_t* p){
 std::lock_guard<std::recursive_mutex> g(g_mu);if(!p)return nullptr;
 auto* w=new Win();w->l=p->left;w->t=p->top;w->r=p->right;w->b=p->bottom;w->visible=p->visible!=0;return w;
}
XPLM_API void XPLMDestroyWindow(XPLMWindowID){}
XPLM_API void XPLMGetScreenSize(int* w,int* h){if(w)*w=1920;if(h)*h=1080;}
XPLM_API void XPLMGetScreenBoundsGlobal(int* l,int* t,int* r,int* b){if(l)*l=0;if(t)*t=1080;if(r)*r=1920;if(b)*b=0;}
XPLM_API void XPLMGetAllMonitorBoundsGlobal(XPLMReceiveMonitorBoundsGlobal_f cb,void* ref){if(cb)cb(0,0,1080,1920,0,ref);}
XPLM_API void XPLMGetMouseLocationGlobal(int* x,int* y){if(x)*x=0;if(y)*y=0;}
XPLM_API void XPLMSetWindowTitle(XPLMWindowID id,const char* t){std::lock_guard<std::recursive_mutex> g(g_mu);auto* w=AsWin(id);if(w)w->title=t?t:"";}
XPLM_API void XPLMSetWindowGeometry(XPLMWindowID id,int l,int t,int r,int b){std::lock_guard<std::recursive_mutex> g(g_mu);auto* w=AsWin(id);if(w){w->l=l;w->t=t;w->r=r;w->b=b;}}
XPLM_API void XPLMGetWindowGeometry(XPLMWindowID id,int* l,int* t,int* r,int* b){
 std::lock_guard<std::recursive_mutex> g(g_mu);auto* w=AsWin(id);if(l)*l=w?w->l:0;if(t)*t=w?w->t:0;if(r)*r=w?w->r:0;if(b)*b=w?w->b:0;
}
XPLM_API void XPLMSetWindowResizingLimits(XPLMWindowID,int,int,int,int){}
XPLM_API void XPLMSetWindowPositioningMode(XPLMWindowID,XPLMWindowPositioningMode,int){}
XPLM_API void XPLMSetWindowIsVisible(XPLMWindowID id,int v){std::lock_guard<std::recursive_mutex> g(g_mu);auto* w=AsWin(id);if(w)w->visible=v!=0;}
XPLM_API int XPLMGetWindowIsVisible(XPLMWindowID id){std::lock_guard<std::recursive_mutex> g(g_mu);auto* w=AsWin(id);return w&&w->visible?1:0;}
XPLM_API int XPLMWindowIsPoppedOut(XPLMWindowID){return 0;}
XPLM_API int XPLMWindowIsInVR(XPLMWindowID){return 0;}
XPLM_API void XPLMSetWindowGeometryVR(XPLMWindowID,int,int){}
XPLM_API void XPLMBringWindowToFront(XPLMWindowID){}
struct DrawReg { XPLMDrawCallback_f cb;XPLMDrawingPhase phase;int before;void* ref; };
std::vector<DrawReg> g_draws;
XPLM_API int XPLMRegisterDrawCallback(XPLMDrawCallback_f cb,XPLMDrawingPhase phase,int before,void* ref){
 std::lock_guard<std::recursive_mutex> g(g_mu);g_draws.push_back({cb,phase,before,ref});return 1;
}
XPLM_API int XPLMUnregisterDrawCallback(XPLMDrawCallback_f cb,XPLMDrawingPhase phase,int before,void* ref){
 std::lock_guard<std::recursive_mutex> g(g_mu);
 g_draws.erase(std::remove_if(g_draws.begin(),g_draws.end(),[&](const DrawReg& d){return d.cb==cb&&d.phase==phase&&d.before==before&&d.ref==ref;}),g_draws.end());return 1;
}
struct Avi { std::string device;bool popup=false; };
Avi* AsAvi(XPLMAvionicsID a){return static_cast<Avi*>(a);}
XPLM_API XPLMAvionicsID XPLMRegisterAvionicsCallbacksEx(XPLMCustomizeAvionics_t*){std::lock_guard<std::recursive_mutex> g(g_mu);return new Avi();}
XPLM_API void XPLMUnregisterAvionicsCallbacks(XPLMAvionicsID){}
XPLM_API XPLMAvionicsID XPLMCreateAvionicsEx(XPLMCreateAvionics_t* p){
 std::lock_guard<std::recursive_mutex> g(g_mu);auto* a=new Avi();if(p&&p->deviceID)a->device=p->deviceID;return a;
}
XPLM_API void XPLMDestroyAvionics(XPLMAvionicsID){}
XPLM_API int XPLMIsAvionicsBound(XPLMAvionicsID a){return a?1:0;}
XPLM_API float XPLMGetAvionicsBusVoltsRatio(XPLMAvionicsID){
 std::lock_guard<std::recursive_mutex> g(g_mu);auto* e=FindEntry("sim/cockpit2/electrical/bus_volts",false);
 const double v=e&&!e->fa.empty()?e->fa[0]:0.;return static_cast<float>(std::max(0.,std::min(1.,v/24.)));
}
XPLM_API void XPLMSetAvionicsPopupVisible(XPLMAvionicsID id,int v){std::lock_guard<std::recursive_mutex> g(g_mu);auto* a=AsAvi(id);if(a)a->popup=v!=0;}
XPLM_API int XPLMIsAvionicsPopupVisible(XPLMAvionicsID id){std::lock_guard<std::recursive_mutex> g(g_mu);auto* a=AsAvi(id);return a&&a->popup?1:0;}
XPLM_API void XPLMAvionicsNeedsDrawing(XPLMAvionicsID){}
// ---- XPLMGraphics -------------------------------------------------------------------------------------------------
int g_next_texture=1000;
XPLM_API void XPLMGenerateTextureNumbers(int* out,int count){std::lock_guard<std::recursive_mutex> g(g_mu);if(out)for(int i=0;i<count;++i)out[i]=g_next_texture++;}
XPLM_API void XPLMBindTexture2d(int,int){}
XPLM_API void XPLMSetGraphicsState(int,int,int,int,int,int,int){}
XPLM_API void XPLMDrawTranslucentDarkBox(int,int,int,int){}
XPLM_API void XPLMDrawString(float*,int,int,const char*,int*,XPLMFontID){}
XPLM_API float XPLMMeasureString(XPLMFontID font,const char* s,int count){
 if(!s)return 0.f;const int n=count<0?static_cast<int>(std::strlen(s)):count;
 return static_cast<float>(n)*(font==xplmFont_Basic?10.f:6.f);
}
XPLM_API void XPLMWorldToLocal(double lat,double lon,double alt,double* x,double* y,double* z){
 std::lock_guard<std::recursive_mutex> g(g_mu);double a=0,b=0,c=0;WorldToLocalImpl(lat,lon,alt,&a,&b,&c);if(x)*x=a;if(y)*y=b;if(z)*z=c;
}
XPLM_API void XPLMLocalToWorld(double x,double y,double z,double* lat,double* lon,double* alt){
 std::lock_guard<std::recursive_mutex> g(g_mu);double a=0,b=0,c=0;LocalToWorldImpl(x,y,z,&a,&b,&c);if(lat)*lat=a;if(lon)*lon=b;if(alt)*alt=c;
}
// ---- XPLMScenery --------------------------------------------------------------------------------------------------
std::uintptr_t g_next_handle=100;
XPLM_API XPLMProbeRef XPLMCreateProbe(XPLMProbeType){std::lock_guard<std::recursive_mutex> g(g_mu);return reinterpret_cast<XPLMProbeRef>(++g_next_handle);}
XPLM_API void XPLMDestroyProbe(XPLMProbeRef){}
XPLM_API XPLMProbeResult XPLMProbeTerrainXYZ(XPLMProbeRef,float x,float,float z,XPLMProbeInfo_t* info){
 std::lock_guard<std::recursive_mutex> g(g_mu);
 if(info){info->locationX=x;info->locationY=0.f;info->locationZ=z;info->normalX=0.f;info->normalY=1.f;info->normalZ=0.f;
  info->velocityX=info->velocityY=info->velocityZ=0.f;info->is_wet=0;}
 return xplm_ProbeHitTerrain;
}
XPLM_API float XPLMGetMagneticVariation(double,double){std::lock_guard<std::recursive_mutex> g(g_mu);return static_cast<float>(g_magvar);}
XPLM_API float XPLMDegTrueToDegMagnetic(float t){std::lock_guard<std::recursive_mutex> g(g_mu);return t+static_cast<float>(g_magvar);}
struct Obj { std::string path; };
XPLM_API XPLMObjectRef XPLMLoadObject(const char* path){
 std::lock_guard<std::recursive_mutex> g(g_mu);
 if(!path||GetFileAttributesA(path)==INVALID_FILE_ATTRIBUTES){LogOnce(std::string("XPLMLoadObject missed: ")+(path?path:""));return nullptr;}
 return new Obj{path};
}
XPLM_API void XPLMUnloadObject(XPLMObjectRef){}
// ---- XPLMWeather ---------------------------------------------------------------------------------------------------
XPLM_API int XPLMGetWeatherAtLocation(double,double,double alt,XPLMWeatherInfo_t* out){
 std::lock_guard<std::recursive_mutex> g(g_mu);if(!out)return 0;
 const int size=out->structSize;std::memset(out,0,static_cast<std::size_t>(std::min(size,static_cast<int>(sizeof(XPLMWeatherInfo_t)))));out->structSize=size;
 out->temperature_alt=static_cast<float>(IsaTemp(alt));out->dewpoint_alt=out->temperature_alt-8.f;
 out->pressure_alt=static_cast<float>(IsaPressure(alt));out->pressure_sl=101325.f;out->visibility=32186.88f;
 return 1;
}
// ---- XPLMNavigation --------------------------------------------------------------------------------------------------
XPLM_API XPLMNavRef XPLMGetFirstNavAid(){std::lock_guard<std::recursive_mutex> g(g_mu);return g_navs.empty()?XPLM_NAV_NOT_FOUND:0;}
XPLM_API XPLMNavRef XPLMGetNextNavAid(XPLMNavRef r){std::lock_guard<std::recursive_mutex> g(g_mu);const int n=r+1;return n<static_cast<int>(g_navs.size())?n:XPLM_NAV_NOT_FOUND;}
XPLM_API XPLMNavRef XPLMFindNavAid(const char* name_frag,const char* id_frag,float* lat,float* lon,int* freq,XPLMNavType types){
 std::lock_guard<std::recursive_mutex> g(g_mu);int best=-1;double best_d=1e30;
 for(std::size_t i=0;i<g_navs.size();++i){const auto& n=g_navs[i];
  if(types!=xplm_Nav_Unknown&&!(n.type&static_cast<int>(types)))continue;
  if(id_frag&&n.id.find(id_frag)==std::string::npos)continue;
  if(name_frag&&n.name.find(name_frag)==std::string::npos)continue;
  if(freq&&*freq&&n.freq!=*freq)continue;
  const double d=lat&&lon?(n.lat-*lat)*(n.lat-*lat)+(n.lon-*lon)*(n.lon-*lon):static_cast<double>(i);
  if(d<best_d){best_d=d;best=static_cast<int>(i);}
 }
 return best<0?XPLM_NAV_NOT_FOUND:best;
}
XPLM_API void XPLMGetNavAidInfo(XPLMNavRef r,XPLMNavType* type,float* lat,float* lon,float* height,int* freq,float* heading,char* id,char* name,char* reg){
 std::lock_guard<std::recursive_mutex> g(g_mu);
 if(r<0||r>=static_cast<int>(g_navs.size())){if(type)*type=xplm_Nav_Unknown;return;}
 const auto& n=g_navs[static_cast<std::size_t>(r)];
 if(type)*type=static_cast<XPLMNavType>(n.type);if(lat)*lat=n.lat;if(lon)*lon=n.lon;if(height)*height=n.height;
 if(freq)*freq=n.freq;if(heading)*heading=n.heading;
 if(id){std::strncpy(id,n.id.c_str(),31);id[31]=0;}
 if(name){std::strncpy(name,n.name.c_str(),255);name[255]=0;}
 if(reg){std::strncpy(reg,n.reg.c_str(),31);reg[31]=0;}
}
XPLM_API XPLMNavRef XPLMGetGPSDestination(){
 std::lock_guard<std::recursive_mutex> g(g_mu);const auto& p=g_plans[static_cast<std::size_t>(xplm_Fpl_Pilot_Primary)];
 const int i=p.direct>=0?p.direct:p.displayed>=0?p.displayed:p.entries.empty()?-1:0;
 return i>=0&&i<static_cast<int>(p.entries.size())?p.entries[static_cast<std::size_t>(i)]:XPLM_NAV_NOT_FOUND;
}
XPLM_API int XPLMCountFMSFlightPlanEntries(XPLMNavFlightPlan p){std::lock_guard<std::recursive_mutex> g(g_mu);auto* plan=GetPlan(p);return plan?static_cast<int>(plan->entries.size()):0;}
XPLM_API void XPLMSetFMSFlightPlanEntryInfo(XPLMNavFlightPlan p,int index,XPLMNavRef ref,int alt){
 std::lock_guard<std::recursive_mutex> g(g_mu);auto* plan=GetPlan(p);if(!plan||index<0)return;
 if(index>=static_cast<int>(plan->entries.size())){plan->entries.resize(static_cast<std::size_t>(index+1),XPLM_NAV_NOT_FOUND);plan->alts.resize(static_cast<std::size_t>(index+1),0);}
 plan->entries[static_cast<std::size_t>(index)]=ref;plan->alts[static_cast<std::size_t>(index)]=alt;
}
XPLM_API void XPLMClearFMSFlightPlanEntry(XPLMNavFlightPlan p,int index){
 std::lock_guard<std::recursive_mutex> g(g_mu);auto* plan=GetPlan(p);if(!plan||index<0||index>=static_cast<int>(plan->entries.size()))return;
 plan->entries.erase(plan->entries.begin()+index);plan->alts.erase(plan->alts.begin()+index);
}
XPLM_API void XPLMSetDisplayedFMSFlightPlanEntry(XPLMNavFlightPlan p,int index){std::lock_guard<std::recursive_mutex> g(g_mu);auto* plan=GetPlan(p);if(plan)plan->displayed=index;}
XPLM_API void XPLMSetDirectToFMSFlightPlanEntry(XPLMNavFlightPlan p,int index){std::lock_guard<std::recursive_mutex> g(g_mu);auto* plan=GetPlan(p);if(plan)plan->direct=index;}
// ---- XPLMSound (silent) ---------------------------------------------------------------------------------------------
XPLM_API FMOD_CHANNEL* XPLMPlayPCMOnBus(void*,uint32_t,FMOD_SOUND_FORMAT,int,int,int,XPLMAudioBus,XPLMPCMComplete_f,void*){
 std::lock_guard<std::recursive_mutex> g(g_mu);return reinterpret_cast<FMOD_CHANNEL*>(++g_next_handle);
}
XPLM_API FMOD_RESULT XPLMStopAudio(FMOD_CHANNEL*){return FMOD_OK;}
XPLM_API FMOD_RESULT XPLMSetAudioPosition(FMOD_CHANNEL*,FMOD_VECTOR*,FMOD_VECTOR*){return FMOD_OK;}
XPLM_API FMOD_RESULT XPLMSetAudioPitch(FMOD_CHANNEL*,float){return FMOD_OK;}
XPLM_API FMOD_RESULT XPLMSetAudioVolume(FMOD_CHANNEL*,float){return FMOD_OK;}
XPLM_API FMOD_RESULT XPLMSetAudioCone(FMOD_CHANNEL*,float,float,float,FMOD_VECTOR*){return FMOD_OK;}
XPLM_API FMOD_RESULT XPLMSetAudioFadeDistance(FMOD_CHANNEL*,float,float){return FMOD_OK;}
// ---- XPLMInstance ------------------------------------------------------------------------------------------------------
XPLM_API XPLMInstanceRef XPLMCreateInstance(XPLMObjectRef,const char**){
 std::lock_guard<std::recursive_mutex> g(g_mu);return reinterpret_cast<XPLMInstanceRef>(++g_next_handle);
}
XPLM_API void XPLMDestroyInstance(XPLMInstanceRef){}
XPLM_API void XPLMInstanceSetPosition(XPLMInstanceRef,const XPLMDrawInfo_t*,const float*){}
}  // extern "C"
// ---- XpHost control exports --------------------------------------------------------------------------------------------
namespace {
std::string g_misses_text;
std::vector<std::string> Fields(const std::string& s){std::vector<std::string> o;std::size_t a=0;while(a<s.size()){while(a<s.size()&&(s[a]==' '||s[a]=='\t'))++a;const auto b=s.find_first_of(" \t",a);o.push_back(s.substr(a,b==std::string::npos?std::string::npos:b-a));a=b==std::string::npos?s.size():b;}return o;}
int NavTypeFor(int row_code){  // earth_nav.dat row codes -> XPLMNavType
 switch(row_code){case 1:return xplm_Nav_Airport;case 2:return xplm_Nav_NDB;case 3:return xplm_Nav_VOR;
  case 4:return xplm_Nav_ILS;case 5:return xplm_Nav_Localizer;case 6:return xplm_Nav_GlideSlope;
  case 7:return xplm_Nav_OuterMarker;case 8:return xplm_Nav_MiddleMarker;case 9:return xplm_Nav_InnerMarker;
  case 12:return xplm_Nav_DME;case 13:return xplm_Nav_Fix;case 14:return xplm_Nav_TACAN;default:return xplm_Nav_Unknown;}
}
}  // namespace
extern "C" {
__declspec(dllexport) int XpHost_Init(const XpHostConfig* cfg){
 std::lock_guard<std::recursive_mutex> g(g_mu);
 if(!cfg||!cfg->out_dir){g_error="XpHost_Init: config/out_dir missing";return -1;}
 if(g_log)std::fclose(g_log);
 const std::string log_path=std::string(cfg->out_dir)+"\\Log.txt";
 if(fopen_s(&g_log,log_path.c_str(),"w")||!g_log){g_error="cannot open "+log_path;return -1;}
 g_system_path=cfg->system_path?cfg->system_path:"";
 if(!g_system_path.empty()&&g_system_path.back()!='\\'&&g_system_path.back()!='/')g_system_path+='\\';
 g_plugin_path=cfg->plugin_path?cfg->plugin_path:"";g_acf_path=cfg->acf_path?cfg->acf_path:"";g_acf_name=cfg->acf_name?cfg->acf_name:"";
 g_lat0=cfg->field_lat;g_lon0=cfg->field_lon;g_elev0=cfg->field_elev_m;g_magvar=cfg->magvar;
 const int nrefs=LoadDataRefs(cfg->datarefs_txt);
 const int ncmds=LoadCommands(cfg->commands_txt);
 if(nrefs<0||ncmds<0){g_error="cannot read explicitly supplied dataref/command input";return -1;}
 SeedParked(cfg->true_heading);
 {std::string rel=g_acf_path;for(char& c:rel)if(c=='\\')c='/';
  const auto slash=rel.rfind('/');const auto prev=slash==std::string::npos?slash:rel.rfind('/',slash-1);
  SeedStr("sim/aircraft/view/acf_relative_path",(prev==std::string::npos?rel:rel.substr(prev+1)).c_str());}
 LogRaw("xphost headless host (fake XPLM_64.dll; no X-Plane flight model or rendering)\n");
 char buf[512];std::snprintf(buf,sizeof buf,"xphost: seeded %d datarefs, %d commands; parked lat %.5f lon %.5f elev %.1f m hdg %.1f\n",
  nrefs,ncmds,g_lat0,g_lon0,g_elev0,cfg->true_heading);LogRaw(buf);
 return nrefs;
}
__declspec(dllexport) const char* XpHost_LastError(){return g_error.c_str();}
__declspec(dllexport) void XpHost_SetSelfInfo(const char* n,const char* s,const char* d){
 std::lock_guard<std::recursive_mutex> g(g_mu);g_self_name=n?n:"";g_self_sig=s?s:"";g_self_desc=d?d:"";
}
__declspec(dllexport) void XpHost_SetEnabled(int v){std::lock_guard<std::recursive_mutex> g(g_mu);g_enabled=v?1:0;}
__declspec(dllexport) void XpHost_OnMessage(XpHostMessageFn fn,void* ref){std::lock_guard<std::recursive_mutex> g(g_mu);g_msg_fn=fn;g_msg_ref=ref;}
__declspec(dllexport) void XpHost_OnCommand(XpHostCommandFn fn,void* ref){std::lock_guard<std::recursive_mutex> g(g_mu);g_cmd_fn=fn;g_cmd_ref=ref;}
__declspec(dllexport) void XpHost_Log(const char* line){std::lock_guard<std::recursive_mutex> g(g_mu);if(line)LogLine(line);}
__declspec(dllexport) int XpHost_Step(double dt){
 std::lock_guard<std::recursive_mutex> g(g_mu);
 g_dt=dt;++g_frame;g_now+=dt;
 SetSimScalar("sim/time/total_running_time_sec",g_now);
 SetSimScalar("sim/time/total_flight_time_sec",g_now);
 SetSimScalar("sim/time/local_time_sec",std::fmod(43200.+g_now,86400.));
 SetSimScalar("sim/time/zulu_time_sec",std::fmod(43200.+g_now,86400.));
 SetSimScalar("sim/operation/misc/frame_rate_period",dt);
 SetSimScalar("sim/time/framerate_period",dt);
 SetSimScalar("sim/time/sim_speed_actual",1.);SetSimScalar("sim/time/sim_speed_actual_ogl",1.);
 const std::size_t nc=g_cmd_order.size();
 for(std::size_t i=0;i<nc;++i)if(g_cmd_order[i]->down)Dispatch(g_cmd_order[i],xplm_CommandContinue);
 DispatchLoops(xplm_FlightLoop_Phase_BeforeFlightModel);
 FmRefresh();
 DispatchLoops(xplm_FlightLoop_Phase_AfterFlightModel);
 return g_faulted?1:0;
}
__declspec(dllexport) double XpHost_SimTime(){std::lock_guard<std::recursive_mutex> g(g_mu);return g_now;}
__declspec(dllexport) int XpHost_Types(const char* n){std::lock_guard<std::recursive_mutex> g(g_mu);auto* e=FindEntry(n,false);return e?e->types:0;}
__declspec(dllexport) int XpHost_Writable(const char* n){std::lock_guard<std::recursive_mutex> g(g_mu);auto* e=FindEntry(n,false);return e&&e->writable?1:0;}
__declspec(dllexport) int XpHost_Length(const char* n){std::lock_guard<std::recursive_mutex> g(g_mu);auto* e=FindEntry(n,false);return e?EntryLength(e):-1;}
__declspec(dllexport) int XpHost_GetScalar(const char* n,double* out){std::lock_guard<std::recursive_mutex> g(g_mu);auto* e=FindEntry(n,false);if(!e||!out)return 0;*out=GetScalarE(e);return 1;}
__declspec(dllexport) int XpHost_SetScalar(const char* n,double v){std::lock_guard<std::recursive_mutex> g(g_mu);auto* e=FindEntry(n,false);if(!e)return 0;SetScalarE(e,v);return 1;}
__declspec(dllexport) int XpHost_GetFloats(const char* n,float* out,int off,int count){std::lock_guard<std::recursive_mutex> g(g_mu);return GetFloatsE(FindEntry(n,false),out,off,count);}
__declspec(dllexport) int XpHost_SetFloats(const char* n,const float* in,int off,int count){std::lock_guard<std::recursive_mutex> g(g_mu);auto* e=FindEntry(n,false);if(!e)return 0;SetFloatsE(e,in,off,count);return 1;}
__declspec(dllexport) int XpHost_GetInts(const char* n,int* out,int off,int count){std::lock_guard<std::recursive_mutex> g(g_mu);return GetIntsE(FindEntry(n,false),out,off,count);}
__declspec(dllexport) int XpHost_SetInts(const char* n,const int* in,int off,int count){std::lock_guard<std::recursive_mutex> g(g_mu);auto* e=FindEntry(n,false);if(!e)return 0;SetIntsE(e,in,off,count);return 1;}
__declspec(dllexport) int XpHost_GetBytes(const char* n,void* out,int off,int count){std::lock_guard<std::recursive_mutex> g(g_mu);return GetBytesE(FindEntry(n,false),out,off,count);}
__declspec(dllexport) int XpHost_SetBytes(const char* n,const void* in,int off,int count){std::lock_guard<std::recursive_mutex> g(g_mu);auto* e=FindEntry(n,false);if(!e)return 0;SetBytesE(e,in,off,count);return 1;}
__declspec(dllexport) int XpHost_Command(const char* n,int action){
 std::lock_guard<std::recursive_mutex> g(g_mu);if(!n)return 0;auto* c=NewCmd(n,"");
 if(action==1)FireBegin(c);else if(action==2)FireEnd(c);else FireOnce(c);return 1;
}
__declspec(dllexport) int XpHost_CountRefs(){std::lock_guard<std::recursive_mutex> g(g_mu);return static_cast<int>(g_order.size());}
__declspec(dllexport) const char* XpHost_RefName(int i){std::lock_guard<std::recursive_mutex> g(g_mu);return i>=0&&i<static_cast<int>(g_order.size())?g_order[static_cast<std::size_t>(i)]->name.c_str():nullptr;}
__declspec(dllexport) int XpHost_CountCommands(){std::lock_guard<std::recursive_mutex> g(g_mu);return static_cast<int>(g_cmd_order.size());}
__declspec(dllexport) const char* XpHost_CommandName(int i){std::lock_guard<std::recursive_mutex> g(g_mu);return i>=0&&i<static_cast<int>(g_cmd_order.size())?g_cmd_order[static_cast<std::size_t>(i)]->name.c_str():nullptr;}
__declspec(dllexport) const char* XpHost_CommandDesc(int i){std::lock_guard<std::recursive_mutex> g(g_mu);return i>=0&&i<static_cast<int>(g_cmd_order.size())?g_cmd_order[static_cast<std::size_t>(i)]->desc.c_str():nullptr;}
__declspec(dllexport) void XpHost_Stats(XpHostStats* s){
 std::lock_guard<std::recursive_mutex> g(g_mu);if(!s)return;
 *s={g_frame,g_sim_r,g_sim_w,g_plug_r,g_plug_w,g_cmd_fired,g_loops_run,static_cast<long long>(g_misses.size()),g_now,g_faulted?1:0};
}
__declspec(dllexport) const char* XpHost_Misses(){
 std::lock_guard<std::recursive_mutex> g(g_mu);g_misses_text.clear();
 for(const auto& m:g_misses)g_misses_text+=m+"\n";return g_misses_text.c_str();
}
__declspec(dllexport) int XpHost_LoadNavDat(const char* path){
 std::lock_guard<std::recursive_mutex> g(g_mu);
 FILE* f=nullptr;if(!path||fopen_s(&f,path,"rb")||!f){g_error=std::string("cannot open navdat: ")+(path?path:"");return -1;}
 std::vector<Nav> navs;char line[1024];int skipped=0;
 while(std::fgets(line,sizeof line,f)){
  const std::string s=Trim(line);if(s.empty()||s[0]=='#'||s=="99")continue;
  const auto p=Fields(s);if(p.size()<8){++skipped;continue;}
  const int code=std::atoi(p[0].c_str());const int type=NavTypeFor(code);if(!type){++skipped;continue;}
  Nav n;n.type=type;n.lat=static_cast<float>(std::atof(p[1].c_str()));n.lon=static_cast<float>(std::atof(p[2].c_str()));
  n.height=static_cast<float>(std::atof(p[3].c_str())*.3048);  // earth_nav.dat elevation is feet; XPLM height is meters
  const int raw_freq=std::atoi(p[4].c_str());
  n.freq=type==xplm_Nav_NDB?raw_freq:raw_freq;  // VHF rows already carry e.g. 11680
  // fields: code lat lon elev freq range {slaved_variation | localizer course ...} ident region name...
  if(type==xplm_Nav_VOR&&p.size()>6)n.heading=static_cast<float>(std::atof(p[6].c_str()));
  if((type==xplm_Nav_Localizer||type==xplm_Nav_ILS||type==xplm_Nav_GlideSlope)&&p.size()>6)n.heading=static_cast<float>(std::atof(p[6].c_str()));
  std::size_t name_at=8;
  if(code==4||code==5||code==6)name_at=9;  // ILS rows carry an extra associated-runway field
  if(p.size()>7){n.id=p[7];}
  if(p.size()>name_at){for(std::size_t i=name_at;i<p.size();++i){if(!n.name.empty())n.name+=' ';n.name+=p[i];}}
  navs.push_back(std::move(n));
 }
 std::fclose(f);
 if(navs.empty()){g_error="navdat parsed no rows";return -1;}
 g_navs=std::move(navs);
 char buf[160];std::snprintf(buf,sizeof buf,"xphost: navdat %s: %d aids (%d rows skipped)\n",path,static_cast<int>(g_navs.size()),skipped);LogRaw(buf);
 return static_cast<int>(g_navs.size());
}
}  // extern "C"
