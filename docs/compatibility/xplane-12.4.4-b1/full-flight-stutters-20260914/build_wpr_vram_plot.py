"""Standalone diagnostic figure, built only after both transactions restore."""
from pathlib import Path
import csv
import json
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
import numpy as np

HERE=Path(__file__).resolve().parent
BASE=Path('D:/X-Plane 12/Output/performance-tests')

def main():
    result=json.loads((HERE/'wpr-vram-results.json').read_text())
    plt.rcParams.update({'font.family':'DejaVu Sans','font.size':10,'axes.spines.top':False,'axes.spines.right':False})
    fig,axes=plt.subplots(3,1,figsize=(11,9),gridspec_kw={'height_ratios':[1.05,1.05,1]},layout='constrained')
    colors=['#b94835','#24776b']
    for i,(run,color) in enumerate(zip(result['runs'],colors)):
        root=BASE/run['name'];flight=run['flights'][1];freq=run['frequency'];start=flight['flight_start_qpc']
        rows=list(csv.DictReader((root/'cards/calm-02/pacing.csv').open()))
        q=np.array([int(r['qpc']) for r in rows]);x=(q[1:]-start)/freq;y=np.diff(q)*1000/freq
        begin=(run['accepted_begin_qpc']-start)/freq;end=(run['accepted_end_qpc']-start)/freq
        mask=(x>=65)&(x<=125)
        axes[i].plot(x[mask],y[mask],color=color,lw=.8)
        axes[i].axhline(50,color='#8b8c8e',ls='--',lw=.8)
        axes[i].axvspan(65,begin,color='#d5d7db',alpha=.45);axes[i].axvspan(end,125,color='#d5d7db',alpha=.45)
        label='Maximum textures (5)' if i==0 else 'One step lower (4)'
        axes[i].set_title(f"{label}  ·  {run['accepted_over50']} pauses >50 ms in the accepted window",loc='left',weight='bold',pad=10)
        axes[i].set(xlim=(65,125),ylim=(0,210),ylabel='Callback interval (ms)')
        axes[i].set_xticks(range(65,126,5));axes[i].grid(axis='y',alpha=.18)
        memory=list(csv.DictReader((root/'gpu-memory/memory.csv').open()))
        mx=np.array([(int(r['receipt_qpc'])-start)/freq for r in memory]);my=np.array([int(r['free_mib'])/1024 for r in memory]);m=(mx>=65)&(mx<=125)
        axes[2].plot(mx[m],my[m],label=label,color=color,lw=2)
    axes[2].set(xlim=(65,125),ylim=(0,32),ylabel='Free board VRAM (GiB)',xlabel='Seconds after unpause · second full flight')
    axes[2].set_xticks(range(65,126,5));axes[2].grid(axis='y',alpha=.18);axes[2].legend(frameon=False,loc='lower left')
    fig.suptitle('Flag enabled, 10 GB disk texture cache: pauses and VRAM headroom',weight='bold',fontsize=15,x=.075,ha='left')
    fig.text(.075,-.025,'Gray areas are excluded trace start/stop periods. Native callbacks are not monitor scan-out.\nVRAM samples are board-wide every 500 ms; they do not measure X-Plane’s residency budget. Separate diagnostic runs include tracing overhead.',fontsize=9,color='#55565b')
    for extension in ['png','svg']:fig.savefig(HERE/f'wpr-vram-comparison.{extension}',dpi=170,bbox_inches='tight')

if __name__=='__main__':main()
