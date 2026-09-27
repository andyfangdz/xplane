from pathlib import Path
import json
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt

H=Path(__file__).resolve().parent
def read(p):return json.loads(p.read_text(encoding='utf-8-sig'))
s=read(H/'trace-summary.json');main=read(H/'hitch-analysis.json');workers=read(H/'worker-analysis.json')
fig,axes=plt.subplots(2,1,figsize=(11,6.4),sharex=True)
for i,ax in enumerate(axes):
    c=s['cases'][i];m=main[i]['main_waits'][0];w=workers[i]['worker_waits'][0]
    offset=lambda t:(t-c['start_qpc'])*1000/s['frequency']
    bars=[(3,0,c['ms'],'#3577a9','Native frame interval'),
          (2,offset(m['start_qpc']),m['full_wait_ms'],'#dd8c39','Main thread: NVIDIA lock'),
          (1,offset(w['begin']),w['full_wait_ms'],'#8c68b1','Background worker: memory lock')]
    for y,start,width,color,label in bars:
        ax.barh(y,width,left=start,height=.47,color=color)
        ax.text(start+width/2,y,f'{width:.1f} ms',ha='center',va='center',color='white',fontsize=10,fontweight='bold')
    ax.scatter([offset(w['ready_qpc'])],[.25],color='#5e357e',marker='v',s=65,zorder=4)
    ax.annotate('Windows memory worker releases backing-store lock',
        (offset(w['ready_qpc']),.25),xytext=(2,-.55),fontsize=9,color='#433650',
        arrowprops=dict(arrowstyle='->',color='#5e357e',lw=1))
    ax.set_yticks([1,2,3],['Background worker waits','Main thread waits','Recorded frame'])
    ax.set_title(f'Pause {i+1}: {c["ms"]:.1f} ms, {c["seconds_from_unpause"]:.2f} s into flight 2',loc='left',fontsize=11,fontweight='bold')
    ax.set_ylim(-.9,3.75);ax.set_xlim(-5,160)
    ax.grid(axis='x',alpha=.2);ax.set_axisbelow(True)
    for spine in ('top','right','left'):ax.spines[spine].set_visible(False)
axes[-1].set_xlabel('Milliseconds relative to the start of the long native interval')
fig.suptitle('Graphics-memory preparation stalls X-Plane’s main thread',x=.03,ha='left',fontsize=15,fontweight='bold')
fig.text(.03,.91,'HAGS off • flag on • 10 GB cache • maximum textures • temporary DXGI presentation',fontsize=10,color='#444444')
fig.text(.03,.025,'Measured waits are nested; their durations must not be added. Exact QPC and ReadyThread events link both releases.\nNVIDIA board-memory samples nearby: 18.3 GiB free (pause 1), 15.5 GiB free (pause 2).',fontsize=9,color='#444444')
fig.subplots_adjust(left=.2,right=.98,top=.85,bottom=.15,hspace=.45)
fig.savefig(H/'blocking-chain.png',dpi=170)
fig.savefig(H/'blocking-chain.svg')
plt.close(fig)
print('Saved blocking-chain.png and .svg')
