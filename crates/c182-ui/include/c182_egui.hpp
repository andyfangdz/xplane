#pragma once
#include <cstddef>
#include <cstdint>
#include "XPLMDisplay.h"
// Borrowed UTF-8 strings/arrays are consumed synchronously by Draw.
struct C182EguiRow {
 std::uint32_t id=0,plus=UINT32_MAX,flags=3,column=0,mask=0xffffffffu,plus_mask=0xffffffffu,tone=0;
 const char*label="",*section="",*reason="";
 double value=0.,low=0.,high=100.; // flags bit 3: slider; otherwise plus selects a stepper
};
struct C182EguiReadout {const char*text="";std::uint32_t tone=0;};
struct C182EguiPage {
 std::uint32_t tab=UINT32_MAX,section=0,ready=1;float opacity=1.f;
 const char*title="C182";const C182EguiRow*rows=nullptr;std::size_t row_count=0;
 const C182EguiReadout*readouts=nullptr;std::size_t readout_count=0;const char*report="";
};
struct C182EguiAction {std::uint32_t kind=0,id=0,flag=0,reserved=0;double value=0.;};
static_assert(sizeof(C182EguiRow)==80);
static_assert(sizeof(C182EguiReadout)==16);
static_assert(sizeof(C182EguiPage)==64);
static_assert(sizeof(C182EguiAction)==24);
extern "C" {
void*c182_egui_new()noexcept;
void c182_egui_free(void*)noexcept;
int c182_egui_draw(void*,XPLMWindowID,const C182EguiPage*,int,int,int,int)noexcept;
void c182_egui_mouse(void*,float,float,int,int)noexcept;
void c182_egui_wheel(void*,float,float,int)noexcept;
void c182_egui_key(void*,unsigned char,unsigned char,int,int)noexcept;
void c182_egui_hide(void*,XPLMWindowID)noexcept;
int c182_egui_wants_keyboard(void*)noexcept;
int c182_egui_action(void*,C182EguiAction*)noexcept;
}
namespace c182::egui_ui {
class Surface {
 void*value_=nullptr;
public:
 Surface()noexcept:value_(c182_egui_new()){}
 ~Surface(){if(value_)c182_egui_free(value_);}
 Surface(const Surface&)=delete;Surface&operator=(const Surface&)=delete;
 void Draw(XPLMWindowID w,const C182EguiPage&p,int l,int t,int r,int b)noexcept{if(value_)c182_egui_draw(value_,w,&p,l,t,r,b);}
 void Mouse(XPLMWindowID w,int x,int y,XPLMMouseStatus status,int button=0)noexcept{int l=0,t=0,r=0,b=0;XPLMGetWindowGeometry(w,&l,&t,&r,&b);if(value_)c182_egui_mouse(value_,float(x-l),float(t-y),int(status),button);}
 void Cursor(XPLMWindowID w,int x,int y)noexcept{Mouse(w,x,y,static_cast<XPLMMouseStatus>(0));}
 void Wheel(XPLMWindowID w,int x,int y,int clicks)noexcept{int l=0,t=0,r=0,b=0;XPLMGetWindowGeometry(w,&l,&t,&r,&b);if(value_)c182_egui_wheel(value_,float(x-l),float(t-y),clicks);}
 void Key(char key,char virtual_key,int flags,int losing)noexcept{if(value_)c182_egui_key(value_,static_cast<unsigned char>(key),static_cast<unsigned char>(virtual_key),flags,losing);}
 void Hide(XPLMWindowID w)noexcept{if(value_)c182_egui_hide(value_,w);}
 bool Keyboard()const noexcept{return value_&&c182_egui_wants_keyboard(value_)!=0;}
 bool Action(C182EguiAction&out)noexcept{return value_&&c182_egui_action(value_,&out)!=0;}
};
}
