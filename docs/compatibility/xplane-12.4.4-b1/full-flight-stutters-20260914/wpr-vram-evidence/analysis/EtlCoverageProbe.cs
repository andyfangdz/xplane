#pragma warning disable CS0618
using System;
using System.Collections.Generic;
using Microsoft.Diagnostics.Tracing;
public static class EtlCoverageProbe {
 public static string Run(string path) {
  var stats=new Dictionary<string,long[]>();
  using(var source=new ETWTraceEventSource(path)) {
   source.Kernel.All += e => { var k=e.EventName; if(!stats.ContainsKey(k))stats[k]=new long[]{0,e.TimeStampQPC,e.TimeStampQPC}; var s=stats[k];s[0]++;s[1]=Math.Min(s[1],e.TimeStampQPC);s[2]=Math.Max(s[2],e.TimeStampQPC); };
   source.Process();
   return System.Text.Json.JsonSerializer.Serialize(new {events_lost=source.EventsLost,duration_seconds=source.SessionDuration.TotalSeconds,kernel=stats});
  }
 }
}