// Frozen C++ v7 reference. This is an offline fixture generator, never loaded
// into X-Plane. Each frame contains 18 protocol inputs and 18 f64 outputs.
#include "guidance.hpp"
#include <fstream>
#include <iostream>
#include <vector>
int main(int argc,char**argv){
 if(argc!=4)return 2;
 std::ifstream config(argv[1]),csv(argv[2]);std::ofstream binary(argv[3],std::ios::binary);
 std::stringstream text;text<<config.rdbuf();xpt::Config c;std::string error;
 if(!c.parse(text.str(),error)){std::cerr<<error;return 3;}
 xpt::Controller control(c);std::string line;std::getline(csv,line);bool first=true;
 const int indices[]={0,1,2,3,4,5,6,7,11,15,16,23,24,25,26,29,30,31};
 unsigned frames=0;
 while(std::getline(csv,line)){
  std::istringstream row(line);std::string cell;float s[75];int i=0;
  while(std::getline(row,cell,',')&&i<75)s[i++]=std::stof(cell);
  if(i!=75)return 4;
  if(first){control.start(s[0]);first=false;}
  xpt::Sample sample;
  sample.t=s[0];sample.x=s[29];sample.y=s[30];sample.h=s[1];sample.ias=s[2];sample.tas_fps=s[26]*3.280839895;
  sample.gs_fps=s[3]*1.68780986;sample.heading=s[5];sample.track=s[31];sample.bank=s[6];sample.pitch=s[7];sample.vvi=s[4];sample.vy=s[23]*3.280839895;
  sample.wind_kt=s[24]*1.94384449;sample.wind_dir=s[25];sample.mass_lb=s[16]*2.20462262185;sample.throttle=s[11];sample.ground=s[15]!=0;
  control.step(sample);
  for(int index:indices)binary.write(reinterpret_cast<char*>(&s[index]),4);
  const double values[]={double(control.phase),double(control.reason),control.bank,control.pitch,control.flap,control.throttle,
   control.lead,control.desired,control.accel,control.wind_ff,control.pitch_rate,control.dt,control.cut_t,control.roundout_t,
   control.gate_s,double(control.steps),control.predicted_cross,control.cross_accel};
  binary.write(reinterpret_cast<const char*>(values),sizeof(values));++frames;
 }
 if(!binary||control.phase!=xpt::Complete)return 5;
 std::cout<<frames<<" reference frames\n";
}
