// xphost: headless host for an X-Plane plugin. Loads win.xpl against the fake
// XPLM_64.dll, plays X-Plane's plugin lifecycle, and drives its flight loops on a fast or real
// clock. See README.md for the CLI and the script language.
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <GL/gl.h>
#include "xphost_api.h"
#include "serve.hpp"
#include <algorithm>
#include <atomic>
#include <chrono>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <filesystem>
#include <mutex>
#include <string>
#include <thread>
#include <vector>
namespace {
namespace fs=std::filesystem;
// XPLM message ids and data type bits (SDK values; the host stays free of SDK headers).
constexpr int kMsgPlaneLoaded=102,kMsgAirportLoaded=103,kMsgSceneryLoaded=104;
constexpr int kTypeInt=1,kTypeFloat=2,kTypeDouble=4,kTypeFloatArray=8,kTypeIntArray=16,kTypeData=32;
XpHostApi g_api{};bool g_api_ok=false;std::atomic<bool> g_stop{false};
typedef int (*FnStart)(char*,char*,char*);typedef int (*FnEnable)();typedef void (*FnVoid)();
typedef void (*FnMessage)(int,int,void*);
FnStart p_start=nullptr;FnEnable p_enable=nullptr;FnVoid p_disable=nullptr,p_stop=nullptr;FnMessage p_message=nullptr;
bool g_plugin_fault=false;FILE* g_cmd_log=nullptr;std::mutex g_cmd_mu;
int SehStart(FnStart f,char* a,char* b,char* c,unsigned* code){__try{return f(a,b,c);}__except(EXCEPTION_EXECUTE_HANDLER){*code=GetExceptionCode();return -1;}}
int SehEnable(FnEnable f,unsigned* code){__try{return f();}__except(EXCEPTION_EXECUTE_HANDLER){*code=GetExceptionCode();return -1;}}
int SehVoid(FnVoid f,unsigned* code){__try{f();return 0;}__except(EXCEPTION_EXECUTE_HANDLER){*code=GetExceptionCode();return -1;}}
int SehMessage(FnMessage f,int from,int msg,void* p,unsigned* code){__try{f(from,msg,p);return 0;}__except(EXCEPTION_EXECUTE_HANDLER){*code=GetExceptionCode();return -1;}}
void SendMessage(int msg){
 if(!p_message)return;unsigned code=0;
 if(SehMessage(p_message,0,msg,nullptr,&code)){g_plugin_fault=true;std::printf("xphost: plugin fault in XPluginReceiveMessage (SEH 0x%08x)\n",code);}
}
void OnFakeMessage(int msg,void*,void*){SendMessage(msg);}
void OnFakeCommand(const char* name,int phase,double t,void*){
 if(phase==1)return;std::lock_guard<std::mutex> g(g_cmd_mu);
 if(g_cmd_log){std::fprintf(g_cmd_log,"%.4f\t%s\t%d\n",t,name,phase);std::fflush(g_cmd_log);}
}
std::string Narrow(const fs::path& p){return p.string();}
void MakeDirs(const fs::path& p){std::error_code e;fs::create_directories(p,e);}
bool MakeGlContext(){
 WNDCLASSA wc{};wc.lpfnWndProc=&DefWindowProcA;wc.hInstance=GetModuleHandleA(nullptr);wc.lpszClassName="xphost-gl";
 if(!RegisterClassA(&wc)&&GetLastError()!=ERROR_CLASS_ALREADY_EXISTS)return false;
 HWND w=CreateWindowExA(0,wc.lpszClassName,"xphost",WS_OVERLAPPED,0,0,64,64,nullptr,nullptr,wc.hInstance,nullptr);
 if(!w)return false;HDC dc=GetDC(w);if(!dc)return false;
 PIXELFORMATDESCRIPTOR pfd{};pfd.nSize=sizeof pfd;pfd.nVersion=1;pfd.dwFlags=PFD_DRAW_TO_WINDOW|PFD_SUPPORT_OPENGL;
 pfd.iPixelType=PFD_TYPE_RGBA;pfd.cColorBits=24;pfd.cDepthBits=16;
 const int fmt=ChoosePixelFormat(dc,&pfd);if(!fmt||!SetPixelFormat(dc,fmt,&pfd))return false;
 HGLRC rc=wglCreateContext(dc);if(!rc)return false;
 return wglMakeCurrent(dc,rc)!=FALSE;
}
std::vector<fs::path> FindXpls(const fs::path& pkg){
 std::error_code e;std::vector<fs::path> found;
 for(const auto& d:fs::directory_iterator(pkg/"plugins",e)){
  const auto cand=d.path()/"64"/"win.xpl";
  if(fs::is_regular_file(cand,e))found.push_back(cand);
 }
 std::sort(found.begin(),found.end());
 return found;
}
// ---- dataref access by "name" or "name[index]" -------------------------------------------------
struct RefSpec { std::string name;int index=-1; };
RefSpec ParseRef(const std::string& tok){
 RefSpec r;const auto lb=tok.find('[');
 if(lb!=std::string::npos&&tok.back()==']'){r.name=tok.substr(0,lb);r.index=std::atoi(tok.c_str()+lb+1);}
 else r.name=tok;
 return r;
}
bool RefValue(const RefSpec& r,double* out){
 const int types=g_api.types(r.name.c_str());if(!types)return false;
 if(types&kTypeFloatArray){float v=0;if(g_api.get_floats(r.name.c_str(),&v,std::max(r.index,0),1)!=1)return false;*out=v;return true;}
 if(types&kTypeIntArray){int v=0;if(g_api.get_ints(r.name.c_str(),&v,std::max(r.index,0),1)!=1)return false;*out=v;return true;}
 return g_api.get_scalar(r.name.c_str(),out)!=0;
}
bool RefSet(const RefSpec& r,double v){
 const int types=g_api.types(r.name.c_str());if(!types)return false;
 if(types&kTypeFloatArray){const float f=static_cast<float>(v);return g_api.set_floats(r.name.c_str(),&f,std::max(r.index,0),1)!=0;}
 if(types&kTypeIntArray){const int i=static_cast<int>(v);return g_api.set_ints(r.name.c_str(),&i,std::max(r.index,0),1)!=0;}
 return g_api.set_scalar(r.name.c_str(),v)!=0;
}
std::string RefDescribe(const RefSpec& r){
 const int types=g_api.types(r.name.c_str());if(!types)return "(absent)";
 char buf[4096];std::string out;
 if(types&kTypeData){
  const int len=g_api.length(r.name.c_str());std::vector<char> b(static_cast<std::size_t>(std::max(len,0))+1,0);
  const int n=len>0?g_api.get_bytes(r.name.c_str(),b.data(),0,len):0;
  for(int i=0;i<n&&b[static_cast<std::size_t>(i)];++i)out+=b[static_cast<std::size_t>(i)];
  return "\""+out+"\"";
 }
 if(types&(kTypeFloatArray|kTypeIntArray)){
  if(r.index>=0){double v=0;return RefValue(r,&v)?(std::snprintf(buf,sizeof buf,"%.9g",v),std::string(buf)):"(bad index)";}
  const int len=std::min(g_api.length(r.name.c_str()),32);out="[";
  for(int i=0;i<len;++i){double v=0;RefSpec e{r.name,i};RefValue(e,&v);std::snprintf(buf,sizeof buf,i?", %.9g":"%.9g",v);out+=buf;}
  return out+"]";
 }
 double v=0;if(!g_api.get_scalar(r.name.c_str(),&v))return "(absent)";
 std::snprintf(buf,sizeof buf,"%.9g",v);return buf;
}
// ---- runner + script engine ---------------------------------------------------------------------
struct Trace { FILE* f=nullptr;std::vector<RefSpec> refs;long long every=1,left=0; };
struct Runner {
 double dt=1./60.;bool fast=true;bool failed=false;std::string out_dir,debug_request,debug_result;
 std::chrono::steady_clock::time_point next{},wall0{};bool pacing=false;
 Trace trace;
 void PaceReset(){pacing=!fast;next=wall0=std::chrono::steady_clock::now();}
 double Wall()const{return std::chrono::duration<double>(std::chrono::steady_clock::now()-wall0).count();}
 void Sample(){
  if(!trace.f)return;
  std::fprintf(trace.f,"%.4f",g_api.sim_time());
  for(const auto& r:trace.refs){double v=0;const bool ok=RefValue(r,&v);std::fprintf(trace.f,ok?"\t%.9g":"\t",v);}
  std::fputc('\n',trace.f);std::fflush(trace.f);
 }
 void StepOnce(){
  if(g_api.step(dt))g_plugin_fault=true;
  if(trace.f&&--trace.left<=0){Sample();trace.left=trace.every;}
  if(pacing){next+=std::chrono::duration_cast<std::chrono::steady_clock::duration>(std::chrono::duration<double>(dt));
   const auto now=std::chrono::steady_clock::now();
   if(next>now)std::this_thread::sleep_until(next);else if(now-next>std::chrono::milliseconds(250))next=now;}
 }
 void Advance(double seconds){const long long n=static_cast<long long>(std::llround(seconds/dt));for(long long i=0;i<n&&!g_stop.load();++i)StepOnce();}
 bool WaitUntil(const RefSpec& r,const std::string& op,double want,double timeout){
  const long long limit=static_cast<long long>(std::llround(timeout/dt));
  for(long long i=0;i<=limit&&!g_stop.load();++i){
   double v=0;if(RefValue(r,&v)){
    const bool ok=op=="=="?v==want:op=="!="?v!=want:op==">="?v>=want:op=="<="?v<=want:op==">"?v>want:v<want;
    if(ok)return true;
   }
   if(i<limit)StepOnce();
  }
  return false;
 }
 void Fail(const std::string& what){failed=true;std::printf("script FAIL: %s\n",what.c_str());}
};
std::vector<std::string> Words(const std::string& s){
 std::vector<std::string> o;std::size_t a=0;
 while(a<s.size()){while(a<s.size()&&(s[a]==' '||s[a]=='\t'))++a;if(a>=s.size())break;
  const auto b=s.find_first_of(" \t",a);o.push_back(s.substr(a,b==std::string::npos?std::string::npos:b-a));a=b==std::string::npos?s.size():b;}
 return o;
}
// The line after its first n words (leading blanks skipped) — for echo/mark/dbg/sets payloads.
std::string RestAfter(const std::string& s,int n){
 std::size_t a=0;
 for(int i=0;i<n;++i){while(a<s.size()&&(s[a]==' '||s[a]=='\t'))++a;const auto b=s.find_first_of(" \t",a);a=b==std::string::npos?s.size():b;}
 while(a<s.size()&&(s[a]==' '||s[a]=='\t'))++a;
 return s.substr(a);
}
void RunScript(Runner& run,const fs::path& file){
 FILE* f=nullptr;
 if(fopen_s(&f,file.string().c_str(),"r")||!f){run.Fail("cannot open script "+file.string());return;}
 char linebuf[8192];int lineno=0;
 while(std::fgets(linebuf,sizeof linebuf,f)){
  ++lineno;std::string line=linebuf;
  while(!line.empty()&&(line.back()=='\n'||line.back()=='\r'))line.pop_back();
  const auto hash=line.find('#');if(hash!=std::string::npos)line.erase(hash);
  const auto w=Words(line);
  if(w.empty())continue;
  const std::string rest=RestAfter(line,1);
  const auto at=[&](std::size_t i)->std::string{return i<w.size()?w[i]:"";};
  const std::string& verb=w[0];
  if(verb=="echo")std::printf("%s\n",rest.c_str());
  else if(verb=="mark")std::printf("mark %s: sim=%.3f wall=%.3f\n",rest.c_str(),g_api.sim_time(),run.Wall());
  else if(verb=="wait")run.Advance(std::atof(at(1).c_str()));
  else if(verb=="wait_until"){
   const auto r=ParseRef(at(1));const double want=std::atof(at(3).c_str()),timeout=w.size()>4?std::atof(at(4).c_str()):60.;
   if(run.WaitUntil(r,at(2),want,timeout))std::printf("wait_until %s %s %.9g: ok at t=%.2f\n",at(1).c_str(),at(2).c_str(),want,g_api.sim_time());
   else run.Fail("wait_until "+line+" timed out (t="+std::to_string(g_api.sim_time())+", value "+RefDescribe(r)+")");
  }
  else if(verb=="set"){if(!RefSet(ParseRef(at(1)),std::atof(at(2).c_str())))run.Fail("set (absent ref): "+line);}
  else if(verb=="sets"){
   const auto r=ParseRef(at(1));const std::string text=RestAfter(line,2);
   if(!g_api.set_bytes(r.name.c_str(),text.c_str(),0,static_cast<int>(text.size())))run.Fail("sets (absent ref): "+line);
  }
  else if(verb=="get")std::printf("%s = %s\n",at(1).c_str(),RefDescribe(ParseRef(at(1))).c_str());
  else if(verb=="cmd")g_api.command(at(1).c_str(),0);
  else if(verb=="cmd_begin")g_api.command(at(1).c_str(),1);
  else if(verb=="cmd_end")g_api.command(at(1).c_str(),2);
  else if(verb=="dbg"){
   const std::string req=rest;
   if(run.debug_request.empty()||run.debug_result.empty()){run.Fail("dbg requires --debug-request and --debug-result");continue;}
   if(!(g_api.types(run.debug_request.c_str())&kTypeData)||!(g_api.types(run.debug_result.c_str())&kTypeData)){
    run.Fail("dbg: request and result must be byte datarefs");continue;}
   if(!g_api.set_bytes(run.debug_request.c_str(),req.c_str(),0,static_cast<int>(req.size()))){run.Fail("dbg: debug request dataref absent");continue;}
   run.Advance(2.*run.dt);
   char out[8192]{};g_api.get_bytes(run.debug_result.c_str(),out,0,static_cast<int>(sizeof out)-1);
   std::printf("dbg %s -> %s\n",req.c_str(),out);
  }
  else if(verb=="trace"){
   if(run.trace.f)std::fclose(run.trace.f);
   const auto path=fs::path(run.out_dir)/at(1);
   if(fopen_s(&run.trace.f,path.string().c_str(),"w")||!run.trace.f){run.Fail("trace: cannot open "+path.string());continue;}
   run.trace.refs.clear();for(std::size_t i=3;i<w.size();++i)run.trace.refs.push_back(ParseRef(w[i]));
   run.trace.every=std::max(1LL,std::llround(std::atof(at(2).c_str())/run.dt));run.trace.left=0;
   std::fprintf(run.trace.f,"t");for(const auto& r:run.trace.refs){std::fprintf(run.trace.f,"\t%s",r.name.c_str());if(r.index>=0)std::fprintf(run.trace.f,"[%d]",r.index);}
   std::fprintf(run.trace.f,"\n");run.Sample();run.trace.left=run.trace.every;
   std::printf("trace -> %s every %lld frame(s)\n",path.string().c_str(),run.trace.every);
  }
  else if(verb=="trace_off"){if(run.trace.f)std::fclose(run.trace.f);run.trace.f=nullptr;}
  else if(verb=="dump"){
   const auto path=fs::path(run.out_dir)/at(1);FILE* o=nullptr;
   if(fopen_s(&o,path.string().c_str(),"w")||!o){run.Fail("dump: cannot open "+path.string());continue;}
   std::fprintf(o,"dataref\tvalue\n");
   for(std::size_t i=2;i<w.size();++i)std::fprintf(o,"%s\t%s\n",w[i].c_str(),RefDescribe(ParseRef(w[i])).c_str());
   std::fclose(o);std::printf("dump -> %s\n",path.string().c_str());
  }
  else if(verb=="expect"){
   const auto r=ParseRef(at(1));const double lo=std::atof(at(2).c_str()),hi=std::atof(at(3).c_str());double v=0;
   if(!RefValue(r,&v))run.Fail("expect (absent ref): "+line);
   else if(v>=lo&&v<=hi)std::printf("expect %s = %.9g in [%.9g, %.9g]: PASS\n",at(1).c_str(),v,lo,hi);
   else run.Fail("expect "+at(1)+" = "+std::to_string(v)+" outside ["+std::to_string(lo)+", "+std::to_string(hi)+"]");
  }
  else run.Fail("unknown verb at line "+std::to_string(lineno)+": "+verb);
 }
 std::fclose(f);
}
void Usage(){
 std::printf(
  "xphost --pkg <package folder> --acf <acf file name> --clock fast|real [options]\n"
  "xphost serve --port 8199 --pkg <package folder> --acf <acf file name> [--clock fast|real]\n"
  "options: --seconds N (default: script length, else 10) --script FILE --out DIR (default out)\n"
  "  --dt SECONDS (default 1/60) --xpl PATH --datarefs PATH|none --commands PATH|none\n"
  "  --navdat PATH|none (earth_nav.dat; datarefs/commands/navdat default none)\n"
  "  --debug-request NAME --debug-result NAME (optional byte refs for script dbg)\n"
  "  --flight-start cold|running (default cold) --lat D --lon D --elev M --heading D --magvar D\n"
  "  Pose defaults to latitude/longitude/elevation/heading/magvar 0.\n"
  "  serve --selftest runs against the bundled xphost/test fixture; it does not listen.\n");
}
template<typename T> T Proc(HMODULE m,const char* name,bool* ok){
 auto p=reinterpret_cast<T>(GetProcAddress(m,name));if(!p){*ok=false;std::printf("xphost: fake XPLM_64.dll lacks %s\n",name);}return p;
}
}  // namespace
int main(int argc,char** argv){
 std::setvbuf(stdout,nullptr,_IONBF,0);
 bool serve=false,selftest=false,bad_arg=false;int port=8199;
 std::string pkg,acf,clock_mode,script,out_dir="out",xpl_override,navdat="none",flight_start="cold",debug_request,debug_result;
 std::string datarefs="none",commands="none";
 double seconds=-1,dt=1./60.,lat=0,lon=0,elev=0,heading=0,magvar=0;
 for(int i=1;i<argc;++i){const std::string a=argv[i];auto val=[&]()->std::string{
   if(i+1<argc&&std::strncmp(argv[i+1],"--",2)!=0)return argv[++i];
   std::printf("xphost: missing value for %s\n",a.c_str());bad_arg=true;return "";};
  auto number=[&](){const std::string s=val();char* end=nullptr;const double n=std::strtod(s.c_str(),&end);
   if(s.empty()||end!=s.c_str()+s.size()||!std::isfinite(n)){std::printf("xphost: invalid number for %s\n",a.c_str());bad_arg=true;return 0.;}return n;};
  if(a=="serve")serve=true;
  else if(a=="--selftest")selftest=true;
  else if(a=="--port"){const double n=number();if(n<1||n>65535||n!=std::floor(n))bad_arg=true;else port=static_cast<int>(n);}
  else if(a=="--pkg")pkg=val();else if(a=="--acf")acf=val();else if(a=="--clock")clock_mode=val();
  else if(a=="--seconds"){seconds=number();if(seconds<0)bad_arg=true;}else if(a=="--script")script=val();
  else if(a=="--out")out_dir=val();else if(a=="--dt")dt=number();
  else if(a=="--xpl")xpl_override=val();else if(a=="--datarefs")datarefs=val();else if(a=="--commands")commands=val();
  else if(a=="--navdat")navdat=val();else if(a=="--flight-start")flight_start=val();
  else if(a=="--debug-request")debug_request=val();else if(a=="--debug-result")debug_result=val();
  else if(a=="--lat")lat=number();else if(a=="--lon")lon=number();
  else if(a=="--elev")elev=number();else if(a=="--heading")heading=number();else if(a=="--magvar")magvar=number();
  else if(a=="--help"||a=="-h"){Usage();return 0;}
  else{std::printf("xphost: unknown argument %s\n",a.c_str());Usage();return 1;}
 }
 if(bad_arg||pkg.empty()||acf.empty()||out_dir.empty()||dt<=0||dt>1.||std::abs(lat)>=90||std::abs(lon)>180){
  std::printf("xphost: --pkg and --acf are required; --dt must be in (0,1], latitude in (-90,90), longitude in [-180,180]\n");Usage();return 1;}
 if((selftest&&!serve)||debug_request.empty()!=debug_result.empty()||(flight_start!="cold"&&flight_start!="running")){
  std::printf("xphost: --selftest requires serve; debug refs must be paired; --flight-start must be cold or running\n");return 1;}
 if(clock_mode.empty())clock_mode=serve?"real":"fast";
 if(clock_mode!="fast"&&clock_mode!="real"){std::printf("xphost: --clock must be fast or real\n");return 1;}
 const fs::path pkg_path=fs::absolute(pkg),acf_path=fs::absolute(fs::path(pkg)/acf);
 if(!fs::is_regular_file(acf_path)){std::printf("xphost: acf not found: %s\n",Narrow(acf_path).c_str());return 1;}
 fs::path xpl_path;
 if(xpl_override.empty()){
  const auto candidates=FindXpls(pkg_path);
  if(candidates.size()!=1){std::printf("xphost: found %zu plugins under %s\\plugins\\*\\64\\win.xpl; select one with --xpl\n",candidates.size(),Narrow(pkg_path).c_str());
   for(const auto& candidate:candidates)std::printf("  %s\n",Narrow(candidate).c_str());return 1;}
  xpl_path=candidates.front();
 }else xpl_path=fs::absolute(xpl_override);
 if(!fs::is_regular_file(xpl_path)){std::printf("xphost: plugin not found: %s\n",Narrow(xpl_path).c_str());return 1;}
 for(const auto& input:{datarefs,commands,navdat})if(input!="none"&&!fs::is_regular_file(input)){
  std::printf("xphost: explicit input file not found: %s\n",input.c_str());return 1;}
 const fs::path out=fs::absolute(out_dir),sysroot=out/"xplane";
 MakeDirs(out);MakeDirs(sysroot/"Output");MakeDirs(sysroot/"Resources"/"plugins");
 if(fopen_s(&g_cmd_log,(out/"commands.tsv").string().c_str(),"w")==0&&g_cmd_log)std::fprintf(g_cmd_log,"t\tcommand\tphase\n");
 // The fake must be loaded (by full path) before win.xpl so the name XPLM_64.dll binds to it.
 char exe_buf[MAX_PATH]{};GetModuleFileNameA(nullptr,exe_buf,MAX_PATH);
 const fs::path exe_dir=fs::path(exe_buf).parent_path();
 SetDllDirectoryW(exe_dir.wstring().c_str());
 HMODULE fake=LoadLibraryW((exe_dir/"XPLM_64.dll").wstring().c_str());
 if(!fake){std::printf("xphost: cannot load %s (error %lu)\n",Narrow(exe_dir/"XPLM_64.dll").c_str(),GetLastError());return 1;}
 bool ok=true;
 g_api.init=Proc<XpHost_InitFn>(fake,"XpHost_Init",&ok);g_api.last_error=Proc<XpHost_LastErrorFn>(fake,"XpHost_LastError",&ok);
 g_api.set_self_info=Proc<XpHost_SetSelfInfoFn>(fake,"XpHost_SetSelfInfo",&ok);g_api.set_enabled=Proc<XpHost_SetEnabledFn>(fake,"XpHost_SetEnabled",&ok);
 g_api.on_message=Proc<XpHost_OnMessageFn>(fake,"XpHost_OnMessage",&ok);g_api.on_command=Proc<XpHost_OnCommandFn>(fake,"XpHost_OnCommand",&ok);
 g_api.step=Proc<XpHost_StepFn>(fake,"XpHost_Step",&ok);g_api.sim_time=Proc<XpHost_SimTimeFn>(fake,"XpHost_SimTime",&ok);
 g_api.types=Proc<XpHost_TypesFn>(fake,"XpHost_Types",&ok);g_api.writable=Proc<XpHost_WritableFn>(fake,"XpHost_Writable",&ok);
 g_api.length=Proc<XpHost_LengthFn>(fake,"XpHost_Length",&ok);
 g_api.get_scalar=Proc<XpHost_GetScalarFn>(fake,"XpHost_GetScalar",&ok);g_api.set_scalar=Proc<XpHost_SetScalarFn>(fake,"XpHost_SetScalar",&ok);
 g_api.get_floats=Proc<XpHost_GetFloatsFn>(fake,"XpHost_GetFloats",&ok);g_api.set_floats=Proc<XpHost_SetFloatsFn>(fake,"XpHost_SetFloats",&ok);
 g_api.get_ints=Proc<XpHost_GetIntsFn>(fake,"XpHost_GetInts",&ok);g_api.set_ints=Proc<XpHost_SetIntsFn>(fake,"XpHost_SetInts",&ok);
 g_api.get_bytes=Proc<XpHost_GetBytesFn>(fake,"XpHost_GetBytes",&ok);g_api.set_bytes=Proc<XpHost_SetBytesFn>(fake,"XpHost_SetBytes",&ok);
 g_api.command=Proc<XpHost_CommandFn2>(fake,"XpHost_Command",&ok);
 g_api.count_refs=Proc<XpHost_CountRefsFn>(fake,"XpHost_CountRefs",&ok);g_api.ref_name=Proc<XpHost_RefNameFn>(fake,"XpHost_RefName",&ok);
 g_api.count_commands=Proc<XpHost_CountCommandsFn>(fake,"XpHost_CountCommands",&ok);
 g_api.command_name=Proc<XpHost_CommandNameFn>(fake,"XpHost_CommandName",&ok);g_api.command_desc=Proc<XpHost_CommandDescFn>(fake,"XpHost_CommandDesc",&ok);
 g_api.stats=Proc<XpHost_StatsFn2>(fake,"XpHost_Stats",&ok);g_api.misses=Proc<XpHost_MissesFn>(fake,"XpHost_Misses",&ok);
 g_api.log=Proc<XpHost_LogFn>(fake,"XpHost_Log",&ok);g_api.load_navdat=Proc<XpHost_LoadNavDatFn>(fake,"XpHost_LoadNavDat",&ok);
 if(!ok)return 1;g_api_ok=true;
 const bool gl=MakeGlContext();
 std::printf("xphost: GL context %s\n",gl?"ready (hidden window)":"unavailable (continuing without GL)");
 const std::string out_s=Narrow(out),sys_s=Narrow(sysroot),xpl_s=Narrow(xpl_path),acf_s=Narrow(acf_path),acf_name=acf_path.filename().string();
 XpHostConfig cfg{};
 cfg.out_dir=out_s.c_str();cfg.system_path=sys_s.c_str();cfg.plugin_path=xpl_s.c_str();
 cfg.acf_path=acf_s.c_str();cfg.acf_name=acf_name.c_str();
 cfg.datarefs_txt=datarefs=="none"?nullptr:datarefs.c_str();
 cfg.commands_txt=commands=="none"?nullptr:commands.c_str();
 cfg.field_lat=lat;cfg.field_lon=lon;cfg.field_elev_m=elev;cfg.magvar=magvar;cfg.true_heading=heading;
 if(g_api.init(&cfg)<0){std::printf("xphost: fake init failed: %s\n",g_api.last_error());return 1;}
 { // An explicitly supplied nav file is also exposed in the generated simulator tree.
  const auto dst=sysroot/"Resources"/"default data"/"earth_nav.dat";std::error_code ec;
  if(navdat!="none"){
   const auto src=fs::absolute(navdat);MakeDirs(dst.parent_path());
   const bool same=fs::equivalent(src,dst,ec);ec.clear();
   if(!same)fs::copy_file(src,dst,fs::copy_options::overwrite_existing,ec);
   if(ec){std::printf("xphost: cannot copy navdat: %s\n",ec.message().c_str());return 1;}
   if(g_api.load_navdat(Narrow(src).c_str())<0){std::printf("xphost: navdat: %s\n",g_api.last_error());return 1;}
  }else{
   fs::remove(dst,ec);  // Do not expose a stale file from an earlier run in this output folder.
   if(ec){std::printf("xphost: cannot clear cached navdat: %s\n",ec.message().c_str());return 1;}
  }
 }
 g_api.on_message(&OnFakeMessage,nullptr);g_api.on_command(&OnFakeCommand,nullptr);
 g_api.log(("host loading "+xpl_s+" for "+acf_s).c_str());
 const auto wall0=std::chrono::steady_clock::now();
 HMODULE plugin=LoadLibraryW(xpl_path.wstring().c_str());
 if(!plugin){std::printf("xphost: cannot load win.xpl (error %lu)\n",GetLastError());g_api.log("win.xpl load failed");return 1;}
 p_start=reinterpret_cast<FnStart>(GetProcAddress(plugin,"XPluginStart"));
 p_enable=reinterpret_cast<FnEnable>(GetProcAddress(plugin,"XPluginEnable"));
 p_disable=reinterpret_cast<FnVoid>(GetProcAddress(plugin,"XPluginDisable"));
 p_stop=reinterpret_cast<FnVoid>(GetProcAddress(plugin,"XPluginStop"));
 p_message=reinterpret_cast<FnMessage>(GetProcAddress(plugin,"XPluginReceiveMessage"));
 if(!p_start||!p_enable||!p_disable||!p_stop||!p_message){std::printf("xphost: win.xpl lacks a plugin entry point\n");return 1;}
 char name[256]{},sig[256]{},desc[256]{};unsigned code=0;
 const int started=SehStart(p_start,name,sig,desc,&code);
 name[255]=sig[255]=desc[255]=0;
 if(started<0){std::printf("xphost: XPluginStart fault (SEH 0x%08x)\n",code);return 2;}
 std::printf("xphost: XPluginStart -> %d (%s / %s)\n",started,name,sig);
 if(!started)return 2;
 g_api.set_self_info(name,sig,desc);
 const int enabled=SehEnable(p_enable,&code);
 if(enabled<0){std::printf("xphost: XPluginEnable fault (SEH 0x%08x)\n",code);return 2;}
 std::printf("xphost: XPluginEnable -> %d\n",enabled);
 if(!enabled)return 2;
 g_api.set_enabled(1);
 if(flight_start=="running"){const int v=3;g_api.set_ints("sim/cockpit2/engine/actuators/ignition_key",&v,0,1);}
 SendMessage(kMsgPlaneLoaded);SendMessage(kMsgAirportLoaded);SendMessage(kMsgSceneryLoaded);
 Runner run;run.dt=dt;run.fast=clock_mode=="fast";run.out_dir=out_s;run.debug_request=debug_request;run.debug_result=debug_result;run.PaceReset();
 int serve_rc=0;
 if(serve&&selftest){serve_rc=ServeSelfTest(g_api,dt);}
 else if(serve){
  SetConsoleCtrlHandler([](DWORD)->BOOL{g_stop.store(true);return TRUE;},TRUE);
  serve_rc=RunServe(g_api,port,dt,clock_mode=="fast",g_stop);
 }
 else{
  if(!script.empty())RunScript(run,fs::absolute(script));
  const double target=seconds>=0?seconds:(script.empty()?10.:0.);
  if(target>0)run.Advance(target);
  if(run.trace.f)std::fclose(run.trace.f);
 }
 unsigned code2=0;
 if(SehVoid(p_disable,&code2)){g_plugin_fault=true;std::printf("xphost: XPluginDisable fault (SEH 0x%08x)\n",code2);}
 g_api.set_enabled(0);code2=0;
 if(SehVoid(p_stop,&code2)){g_plugin_fault=true;std::printf("xphost: XPluginStop fault (SEH 0x%08x)\n",code2);}
 XpHostStats st{};g_api.stats(&st);
 if(FILE* mf=nullptr;fopen_s(&mf,(out/"dataref-misses.txt").string().c_str(),"w")==0&&mf){std::fputs(g_api.misses(),mf);std::fclose(mf);}
 const double wall=std::chrono::duration<double>(std::chrono::steady_clock::now()-wall0).count();
 std::printf("xphost: sim %.2f s in %.2f s wall (%.1fx, %lld frames, dt %.4f)\n",st.sim_seconds,wall,wall>0?st.sim_seconds/wall:0.,st.frames,dt);
 std::printf("xphost: dataref reads sim=%lld plugin=%lld, writes sim=%lld plugin=%lld, commands=%lld, loops=%lld, find misses=%lld\n",
  st.sim_reads,st.plugin_reads,st.sim_writes,st.plugin_writes,st.commands_fired,st.loops_run,st.find_misses);
 std::printf("xphost: log %s\n",Narrow(out/"Log.txt").c_str());
 const bool faulted=g_plugin_fault||st.faulted;
 if(faulted)std::printf("xphost: PLUGIN FAULT (see Log.txt)\n");
 if(g_cmd_log)std::fclose(g_cmd_log);
 FreeLibrary(plugin);FreeLibrary(fake);
 if(faulted)return 2;
 if(serve)return selftest&&serve_rc?3:serve_rc;
 return run.failed?3:0;
}
