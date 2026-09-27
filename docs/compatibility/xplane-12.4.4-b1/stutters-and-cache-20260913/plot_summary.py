"""Show the two measured effects with their separate sample counts."""
import json
from pathlib import Path
from statistics import mean
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
import numpy as np

HERE=Path(__file__).resolve().parent
data=json.loads((HERE/'results.json').read_text())
capacity=json.loads((HERE/'cache-capacity-results.json').read_text())
plt.rcParams.update({'font.family':'DejaVu Sans','font.size':11,'axes.spines.top':False,
    'axes.spines.right':False,'axes.titleweight':'bold','svg.fonttype':'none'})
fig,axs=plt.subplots(1,2,figsize=(11,4.7),layout='constrained')
for i,(flag,keys,color) in enumerate([(False,'AD','#b14b3b'),(True,'BC','#137d89')]):
    cards=[c for k in keys for c in data['conditions'][k]['individual_flights']]
    values=[c['over_50_ms_per_minute'] for c in cards]
    rate=sum(c['over_50_ms'] for c in cards)*60/sum(c['observed_wall_seconds'] for c in cards)
    axs[0].scatter(i+np.linspace(-.13,.13,len(values)),values,color=color,s=45,edgecolors='white',linewidth=.6,zorder=3)
    axs[0].plot([i-.28,i+.28],[rate]*2,color=color,linewidth=3)
axs[0].set_xticks([0,1],['Flag absent','Flag enabled'])
axs[0].set(title='Pauses longer than 50 ms',ylabel='Pauses per minute',ylim=(-.35,None))
axs[0].text(.5,-.22,'6 flights per setting · bars show overall rates',transform=axs[0].transAxes,ha='center',fontsize=9)
for i,(cache,color) in enumerate([(5,'#667586'),(10,'#137d89')]):
    values=[r['load_to_first_resume_seconds'] for r in capacity['rows'] if r['cache_gb']==cache]
    axs[1].scatter(i+np.linspace(-.1,.1,len(values)),values,color=color,s=55,edgecolors='white',linewidth=.6,zorder=3)
    axs[1].plot([i-.28,i+.28],[mean(values)]*2,color=color,linewidth=3)
    axs[1].text(i,mean(values)+1.7,f'{mean(values):.1f} s',ha='center',fontweight='bold',color=color)
axs[1].set_xticks([0,1],['5 GB cache','10 GB cache'])
axs[1].set(title='Flight loading with the flag enabled',ylabel='Load request to first unpause (seconds)',ylim=(0,41))
axs[1].text(.5,-.22,'2 fresh-process loads per capacity · bars show means',transform=axs[1].transAxes,ha='center',fontsize=9)
for ax in axs:ax.grid(axis='y',alpha=.18)
fig.suptitle('X-Plane 12.4.4 beta 1 · measured results',fontsize=16,fontweight='bold')
fig.supxlabel('TorqueSim SR20 at KCDW · reduced 2D graphics and stock scenery · every dot is a retained trial',fontsize=9)
for ext in ['png','svg']:fig.savefig(HERE/f'summary-comparison.{ext}',dpi=170)
