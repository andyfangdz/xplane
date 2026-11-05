// xphost serve: a subset of the X-Plane 12 Web API v3 over the fake store, so drivers written
// for the real sim can also talk to xphost. All value access and commands run on the sim
// thread; a dataref's name does not establish who owns its accessors.
#define WIN32_LEAN_AND_MEAN
#include <winsock2.h>
#include <ws2tcpip.h>
#include <windows.h>
#include "serve.hpp"
#include <algorithm>
#include <chrono>
#include <cmath>
#include <cstdio>
#include <cstdlib>
#include <cstring>
#include <functional>
#include <future>
#include <limits>
#include <map>
#include <mutex>
#include <optional>
#include <queue>
#include <string>
#include <thread>
#include <vector>
namespace {
constexpr int kTypeInt=1,kTypeFloat=2,kTypeDouble=4,kTypeFloatArray=8,kTypeIntArray=16,kTypeData=32;
const XpHostApi* g_api=nullptr;
std::atomic<bool>* g_serve_stop=nullptr;
std::mutex g_task_mu;std::queue<std::function<void()>> g_tasks;
void Enqueue(std::function<void()> f){std::lock_guard<std::mutex> g(g_task_mu);g_tasks.push(std::move(f));}
void DrainTasks(){for(;;){std::function<void()> f;{std::lock_guard<std::mutex> g(g_task_mu);if(g_tasks.empty())return;f=std::move(g_tasks.front());g_tasks.pop();}f();}}
std::string JsonEscape(const std::string& s){
 std::string o;for(char c:s){if(c=='"'||c=='\\'){o+='\\';o+=c;}else if(static_cast<unsigned char>(c)<0x20){char b[8];std::snprintf(b,sizeof b,"\\u%04x",c);o+=b;}else o+=c;}return o;
}
std::string Num(double v){char b[64];if(!std::isfinite(v))return "0";std::snprintf(b,sizeof b,"%.9g",v);return b;}
const char* kB64="ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
std::string B64Encode(const unsigned char* p,std::size_t n){
 std::string o;for(std::size_t i=0;i<n;i+=3){const unsigned a=p[i],b=i+1<n?p[i+1]:0,c=i+2<n?p[i+2]:0;
  o+=kB64[a>>2];o+=kB64[((a&3)<<4)|(b>>4)];o+=i+1<n?kB64[((b&15)<<2)|(c>>6)]:'=';o+=i+2<n?kB64[c&63]:'=';}return o;
}
bool B64Decode(const std::string& s,std::vector<unsigned char>* out){
 if(s.empty()||s.size()%4)return false;
 auto val=[](char c)->int{const char* p=std::strchr(kB64,c);return p?static_cast<int>(p-kB64):-1;};
 std::vector<unsigned char> o;
 for(std::size_t i=0;i<s.size();i+=4){int v[4];int pad=0;
  for(int k=0;k<4;++k){const char c=s[i+static_cast<std::size_t>(k)];if(c=='='){v[k]=0;++pad;}else{v[k]=val(c);if(v[k]<0||pad)return false;}}
  o.push_back(static_cast<unsigned char>((v[0]<<2)|(v[1]>>4)));
  if(pad<2)o.push_back(static_cast<unsigned char>(((v[1]&15)<<4)|(v[2]>>2)));
  if(pad<1)o.push_back(static_cast<unsigned char>(((v[2]&3)<<6)|v[3]));}
 *out=std::move(o);return true;
}
const char* TypeName(int t){
 if(t&kTypeFloatArray)return "float_array";if(t&kTypeIntArray)return "int_array";if(t&kTypeData)return "data";
 if((t&kTypeDouble)&&!(t&kTypeFloat))return "double";if(t&kTypeInt)return "int";return "float";
}
std::string ValueJson(const std::string& name){
 const int t=g_api->types(name.c_str());if(!t)return "null";
 if(t&kTypeFloatArray){const int n=g_api->length(name.c_str());std::vector<float> v(static_cast<std::size_t>(std::max(n,0)));
  const int got=n>0?g_api->get_floats(name.c_str(),v.data(),0,n):0;std::string o="[";
  for(int i=0;i<got;++i){if(i)o+=",";o+=Num(v[static_cast<std::size_t>(i)]);}return o+"]";}
 if(t&kTypeIntArray){const int n=g_api->length(name.c_str());std::vector<int> v(static_cast<std::size_t>(std::max(n,0)));
  const int got=n>0?g_api->get_ints(name.c_str(),v.data(),0,n):0;std::string o="[";
  for(int i=0;i<got;++i){if(i)o+=",";o+=std::to_string(v[static_cast<std::size_t>(i)]);}return o+"]";}
 if(t&kTypeData){const int n=g_api->length(name.c_str());std::vector<unsigned char> v(static_cast<std::size_t>(std::max(n,0)),0);
  const int got=n>0?g_api->get_bytes(name.c_str(),v.data(),0,n):0;
  return "\""+B64Encode(v.data(),static_cast<std::size_t>(std::max(got,0)))+"\"";}  // the real API base64s byte datarefs
 double v=0;g_api->get_scalar(name.c_str(),&v);
 if((t&kTypeInt)&&!(t&(kTypeFloat|kTypeDouble)))return std::to_string(static_cast<long long>(v));
 return Num(v);
}
// The supported request shapes are deliberately small, but must still be valid JSON.
struct DataValue { int kind=0;double num=0;std::vector<double> arr;std::string str; };  // kind 1 num 2 arr 3 str
struct JsonInput {
 const std::string& text;std::size_t at=0;
 void Space(){while(at<text.size()&&(text[at]==' '||text[at]=='\t'||text[at]=='\r'||text[at]=='\n'))++at;}
 bool Take(char c){Space();if(at>=text.size()||text[at]!=c)return false;++at;return true;}
 bool End(){Space();return at==text.size();}
 bool Hex4(unsigned* value){*value=0;for(int i=0;i<4;++i){if(at>=text.size())return false;const char c=text[at++];
   const int v=c>='0'&&c<='9'?c-'0':c>='a'&&c<='f'?c-'a'+10:c>='A'&&c<='F'?c-'A'+10:-1;
   if(v<0)return false;*value=(*value<<4)|static_cast<unsigned>(v);}return true;}
 bool String(std::string* out){
  if(!Take('"'))return false;out->clear();
  while(at<text.size()){
   char c=text[at++];if(c=='"')return true;if(static_cast<unsigned char>(c)<0x20)return false;
   if(c!='\\'){*out+=c;continue;}if(at>=text.size())return false;c=text[at++];
   if(c=='"'||c=='\\'||c=='/')*out+=c;
   else if(c=='b')*out+='\b';else if(c=='f')*out+='\f';else if(c=='n')*out+='\n';else if(c=='r')*out+='\r';else if(c=='t')*out+='\t';
   else if(c=='u'){
    unsigned u=0;if(!Hex4(&u))return false;
    if(u>=0xd800&&u<=0xdbff){if(at+2>text.size()||text[at++]!='\\'||text[at++]!='u')return false;unsigned lo=0;
     if(!Hex4(&lo)||lo<0xdc00||lo>0xdfff)return false;u=0x10000+((u-0xd800)<<10)+(lo-0xdc00);}
    else if(u>=0xdc00&&u<=0xdfff)return false;
    if(u<0x80)*out+=static_cast<char>(u);
    else if(u<0x800){*out+=static_cast<char>(0xc0|(u>>6));*out+=static_cast<char>(0x80|(u&63));}
    else if(u<0x10000){*out+=static_cast<char>(0xe0|(u>>12));*out+=static_cast<char>(0x80|((u>>6)&63));*out+=static_cast<char>(0x80|(u&63));}
    else{*out+=static_cast<char>(0xf0|(u>>18));*out+=static_cast<char>(0x80|((u>>12)&63));*out+=static_cast<char>(0x80|((u>>6)&63));*out+=static_cast<char>(0x80|(u&63));}
   }else return false;
  }
  return false;
 }
 bool Number(double* out){
  Space();const auto begin=at;const auto digit=[&]{return at<text.size()&&text[at]>='0'&&text[at]<='9';};
  if(at<text.size()&&text[at]=='-')++at;
  if(!digit())return false;if(text[at]=='0')++at;else while(digit())++at;
  if(at<text.size()&&text[at]=='.'){++at;if(!digit())return false;while(digit())++at;}
  if(at<text.size()&&(text[at]=='e'||text[at]=='E')){++at;if(at<text.size()&&(text[at]=='+'||text[at]=='-'))++at;
   if(!digit())return false;while(digit())++at;}
  const std::string token=text.substr(begin,at-begin);char* end=nullptr;*out=std::strtod(token.c_str(),&end);
  return end==token.c_str()+token.size()&&std::isfinite(*out);
 }
};
bool ParseData(const std::string& body,DataValue* out){
 JsonInput in{body};std::string key;*out={};
 if(!in.Take('{')||!in.String(&key)||key!="data"||!in.Take(':'))return false;
 in.Space();if(in.at>=body.size())return false;
 if(body[in.at]=='"'){out->kind=3;if(!in.String(&out->str))return false;}
 else if(in.Take('[')){
  out->kind=2;if(!in.Take(']'))for(;;){double n=0;if(!in.Number(&n))return false;out->arr.push_back(n);
   if(in.Take(']'))break;if(!in.Take(','))return false;}
 }else{out->kind=1;if(!in.Number(&out->num))return false;}
 return in.Take('}')&&in.End();
}
bool ParseDuration(const std::string& body,double* duration){
 JsonInput in{body};*duration=0;if(in.End())return true;if(!in.Take('{'))return false;
 if(in.Take('}'))return in.End();std::string key;
 return in.String(&key)&&key=="duration"&&in.Take(':')&&in.Number(duration)&&*duration>=0&&in.Take('}')&&in.End();
}
bool ParseId(const std::string& s,int* out){
 if(s.empty())return false;unsigned value=0;
 for(char c:s){if(c<'0'||c>'9')return false;const unsigned digit=static_cast<unsigned>(c-'0');
  if(value>(static_cast<unsigned>(std::numeric_limits<int>::max())-digit)/10)return false;value=value*10+digit;}
 *out=static_cast<int>(value);return true;
}
int ApplySet(const std::string& name,const DataValue& v){
 const int t=g_api->types(name.c_str());if(!t||!g_api->writable(name.c_str()))return 0;
 if(t&kTypeData){
  if(v.kind==3){std::vector<unsigned char> raw;
   if(B64Decode(v.str,&raw))return g_api->set_bytes(name.c_str(),raw.data(),0,static_cast<int>(raw.size()));
   return g_api->set_bytes(name.c_str(),v.str.c_str(),0,static_cast<int>(v.str.size()));}
  if(v.kind==2){std::vector<unsigned char> b;for(double d:v.arr){if(d<0||d>255||d!=std::floor(d))return 0;b.push_back(static_cast<unsigned char>(d));}
   return g_api->set_bytes(name.c_str(),b.data(),0,static_cast<int>(b.size()));}
  return 0;
 }
 if(v.kind!=1&&v.kind!=2)return 0;
 const auto numbers=v.kind==2?v.arr:std::vector<double>{v.num};
 const bool integral=(t&kTypeIntArray)||((t&kTypeInt)&&!(t&(kTypeFloat|kTypeDouble)));
 for(double d:numbers){
  if(!std::isfinite(d))return 0;
  if(integral&&(d<std::numeric_limits<int>::min()||d>std::numeric_limits<int>::max()||d!=std::floor(d)))return 0;
  if((t&(kTypeFloat|kTypeFloatArray))&&std::abs(d)>std::numeric_limits<float>::max())return 0;
 }
 if(t&kTypeFloatArray){std::vector<float> a;for(double d:(v.kind==2?v.arr:std::vector<double>{v.num}))a.push_back(static_cast<float>(d));
  return g_api->set_floats(name.c_str(),a.data(),0,static_cast<int>(a.size()));}
 if(t&kTypeIntArray){std::vector<int> a;for(double d:(v.kind==2?v.arr:std::vector<double>{v.num}))a.push_back(static_cast<int>(d));
  return g_api->set_ints(name.c_str(),a.data(),0,static_cast<int>(a.size()));}
 if(numbers.size()!=1)return 0;return g_api->set_scalar(name.c_str(),numbers[0]);
}
std::thread::id g_sim_thread;
template<typename F> auto OnSimThread(F&& f)->std::optional<decltype(f())>{
 if(std::this_thread::get_id()==g_sim_thread)return f();
 using R=decltype(f());
 struct Work { std::promise<R> promise;std::mutex mutex;bool cancelled=false; };
 auto work=std::make_shared<Work>();auto fut=work->promise.get_future();
 Enqueue([work,fn=std::forward<F>(f)]()mutable{
  {std::lock_guard<std::mutex> guard(work->mutex);if(work->cancelled)return;}
  work->promise.set_value(fn());
 });
 if(fut.wait_for(std::chrono::seconds(5))!=std::future_status::ready){
  // Cancel queued work. A call already executing may finish; its captures own their inputs.
  std::lock_guard<std::mutex> guard(work->mutex);work->cancelled=true;return std::nullopt;
 }
 return fut.get();
}
std::string SimTimeout(int* code){*code=504;return "{\"error\":\"sim thread timed out\"}";}
std::string RefNameAt(int id){const char* n=g_api->ref_name(id);return n?n:"";}
int FindRefId(const std::string& name){
 const int n=g_api->count_refs();for(int i=0;i<n;++i)if(name==RefNameAt(i))return i;return -1;
}
std::string UrlDecode(const std::string& s){
 std::string o;for(std::size_t i=0;i<s.size();++i){
  if(s[i]=='+')o+=' ';
  else if(s[i]=='%'&&i+2<s.size()){o+=static_cast<char>(std::strtol(s.substr(i+1,2).c_str(),nullptr,16));i+=2;}
  else o+=s[i];}
 return o;
}
std::string QueryValue(const std::string& q,const std::string& key){
 for(const std::string k:{std::string("filter[")+key+"]",key}){
  const auto p=q.find(k+"=");if(p==std::string::npos)continue;
  const auto e=q.find('&',p);return UrlDecode(q.substr(p+k.size()+1,e==std::string::npos?std::string::npos:e-p-k.size()-1));
 }
 return "";
}
void Respond(SOCKET s,int code,const std::string& body,bool close_after){
 char head[256];std::snprintf(head,sizeof head,"HTTP/1.1 %d %s\r\nContent-Type: application/json\r\nContent-Length: %zu\r\nConnection: %s\r\n\r\n",
  code,code==200?"OK":code==404?"Not Found":"Error",body.size(),close_after?"close":"keep-alive");
 auto send_all=[s](const char* bytes,std::size_t count){
  while(count){const int sent=send(s,bytes,static_cast<int>(std::min(count,static_cast<std::size_t>(16384))),0);
   if(sent<=0)return false;bytes+=sent;count-=static_cast<std::size_t>(sent);}return true;
 };
 if(send_all(head,std::strlen(head)))send_all(body.c_str(),body.size());
}
struct PendingEnd { std::string name;double at; };
std::mutex g_end_mu;std::vector<PendingEnd> g_ends;
std::string Route(const std::string& method,const std::string& target,const std::string& body,int* code){
 *code=200;
 std::string path=target,query;
 if(const auto q=target.find('?');q!=std::string::npos){query=target.substr(q+1);path=target.substr(0,q);}
 if(path=="/"||path.empty())return "{\"data\":\"xphost Web API v3 subset\"}";
 auto seg=[&]{std::vector<std::string> o;std::size_t a=0;while(a<path.size()){const auto b=path.find('/',a+1);o.push_back(UrlDecode(path.substr(a+1,b==std::string::npos?std::string::npos:b-a-1)));a=b==std::string::npos?path.size():b;}return o;}();
 if(seg.size()>=3&&seg[0]=="api"&&seg[1]=="v3"&&seg[2]=="datarefs"){
  if(seg.size()==3&&method=="GET"){
   const std::string want=QueryValue(query,"name");std::string o="{\"data\":[";
   const int n=g_api->count_refs();bool first=true;
   for(int i=0;i<n;++i){const std::string name=RefNameAt(i);if(name.empty())continue;
    if(!want.empty()&&name!=want)continue;
    if(!first)o+=",";first=false;
    o+="{\"id\":"+std::to_string(i)+",\"name\":\""+JsonEscape(name)+"\",\"value_type\":\""+TypeName(g_api->types(name.c_str()))+"\",\"is_writable\":"+(g_api->writable(name.c_str())?"true":"false")+"}";}
   return o+"]}";
  }
  if(seg.size()==5&&seg[4]=="value"){
   int id=0;if(!ParseId(seg[3],&id)){*code=400;return "{\"error\":\"invalid dataref id\"}";}const std::string name=RefNameAt(id);
   if(name.empty()){*code=404;return "{\"error\":\"no such dataref id\"}";}
   if(method=="GET"){
    const auto json=OnSimThread([name]{return ValueJson(name);});
    if(!json)return SimTimeout(code);
    return "{\"data\":"+*json+"}";
   }
   if(method=="PATCH"||method=="POST"){
    DataValue v;if(!ParseData(body,&v)){*code=400;return "{\"error\":\"expected {\\\"data\\\": value}\"}";}
    const auto ok=OnSimThread([name,v]{return ApplySet(name,v);});
    if(!ok)return SimTimeout(code);
    if(!*ok){*code=400;return "{\"error\":\"value type, range, or writability mismatch\"}";}
    const auto json=OnSimThread([name]{return ValueJson(name);});
    if(!json)return SimTimeout(code);
    return "{\"data\":"+*json+"}";
   }
  }
  if(seg.size()==4&&method=="GET"){
   int id=0;if(!ParseId(seg[3],&id)){*code=400;return "{\"error\":\"invalid dataref id\"}";}const std::string name=RefNameAt(id);
   if(name.empty()){*code=404;return "{\"error\":\"no such dataref id\"}";}
   return "{\"data\":{\"id\":"+std::to_string(id)+",\"name\":\""+JsonEscape(name)+"\",\"value_type\":\""+TypeName(g_api->types(name.c_str()))+"\",\"is_writable\":"+(g_api->writable(name.c_str())?"true":"false")+"}}";
  }
 }
 if(seg.size()>=3&&seg[0]=="api"&&seg[1]=="v3"&&(seg[2]=="commands"||seg[2]=="command")){
  if(seg.size()==3&&method=="GET"){
   const std::string want=QueryValue(query,"name");std::string o="{\"data\":[";
   const int n=g_api->count_commands();bool first=true;
   for(int i=0;i<n;++i){const char* nm=g_api->command_name(i);const char* ds=g_api->command_desc(i);
    const std::string name=nm?nm:"";if(name.empty())continue;if(!want.empty()&&name!=want)continue;
    if(!first)o+=",";first=false;
    o+="{\"id\":"+std::to_string(i)+",\"name\":\""+JsonEscape(name)+"\",\"description\":\""+JsonEscape(ds?ds:"")+"\"}";}
   return o+"]}";
  }
  if(seg.size()==5&&seg[4]=="activate"&&(method=="POST"||method=="PATCH")){
   int id=0;if(!ParseId(seg[3],&id)){*code=400;return "{\"error\":\"invalid command id\"}";}const char* nm=g_api->command_name(id);
   if(!nm||!*nm){*code=404;return "{\"error\":\"no such command id\"}";}
   const std::string name=nm;double duration=0;
   if(!ParseDuration(body,&duration)){*code=400;return "{\"error\":\"expected a finite nonnegative duration\"}";}
   const auto ok=OnSimThread([name,duration]{
    g_api->command(name.c_str(),duration>0?1:0);
    if(duration>0){std::lock_guard<std::mutex> g(g_end_mu);g_ends.push_back({name,g_api->sim_time()+duration});}
    return 1;
   });
   if(!ok)return SimTimeout(code);
   return "{\"data\":{}}";
  }
 }
 *code=404;return "{\"error\":\"not found: "+JsonEscape(method+" "+path)+"\"}";
}
void HandleConn(SOCKET s){
 std::string buf;char chunk[16384];
 auto fill=[&](std::size_t need)->bool{while(buf.size()<need){
  if(g_serve_stop&&g_serve_stop->load())return false;
  const int n=recv(s,chunk,sizeof chunk,0);
  if(n==SOCKET_ERROR&&WSAGetLastError()==WSAETIMEDOUT)continue;
  if(n<=0)return false;buf.append(chunk,static_cast<std::size_t>(n));if(buf.size()>262144)return false;
 }return true;};
 for(;;){  // keep-alive: one thread serves this connection's requests in turn
  std::size_t head_end=std::string::npos;
  while((head_end=buf.find("\r\n\r\n"))==std::string::npos){if(!fill(buf.size()+1))return;}
  const std::string head=buf.substr(0,head_end);
  const auto eol=head.find("\r\n");const std::string first=head.substr(0,eol);
  std::string method,target;{char m[16]{},t[4096]{};if(std::sscanf(first.c_str(),"%15s %4095s",m,t)==2){method=m;target=t;}}
  int want=0;for(const char* key:{"Content-Length:","content-length:"}){const auto cl=head.find(key);if(cl!=std::string::npos){want=std::atoi(head.c_str()+cl+15);break;}}
  if(!fill(head_end+4+static_cast<std::size_t>(std::max(want,0))))return;
  const std::string body=buf.substr(head_end+4,static_cast<std::size_t>(std::max(want,0)));
  buf.erase(0,head_end+4+body.size());
  const bool close_after=head.find("Connection: close")!=std::string::npos||head.find("connection: close")!=std::string::npos;
  const bool stop_after=g_serve_stop&&method=="POST"&&target=="/xphost/stop";
  int code=200;const std::string out=stop_after?"{\"data\":{}}":Route(method,target,body,&code);
  Respond(s,code,out,close_after);
  if(stop_after){g_serve_stop->store(true);std::printf("xphost serve: stop requested\n");std::fflush(stdout);return;}
  if(close_after)return;
 }
}
}  // namespace
int ServeSelfTest(const XpHostApi& api,double dt){
 g_api=&api;g_sim_thread=std::this_thread::get_id();
 int fails=0;auto check=[&](const char* what,bool ok){std::printf("selftest %-38s %s\n",what,ok?"PASS":"FAIL");if(!ok)++fails;};
 auto call=[&](const char* method,const std::string& target,const std::string& body=""){int code=0;std::string out=Route(method,target,body,&code);return std::pair{code,out};};
 auto find_id=[&](const std::string& list_json){const auto p=list_json.find("\"id\":");return p==std::string::npos?-1:std::atoi(list_json.c_str()+p+5);};
 auto step_seconds=[&](double s){const long long n=static_cast<long long>(s/dt);for(long long i=0;i<n;++i){DrainTasks();api.step(dt);}};
 auto lookup=[&](const std::string& name){return find_id(call("GET","/api/v3/datarefs?filter[name]="+name).second);};
 auto path=[&](const std::string& name){return "/api/v3/datarefs/"+std::to_string(lookup(name))+"/value";};
 auto scalar=[&](const std::string& name){DataValue value;const auto response=call("GET",path(name));
  return response.first==200&&ParseData(response.second,&value)&&value.kind==1?value.num:std::numeric_limits<double>::quiet_NaN();};
 const auto all=call("GET","/api/v3/datarefs");
 check("GET /datarefs lists sim refs",all.first==200&&all.second.find("sim/time/paused")!=std::string::npos);
 check("fixture ready via API",scalar("xphost/test/ready")==1);
 const double t1=scalar("sim/time/total_running_time_sec"),f1=scalar("xphost/test/frames");
 step_seconds(.25);
 check("sim time advances",scalar("sim/time/total_running_time_sec")>t1);
 check("plugin flight loop advances",scalar("xphost/test/frames")>f1);
 check("owned scalar PATCH",call("PATCH",path("xphost/test/value"),"{\"data\":3.75}").first==200&&scalar("xphost/test/value")==3.75);
 const auto bytes=path("xphost/test/bytes");
 check("owned byte PATCH/GET base64",call("PATCH",bytes,"{\"data\":\"aGVsbG8=\"}").first==200&&call("GET",bytes).second=="{\"data\":\"aGVsbG8=\"}");
 const auto array=path("xphost/test/array");
 check("owned array PATCH/GET",call("PATCH",array,"{\"data\":[4,3,2,1]}").first==200&&call("GET",array).second=="{\"data\":[4,3,2,1]}");
 const int cmd_id=find_id(call("GET","/api/v3/commands?filter[name]=xphost/test/pulse").second);
 check("command lookup by name",cmd_id>=0);
 const double begins=scalar("xphost/test/command_begin"),ends=scalar("xphost/test/command_end");
 check("command activate (singular)",call("POST","/api/v3/command/"+std::to_string(cmd_id)+"/activate","{\"duration\":0}").first==200);
 check("command Begin/End delivered",scalar("xphost/test/command_begin")==begins+1&&scalar("xphost/test/command_end")==ends+1);
 check("404 shape",call("GET","/api/v3/nope").first==404);
 check("malformed dataref id rejected",call("PATCH","/api/v3/datarefs/abc/value","{\"data\":99}").first==400);
 check("malformed command id rejected",call("POST","/api/v3/command/abc/activate","{}").first==400);
 const auto numeric=path("xphost/test/value");
 check("wrong scalar kind rejected",call("PATCH",numeric,"{\"data\":\"oops\"}").first==400&&scalar("xphost/test/value")==3.75);
 check("nonfinite number rejected",call("PATCH",numeric,"{\"data\":1e999}").first==400);
 check("invalid JSON rejected",call("PATCH",numeric,"{\"data\":[1,,2]}").first==400);
 // The sim thread deliberately does not drain work during these requests. Startup source
 // construction can block it longer than the deadline; HTTP must remain valid and queued
 // writes must not execute after their request has timed out.
 std::atomic<int> touched{0};std::optional<int> delayed;
 std::pair<int,std::string> timeout_response;
 std::thread delayed_writer([&]{delayed=OnSimThread([&]{++touched;return 1;});});
 const auto ready_path=path("xphost/test/ready");
 std::thread delayed_reader([&]{timeout_response=call("GET",ready_path);});
 delayed_writer.join();delayed_reader.join();
 check("sim-thread timeout is HTTP 504 JSON",timeout_response.first==504&&timeout_response.second=="{\"error\":\"sim thread timed out\"}");
 check("queued write reports timeout",!delayed);
 DrainTasks();check("timed-out queued write is cancelled",touched.load()==0);
 std::printf("selftest done: %d failure(s)\n",fails);
 return fails;
}
int RunServe(const XpHostApi& api,int port,double dt,bool fast,std::atomic<bool>& stop){
 g_api=&api;g_sim_thread=std::this_thread::get_id();g_serve_stop=&stop;
 WSADATA wsa{};if(WSAStartup(MAKEWORD(2,2),&wsa)){std::printf("xphost serve: WSAStartup failed\n");return 1;}
 SOCKET listener=socket(AF_INET,SOCK_STREAM,IPPROTO_TCP);
 if(listener==INVALID_SOCKET){std::printf("xphost serve: socket failed\n");WSACleanup();return 1;}
 sockaddr_in addr{};addr.sin_family=AF_INET;addr.sin_port=htons(static_cast<unsigned short>(port));
 inet_pton(AF_INET,"127.0.0.1",&addr.sin_addr);
 int reuse=1;setsockopt(listener,SOL_SOCKET,SO_REUSEADDR,reinterpret_cast<char*>(&reuse),sizeof reuse);
 if(bind(listener,reinterpret_cast<sockaddr*>(&addr),sizeof addr)||listen(listener,16)){
  std::printf("xphost serve: cannot listen on 127.0.0.1:%d (WSA error %d)\n",port,WSAGetLastError());closesocket(listener);WSACleanup();return 1;}
 u_long nonblock=1;ioctlsocket(listener,FIONBIO,&nonblock);
 std::printf("xphost serve: http://127.0.0.1:%d/api/v3 (clock %s, dt %.4f); Ctrl-C stops\n",port,fast?"fast":"real",dt);
 api.log(("serve on port "+std::to_string(port)).c_str());
 auto next=std::chrono::steady_clock::now();
 struct Worker { std::thread thread;std::shared_ptr<std::atomic<bool>> done; };
 std::mutex connections_mutex;std::vector<SOCKET> connections;std::vector<Worker> workers;
 const auto pace=fast?std::chrono::duration<double>(0.0005):std::chrono::duration<double>(dt);
 while(!stop.load()){
  for(;;){
   if(stop.load())break;
   fd_set ready;FD_ZERO(&ready);FD_SET(listener,&ready);timeval immediate{};
   if(select(0,&ready,nullptr,nullptr,&immediate)<=0)break;
   SOCKET c=accept(listener,nullptr,nullptr);if(c==INVALID_SOCKET)break;
   // Accepted Winsock sockets inherit the listener's nonblocking mode. HandleConn blocks.
   u_long blocking=0;if(ioctlsocket(c,FIONBIO,&blocking)){closesocket(c);continue;}
   // Idle keep-alive clients must not pin a worker during headless shutdown. Winsock
   // shutdown alone does not reliably wake every blocking recv on this host.
   DWORD timeout=250;
   if(setsockopt(c,SOL_SOCKET,SO_RCVTIMEO,reinterpret_cast<const char*>(&timeout),sizeof timeout)||
      setsockopt(c,SOL_SOCKET,SO_SNDTIMEO,reinterpret_cast<const char*>(&timeout),sizeof timeout)){
    closesocket(c);continue;
   }
   {std::lock_guard<std::mutex> guard(connections_mutex);connections.push_back(c);}
   auto done=std::make_shared<std::atomic<bool>>(false);
   std::thread thread([c,done,&connections,&connections_mutex]{
    HandleConn(c);
    {std::lock_guard<std::mutex> guard(connections_mutex);
     connections.erase(std::remove(connections.begin(),connections.end(),c),connections.end());
     closesocket(c);}
    done->store(true);
   });
   workers.push_back({std::move(thread),done});
  }
  // Release OS thread handles throughout long polling runs, not only on shutdown.
  workers.erase(std::remove_if(workers.begin(),workers.end(),[](Worker& worker){
   if(!worker.done->load())return false;worker.thread.join();return true;
  }),workers.end());
  if(stop.load())break;
  DrainTasks();
  {std::lock_guard<std::mutex> g(g_end_mu);const double now=api.sim_time();
   for(std::size_t i=0;i<g_ends.size();)if(now>=g_ends[i].at){api.command(g_ends[i].name.c_str(),2);g_ends.erase(g_ends.begin()+static_cast<int>(i));}else ++i;}
  api.step(dt);
  next+=std::chrono::duration_cast<std::chrono::steady_clock::duration>(pace);
  const auto now=std::chrono::steady_clock::now();
  if(next>now)std::this_thread::sleep_until(next);else if(now-next>std::chrono::milliseconds(250))next=now;
 }
 closesocket(listener);
 std::printf("xphost serve: stopping (%zu workers)\n",workers.size());std::fflush(stdout);
 {std::lock_guard<std::mutex> guard(connections_mutex);for(SOCKET c:connections)shutdown(c,SD_BOTH);}
 std::printf("xphost serve: sockets shut down\n");std::fflush(stdout);
 // Never unload the plugin/fake while a request thread still holds API pointers.
 for(auto& worker:workers)worker.thread.join();
 std::printf("xphost serve: workers joined\n");std::fflush(stdout);
 {std::lock_guard<std::mutex> guard(g_task_mu);while(!g_tasks.empty())g_tasks.pop();}
 g_serve_stop=nullptr;
 WSACleanup();
 std::printf("xphost serve: stopped\n");
 return 0;
}
