#pragma warning disable CS0618
using System;
using System.Collections.Generic;
using System.Text.Json;
using Microsoft.Diagnostics.Tracing;

public static class EtlHitchProbe {
 public static string Run(string path, int targetPid, long begin, long end) {
  var processes = new Dictionary<int,string>();
  var threads = new Dictionary<int,object>();
  var threadPids = new Dictionary<int,int>();
  var images = new List<object>();
  var events = new List<object>();
  var stacks = new List<object>();
  var fileNames = new Dictionary<ulong,string>();
  var fileStats = new Dictionary<string,long[]>();
  var cpuThread = new Dictionary<int,int>();
  using (var s = new ETWTraceEventSource(path)) {
   Action<Microsoft.Diagnostics.Tracing.Parsers.Kernel.ProcessTraceData> proc = e => processes[e.ProcessID]=e.ImageFileName;
   Action<Microsoft.Diagnostics.Tracing.Parsers.Kernel.ThreadTraceData> thread = e => {
    threadPids[e.ThreadID]=e.ProcessID;
    if(e.ProcessID==targetPid) threads[e.ThreadID]=new {tid=e.ThreadID,pid=e.ProcessID,name=e.ThreadName,start=e.StartAddr,win32start=e.Win32StartAddr};
   };
   Action<Microsoft.Diagnostics.Tracing.Parsers.Kernel.ImageLoadTraceData> img = e => {
    if(e.ProcessID==targetPid || e.ProcessID==0 || e.ProcessID==4) images.Add(new {pid=e.ProcessID,address=e.ImageBase,size=e.ImageSize,file=e.FileName,checksum=e.ImageChecksum,timestamp=e.TimeDateStamp});
   };
   s.Kernel.ProcessStart += proc; s.Kernel.ProcessDCStop += proc;
   s.Kernel.ThreadStart += thread; s.Kernel.ThreadDCStop += thread;
   s.Kernel.ImageLoad += img; s.Kernel.ImageDCStop += img;
   s.Kernel.FileIOFileRundown += e => fileNames[e.FileKey]=e.FileName;
   s.Kernel.FileIOName += e => fileNames[e.FileKey]=e.FileName;
   s.Process();
  }
  int Pid(TraceEvent e) => threadPids.TryGetValue(e.ThreadID,out int p) ? p : e.ProcessID;
  using (var s = new ETWTraceEventSource(path)) {
   s.Kernel.StackWalkStack += e => {
    if(e.EventTimeStampQPC<begin || e.EventTimeStampQPC>end) return;
    var ips=new ulong[e.FrameCount]; for(int i=0;i<ips.Length;i++) ips[i]=e.InstructionPointer(i);
    stacks.Add(new {event_qpc=e.EventTimeStampQPC,receipt_qpc=e.TimeStampQPC,tid=e.ThreadID,pid=Pid(e),ips});
   };
   s.Kernel.All += e => {
    if(e.EventName=="Thread/CSwitch")cpuThread[e.ProcessorNumber]=Convert.ToInt32(e.PayloadByName("NewThreadID"));
    if(e.TimeStampQPC<begin || e.TimeStampQPC>end) return;
    string n=e.EventName;
    if(n=="FileIO/Read" || n=="FileIO/Write") {
     ulong key=Convert.ToUInt64(e.PayloadByName("FileKey"));
     string file=fileNames.TryGetValue(key,out string f) ? f : key.ToString("x");
     string k=$"{Pid(e)}|{n}|{file}";
     if(!fileStats.ContainsKey(k))fileStats[k]=new long[]{0,0};
     fileStats[k][0]++;fileStats[k][1]+=Convert.ToInt64(e.PayloadByName("IoSize"));return;
    }
    if(!(n=="PerfInfo/Sample" || n=="Thread/CSwitch" || n=="Dispatcher/ReadyThread" || n=="Memory/HardFault" || n.StartsWith("DiskIO/") || n.Contains("DPC") || n.Contains("ISR"))) return;
    var fields=new Dictionary<string,object>();
    for(int i=0;i<e.PayloadNames.Length;i++) {
     object v=e.PayloadValue(i);
     fields[e.PayloadNames[i]]=v is Enum ? v.ToString() : v;
    }
    int actor=cpuThread.TryGetValue(e.ProcessorNumber,out int at) ? at : -1;
    events.Add(new {qpc=e.TimeStampQPC,cpu=e.ProcessorNumber,pid=Pid(e),tid=e.ThreadID,scheduled_tid=actor,name=n,fields});
   };
   s.Process();
   return JsonSerializer.Serialize(new {target_pid=targetPid,begin_qpc=begin,end_qpc=end,events_lost=s.EventsLost,processes,threads,thread_pids=threadPids,images,events,stacks,file_stats=fileStats});
  }
 }
}
