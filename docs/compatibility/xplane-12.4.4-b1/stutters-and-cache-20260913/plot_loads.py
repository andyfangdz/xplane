"""Export measured first-load and in-process reload times separately."""
import json
from pathlib import Path
from statistics import mean
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
import numpy as np

HERE=Path(__file__).resolve().parent
rows=json.loads((HERE/'load-results.json').read_text())['rows']
colors={'A':'#b14b3b','B':'#137d89','C':'#406cad','D':'#c78b2a'}
plt.rcParams.update({'font.family':'DejaVu Sans','font.size':10,'axes.spines.top':False,
    'axes.spines.right':False,'axes.titleweight':'bold','svg.fonttype':'none'})
fig,axs=plt.subplots(1,2,figsize=(12,4.7),layout='constrained')
labels=['No flag\n5 GB','Flag\n5 GB','Flag\n0 GB','No flag\n0 GB']
for ax,kind in zip(axs,['first_in_process','same_process_reload']):
    for i,key in enumerate('ABCD'):
        values=[r['load_to_first_resume_seconds'] for r in rows if r['run'][0]==key and r['load_kind']==kind]
        if not values:continue
        x=i+np.linspace(-.12,.12,len(values))
        ax.scatter(x,values,s=55,color=colors[key],edgecolors='white',linewidth=.8,zorder=3)
        ax.plot([i-.24,i+.24],[mean(values)]*2,linewidth=3,color=colors[key])
    ax.set_xticks(range(4),labels)
    ax.set_ylim(bottom=0)
    ax.grid(axis='y',alpha=.18)
    ax.set_ylabel('Load request to first successful unpause (seconds)')
axs[0].set_title('First load in a fresh simulator process')
axs[1].set_title('Reload in the same simulator process')
fig.suptitle('Texture cache · measured flight loading',fontsize=15,fontweight='bold')
fig.supxlabel('Each dot is one load; bars show means. Same stock scene and reduced 2D profile; OS caches not cleared.',fontsize=9)
for ext in ['png','svg']:fig.savefig(HERE/f'load-comparison.{ext}',dpi=160)
