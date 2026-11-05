#pragma once
// Host <-> fake XPLM_64.dll control interface. Plain C across the boundary; the fake exports
// these XpHost* functions beside the XPLM ones and xphost.exe resolves them with GetProcAddress.
#ifdef __cplusplus
extern "C" {
#endif
struct XpHostConfig {
 const char* out_dir;       // <out>: Log.txt lives here
 const char* system_path;   // fake X-Plane root (trailing separator added by the fake if missing)
 const char* plugin_path;   // full path of win.xpl (XPLMGetPluginInfo file path)
 const char* acf_path;      // full path of the loaded .acf (XPLMGetNthAircraftModel 0)
 const char* acf_name;      // file name only
 const char* datarefs_txt;  // DataRefs.txt to seed the store (may be null)
 const char* commands_txt;  // Commands.txt to seed sim commands (may be null)
 double field_lat,field_lon,field_elev_m;  // local-frame origin (parked position)
 double magvar;             // XPLMGetMagneticVariation value: magnetic = true + magvar
 double true_heading;       // parked psi, degrees true
};
struct XpHostStats { long long frames,sim_reads,sim_writes,plugin_reads,plugin_writes,commands_fired,loops_run,find_misses; double sim_seconds; int faulted; };
// message callback: host delivers a plugin message (from XPLM_PLUGIN_XPLANE) synchronously.
typedef void (*XpHostMessageFn)(int message,void* param,void* ref);
// command observer: name, phase (0 begin 1 continue 2 end), sim time; for the command log.
typedef void (*XpHostCommandFn)(const char* name,int phase,double sim_time,void* ref);
typedef int          (*XpHost_InitFn)(const XpHostConfig*);
typedef const char*  (*XpHost_LastErrorFn)();
typedef void         (*XpHost_SetSelfInfoFn)(const char* name,const char* signature,const char* description);
typedef void         (*XpHost_SetEnabledFn)(int);
typedef void         (*XpHost_OnMessageFn)(XpHostMessageFn,void*);
typedef void         (*XpHost_OnCommandFn)(XpHostCommandFn,void*);
typedef int          (*XpHost_StepFn)(double dt);            // one frame; returns faulted
typedef double       (*XpHost_SimTimeFn)();
typedef int          (*XpHost_TypesFn)(const char* name);    // XPLMDataTypeID bits, 0 = absent
typedef int          (*XpHost_WritableFn)(const char* name);
typedef int          (*XpHost_LengthFn)(const char* name);   // array/data length, 1 for scalars, -1 absent
typedef int          (*XpHost_GetScalarFn)(const char* name,double* out);
typedef int          (*XpHost_SetScalarFn)(const char* name,double value);
typedef int          (*XpHost_GetFloatsFn)(const char* name,float* out,int offset,int count);
typedef int          (*XpHost_SetFloatsFn)(const char* name,const float* in,int offset,int count);
typedef int          (*XpHost_GetIntsFn)(const char* name,int* out,int offset,int count);
typedef int          (*XpHost_SetIntsFn)(const char* name,const int* in,int offset,int count);
typedef int          (*XpHost_GetBytesFn)(const char* name,void* out,int offset,int count);
typedef int          (*XpHost_SetBytesFn)(const char* name,const void* in,int offset,int count);
typedef int          (*XpHost_CommandFn2)(const char* name,int action);  // 0 once 1 begin 2 end
typedef int          (*XpHost_CountRefsFn)();
typedef const char*  (*XpHost_RefNameFn)(int index);         // enumeration for the Web API (id = index)
typedef int          (*XpHost_CountCommandsFn)();
typedef const char*  (*XpHost_CommandNameFn)(int index);
typedef const char*  (*XpHost_CommandDescFn)(int index);
typedef void         (*XpHost_StatsFn2)(XpHostStats*);
typedef const char*  (*XpHost_MissesFn)();                   // newline-separated FindDataRef misses
typedef void         (*XpHost_LogFn)(const char* line);      // append a host line to Log.txt
typedef int          (*XpHost_LoadNavDatFn)(const char* path);  // earth_nav.dat ingest; count or -1
struct XpHostApi {
 XpHost_InitFn init; XpHost_LastErrorFn last_error; XpHost_SetSelfInfoFn set_self_info; XpHost_SetEnabledFn set_enabled;
 XpHost_OnMessageFn on_message; XpHost_OnCommandFn on_command; XpHost_StepFn step; XpHost_SimTimeFn sim_time;
 XpHost_TypesFn types; XpHost_WritableFn writable; XpHost_LengthFn length;
 XpHost_GetScalarFn get_scalar; XpHost_SetScalarFn set_scalar;
 XpHost_GetFloatsFn get_floats; XpHost_SetFloatsFn set_floats; XpHost_GetIntsFn get_ints; XpHost_SetIntsFn set_ints;
 XpHost_GetBytesFn get_bytes; XpHost_SetBytesFn set_bytes; XpHost_CommandFn2 command;
 XpHost_CountRefsFn count_refs; XpHost_RefNameFn ref_name; XpHost_CountCommandsFn count_commands;
 XpHost_CommandNameFn command_name; XpHost_CommandDescFn command_desc; XpHost_StatsFn2 stats;
 XpHost_MissesFn misses; XpHost_LogFn log; XpHost_LoadNavDatFn load_navdat;
};
#ifdef __cplusplus
}
#endif
