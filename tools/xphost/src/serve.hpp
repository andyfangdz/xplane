#pragma once
#include "xphost_api.h"
#include <atomic>
// Runs the X-Plane 12 Web API v3 subset (datarefs list/value, commands list/activate) on
// 127.0.0.1:port while stepping the plugin on the calling thread. Returns on stop.
int RunServe(const XpHostApi& api,int port,double dt,bool fast,std::atomic<bool>& stop);
// Exercises request routing against the bundled xphost/test fixture without sockets.
// Runs on the calling (sim) thread; returns the number of failed checks.
int ServeSelfTest(const XpHostApi& api,double dt);
