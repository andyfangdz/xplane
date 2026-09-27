"""Export callback-interval plots for the capture-free texture comparison."""
import csv
import json
from pathlib import Path
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt

BASE=Path('D:/X-Plane 12/Output/performance-tests')
OUT=Path(__file__).resolve().parent
RUNS=[('XPT_USER_FLAG10_TEX4_20260914','Texture detail 4 / 5'),
      ('XPT_USER_FLAG10_CONTROL_20260914','Saved maximum 5 / 5')]

if __name__=='__main__':
    fig,axes=plt.subplots(2,2,figsize=(12,6.4),sharex=True,sharey=True)
    maximum=0
    for i,(run,label) in enumerate(RUNS):
        for j,card in enumerate(['calm-01','calm-02']):
            rows=list(csv.DictReader((BASE/run/'cards'/card/'trace.csv').open()))
            x=[float(r['wall_elapsed_s']) for r in rows[1:]]
            y=[float(r['wall_frame_dt_s'])*1000 for r in rows[1:]]
            maximum=max(maximum,max(y))
            ax=axes[i,j]
            ax.plot(x,y,color=['#007d8a','#8256ab','#007d8a'][i],lw=.65)
            ax.axhline(50,color='#c95834',lw=1,ls='--')
            ax.axhline(100,color='#ad3a34',lw=.8,ls=':')
            ax.set_title(f'{label} · flight {j+1}',loc='left',fontsize=11)
            ax.text(.02,.95,f'{sum(v>50 for v in y)} >50 ms | max {max(y):.1f} ms',
                    transform=ax.transAxes,va='top',fontsize=10)
            ax.grid(axis='y',alpha=.18)
            ax.spines[['top','right']].set_visible(False)
            ax.set_ylim(bottom=0)
            if i==0 and j==0:
                ax.axvspan(0,30,color='#555555',alpha=.10)
                ax.text(.02,.82,'Opening UI activity: diagnostic flight',transform=ax.transAxes,fontsize=8,color='#555555')
    axes[0,0].set_ylim(0,maximum*1.12)
    for ax in axes[:,0]: ax.set_ylabel('Callback interval (ms)')
    for ax in axes[-1]: ax.set_xlabel('Wall time since maneuver start (s)')
    fig.suptitle('Full scenery, 2D · flag enabled · 10 GB cache · no live profiler dumps',fontsize=14,y=.99)
    fig.text(.01,.01,'Native callback timing; scene loading excluded, activation intervals retained. Sequential runs; evolving cache.',fontsize=9,color='#555555')
    fig.tight_layout(rect=(0,.04,1,.95))
    for ext in ['png','svg']:fig.savefig(OUT/f'texture-comparison.{ext}',dpi=170)
