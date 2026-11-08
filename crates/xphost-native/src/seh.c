/* Only Windows structured exception handling lives in C. Never unwind through
   Rust: each guard directly calls the foreign function inside __try. */
#include <windows.h>
#define GUARD(body) __try { body; return 0; } __except(EXCEPTION_EXECUTE_HANDLER) { return GetExceptionCode(); }
unsigned xp_start(int (*f)(char*,char*,char*), char* a,char* b,char* c,int* result) { GUARD(*result=f(a,b,c)) }
unsigned xp_enable(int (*f)(void),int* result) { GUARD(*result=f()) }
unsigned xp_void(void (*f)(void)) { GUARD(f()) }
unsigned xp_message(void (*f)(int,int,void*),int from,int msg,void* p) { GUARD(f(from,msg,p)) }
unsigned xp_loop(float (*f)(float,float,int,void*),float a,float b,int c,void* r,float* result) { GUARD(*result=f(a,b,c,r)) }
unsigned xp_command(int (*f)(void*,int,void*),void* c,int p,void* r,int* result) { GUARD(*result=f(c,p,r)) }
#define SCALAR(tag,type) \
unsigned xp_get_##tag(type (*f)(void*),void* r,type* result) { GUARD(*result=f(r)) } \
unsigned xp_set_##tag(void (*f)(void*,type),void* r,type value) { GUARD(f(r,value)) }
SCALAR(i,int)
SCALAR(f,float)
SCALAR(d,double)
#define ARRAY(tag,type) \
unsigned xp_get_##tag(int (*f)(void*,type*,int,int),void* r,type* values,int off,int count,int* result) { GUARD(*result=f(r,values,off,count)) } \
unsigned xp_set_##tag(void (*f)(void*,type*,int,int),void* r,type* values,int off,int count) { GUARD(f(r,values,off,count)) }
ARRAY(vi,int)
ARRAY(vf,float)
ARRAY(b,void)
