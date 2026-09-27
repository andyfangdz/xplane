"""Export the measured frame-timing comparison with individual flight variation."""
import csv
import json
from pathlib import Path
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
import numpy as np
import collect

HERE=Path(__file__).resolve().parent
data=json.loads((HERE/'results.json').read_text())
colors={'A':'#b14b3b','B':'#137d89','C':'#406cad','D':'#c78b2a'}
plt.rcParams.update({'font.family':'DejaVu Sans','font.size':10,'axes.spines.top':False,
    'axes.spines.right':False,'axes.titleweight':'bold','svg.fonttype':'none'})
fig,axs=plt.subplots(1,2,figsize=(12,4.7),layout='constrained')
labels=['No flag\n5 GB','Flag\n5 GB','Flag\n0 GB','No flag\n0 GB']
for i,key in enumerate('ABCD'):
    group=data['conditions'].get(key)
    if not group:continue
    cards=group['individual_flights']
    for ax,metric,mean_key in [(axs[0],'over_50_ms_per_minute','over_50_ms_per_minute'),
            (axs[1],'max_ms','max_ms')]:
        vals=[c[metric] for c in cards]
        x=i+np.linspace(-.13,.13,len(vals))
        ax.scatter(x,vals,color=colors[key],s=50,zorder=3,edgecolors='white',linewidth=.8)
        if ax is axs[0]:
            ax.plot([i-.27,i+.27],[group[mean_key]]*2,color=colors[key],linewidth=3)
        ax.set_xticks(range(4),labels)
        ax.set_ylim(bottom=0)
        ax.grid(axis='y',alpha=.18)
axs[0].set(title='Pauses longer than 50 ms',ylabel='Pauses per minute')
axs[0].set_ylim(bottom=-.35)
axs[1].set(title='Longest pause in each flight',ylabel='Wall interval (ms)')
fig.suptitle('X-Plane 12.4.4 beta 1 · SR20 repeatable landing test',fontsize=15,fontweight='bold')
fig.supxlabel('Each dot is one flight. Bars = pauses / total measured minutes. Reduced 2D / stock scenery.',fontsize=9)
for ext in ['png','svg']:fig.savefig(HERE/f'comparison.{ext}',dpi=160)
plt.close(fig)

fig,ax=plt.subplots(figsize=(12,7.5),layout='constrained')
names=[]
for result in data['runs']:
    key=result['label'][0]
    path=collect.ROOT/result['run']
    for card in result['cards']:
        with (path/'cards'/card['card']/'trace.csv').open(newline='') as f:rows=list(csv.DictReader(f))
        cut=next((float(r['wall_elapsed_s']) for r in rows if float(r['cut_sim_time'])>=0),0)
        points=[r for r in rows if float(r['wall_frame_dt_s'])>.05]
        y=len(names)
        names.append(result['label']+' / '+card['card'])
        ax.plot([-76,90],[y,y],color='#e9edf0',linewidth=1,zorder=0)
        ax.scatter([float(r['wall_elapsed_s'])-cut for r in points],[y]*len(points),
            s=[float(r['wall_frame_dt_s'])*600 for r in points],color=colors[key],alpha=.8,
            edgecolors='white',linewidth=.5,zorder=3)
ax.axvline(0,color='#444b55',linestyle='--',linewidth=1)
ax.set_yticks(range(len(names)),names)
ax.invert_yaxis()
ax.set(xlabel='Wall seconds from power cut',xlim=(-80,95))
fig.suptitle('Every measured pause above 50 ms',fontsize=15,fontweight='bold')
ax.set_title('Marker area scales with pause duration; each row is a complete flight.',fontsize=9,fontweight='normal',pad=10)
fig.supxlabel('A: no flag / 5 GB     B: flag / 5 GB     C: flag / 0 GB     D: no flag / 0 GB',fontsize=9)
for ext in ['png','svg']:fig.savefig(HERE/f'hitch-timeline.{ext}',dpi=160)
plt.close(fig)
