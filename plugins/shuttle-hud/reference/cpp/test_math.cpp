#include "hud_math.hpp"
#undef NDEBUG
#include <cassert>
#include <iostream>
int main(){
 using namespace shud;
 assert(std::abs(eas(100,1.225)-194.3844492)<1e-6);
 assert(std::abs(eas(100,1.225*.25)-97.1922246)<1e-6);
 assert(clutter(3999,true,-1)==2&&clutter(4000,true,-1)==1&&clutter(10000,true,-1)==0&&clutter(6000,false,-1)==0);
 assert(clutter(200,true,0)==0&&clutter(200,true,1)==1);
 auto p=project(238,-10,238,-10,0,1000,1000,{960,300});assert(std::abs(p.p.x-960)<1e-8&&std::abs(p.p.y-300)<1e-8);
 p=project(243,-10,238,-10,0,1000,1000,{960,300});assert(p.p.x>960);
 p=project(238,-20,238,-10,0,1000,1000,{960,300});assert(p.p.y>300);
 p=constrain({{3000,-20},false},500,100,1400,900);assert(p.limited&&p.p.x==1400&&p.p.y==100);
 auto q=rotate({100,0},{0,0},90);assert(std::abs(q.x)<1e-8&&std::abs(q.y-100)<1e-8);
 assert(speedTapeY(270,280)>speedTapeY(270,270)&&speedTapeY(270,260)<535);
 assert(speedText(201,180,false)=="201"&&speedText(201,180,true)=="G180");
 assert(matches("a/Orbiter_Glider.acf","a/Orbiter_Glider.acf")&&!matches("b/Orbiter_Glider.acf","a/Orbiter_Glider.acf")&&!matches("",""));
 assert(advanceFade(0,12,0,true)==5); // Same aircraft, new flight on final.
 assert(advanceFade(5,12,0,false)==0); // A reset outside prefinal clears the circle.
 assert(advanceFade(2.5,12,12,true)==2.5); // Pause preserves a continuous transition.
 assert(std::abs(advanceFade(2.5,12,12.1,true)-2.6)<1e-9);
 assert(std::abs(advanceFade(2.5,12,12.1,false)-2.4)<1e-9);
 std::cout<<"PASS: EAS, declutter boundaries, projection/drift, off-screen limits, rotation, WOW speed and exact aircraft identity\n";
}
