"""Standalone plots, repeat overlays, divergence flags, and restoration summary."""
import csv
import html
import json
from collections import defaultdict
from pathlib import Path
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
import numpy as np
from .config import atomic_json

def read(path):return json.loads(path.read_text(encoding='utf-8-sig'))

def load_rows(path):
    with path.open(encoding='utf-8') as stream:
        return [{k:float(v) for k,v in r.items()} for r in csv.DictReader(stream)]

def aligned(rows):
    cut=next((r.get('cut_sim_time') for r in rows if r.get('cut_sim_time',-1)>=0),None)
    if cut is None:return []
    return [dict(r,elapsed=r['sim_time']-cut,physical_sink_fpm=-r['vertical_speed_mps']*196.850394) for r in rows if r['sim_time']>=cut]

def baseline_traces(directory,card):
    if not directory or not directory.exists():return []
    if (directory/'cards').exists():
        return [aligned(load_rows(path)) for path in sorted((directory/'cards').glob(card+'-*/trace.csv'))]
    manifest=directory/'frozen-manifest.json';version=read(manifest)['version'] if manifest.exists() else None
    traces=[]
    for path in sorted(directory.glob(card+'-val*-raw.json')):
        d=read(path)
        if version and d.get('parameters',{}).get('version')!=version:continue
        if not d.get('cut'):continue
        cut=d['cut']['sim_time']
        traces.append([dict(r,elapsed=r['sim_time']-cut,physical_sink_fpm=-r['vertical_speed_mps']*196.850394) for r in d['trace'] if r['sim_time']>=cut])
    return traces

def divergence(traces):
    valid=[r for r in traces if len(r)>2]
    if len(valid)<2:return {'repeat_count':len(valid),'flags':['Fewer than two traces; repeatability is not established']}
    end=min(r[-1]['elapsed'] for r in valid);grid=np.arange(0,end,.1);out={'repeat_count':len(valid),'flags':[]}
    for field,threshold in [('ias_kias',3),('pitch_deg',2),('runway_cross_ft',50),('physical_sink_fpm',150)]:
        values=np.array([np.interp(grid,[r['elapsed'] for r in rows],[r[field] for r in rows]) for rows in valid])
        spread=np.ptp(values,axis=0);i=int(np.argmax(spread))
        out[field]={'maximum_spread':float(spread[i]),'seconds_from_cut':float(grid[i]),'rms_spread':float(np.sqrt(np.mean(spread**2)))}
        if spread[i]>threshold:out['flags'].append(f'{field} spread {spread[i]:.1f} at {grid[i]:.1f}s exceeds diagnostic threshold {threshold}')
    return out

def build(directory, baseline=None):
    directory=Path(directory);charts=directory/'charts';charts.mkdir(exist_ok=True)
    groups=defaultdict(list);trial_rows=[];limits={}
    for path in sorted((directory/'cards').glob('*/assessment.json')) if (directory/'cards').exists() else []:
        assessment=read(path);result=read(path.parent/'result.json');card=result['effective_config']['card_name']
        rows=aligned(load_rows(path.parent/'trace.csv'))
        groups[card].append((path.parent.name,assessment,rows))
        limits[card]=result['effective_config']['acceptance']
        trial_rows.append({'label':path.parent.name,**assessment})
    comparisons={};figures=[]
    plt.rcParams.update({'font.family':'DejaVu Sans','font.size':10,'axes.spines.top':False,'axes.spines.right':False})
    for card,runs in groups.items():
        comparisons[card]=divergence([rows for _,_,rows in runs])
        fig,axes=plt.subplots(2,2,figsize=(12,8),constrained_layout=True)
        for index,rows in enumerate(baseline_traces(baseline,card)):
            if not rows:continue
            for ax,x,y in [(axes[0,0],'runway_along_ft','runway_cross_ft'),(axes[0,1],'elapsed','ias_kias'),(axes[1,0],'elapsed','pitch_deg'),(axes[1,1],'elapsed','physical_sink_fpm')]:
                previous='Previous native guidance' if (baseline/'cards').exists() else 'Previous HTTP guidance'
                ax.plot([r[x] for r in rows],[r[y] for r in rows],color='#a1a9af',alpha=.6,lw=1,label=previous if index==0 else None)
        colors=['#007d86','#c45f26','#61458b','#287342']
        for index,(label,assessment,rows) in enumerate(runs):
            if not rows:continue
            for ax,x,y in [(axes[0,0],'runway_along_ft','runway_cross_ft'),(axes[0,1],'elapsed','ias_kias'),(axes[1,0],'elapsed','pitch_deg'),(axes[1,1],'elapsed','physical_sink_fpm')]:
                ax.plot([r[x] for r in rows],[r[y] for r in rows],lw=1.6,color=colors[index%len(colors)],label=label)
            if 'touchdown_ft' in assessment['metrics']:
                m=assessment['metrics'];axes[0,0].scatter([m['touchdown_ft']],[m['cross_ft']],color=colors[index%len(colors)],s=32,zorder=4)
        axes[0,0].axhline(0,color='#3a4650',lw=.8)
        axes[0,0].axvspan(limits[card]['touchdown_min_ft'],limits[card]['touchdown_max_ft'],color='#c4e3d4',alpha=.5)
        labels=[('Ground path','Along runway, ft','Cross runway, ft'),('Airspeed','Seconds after power cut','KIAS'),('Pitch','Seconds after power cut','Degrees'),('Physical descent','Seconds after power cut','fpm, positive down')]
        for ax,(title,x,y) in zip(axes.flat,labels):
            ax.set(title=title,xlabel=x,ylabel=y);ax.grid(alpha=.18)
        axes[0,1].legend(fontsize=8);fig.suptitle(f'{card.replace("_"," ")} — native guidance repeat overlay',fontsize=16)
        for ext in ('png','svg'):fig.savefig(charts/f'{card}.{ext}',dpi=150)
        plt.close(fig);figures.append(card)
    restored=read(directory/'restoration.json') if (directory/'restoration.json').exists() else {'restored':False}
    errors=[]
    for path in [directory/'runner-error.json',directory/'worker-error.json',directory/'recovery-error.json',directory/'owner-loss.json']:
        if path.exists():errors.append(read(path))
    summary={'schema_version':1,'flights':len(trial_rows),'strict_passes':sum(r['passed'] for r in trial_rows),
             'measurement_valid':sum(r['measurement_valid'] for r in trial_rows),'restoration':restored,
             'comparisons':comparisons,'errors':errors,'trials':trial_rows}
    atomic_json(directory/'summary.json',summary)
    lines=['# Native X-Plane harness report','',f'**{summary["strict_passes"]}/{summary["flights"]} flights passed the landing limits.** '
           f'Measurement-valid flights: {summary["measurement_valid"]}. Installation restored: {restored.get("restored",False)}.','',
           'Landing pass/fail is separate from harness operation. Native guidance runs on simulator frames; Python only supervises it. '
           'All attempts, including aborted and non-passing flights, remain in this report.','',
           '| Trial | Touchdown ft | KIAS | Physical sink fpm | Peak pitch ° | Peak pitch rate °/s | Result |',
           '|---|---:|---:|---:|---:|---:|---|']
    for row in trial_rows:
        m=row['metrics'];fmt=lambda k:f'{m[k]:.1f}' if m.get(k) is not None else '—'
        lines.append(f'| {row["label"]} | {fmt("touchdown_ft")} | {fmt("kias")} | {fmt("physical_sink_fpm")} | '
                     f'{fmt("max_flare_pitch_deg")} | {fmt("max_flare_pitch_rate_deg_s")} | {"Pass" if row["passed"] else "; ".join(row["reasons"])} |')
    for card in figures:
        lines+=['',f'## {card.replace("_"," ")}', '',f'![Flight-path, speed, pitch and descent overlays](charts/{card}.png)','']
        lines.extend('- '+flag for flag in comparisons[card]['flags'])
        if not comparisons[card]['flags']:lines.append('No repeat-divergence diagnostic threshold was exceeded.')
    lines+=['','## Evidence','',
            '- `resolved-config.json` and each card’s `effective-config.json`: complete settings, with no inactive legacy fields.',
            '- `native-effective.ini`: exact configuration read back from the plugin before flight.',
            '- `trace.csv`: native-frame observations; `supervision.json`: independent polling and deliberate gap probes.',
            '- `source-manifest.json`: harness, native binary, setup adapter and original aircraft lineage.',
            '- `events.jsonl`, `status.json`: structured progress; `restoration.json`: recovery verification.',
            '- `summary.json`: metrics and repeat-divergence timestamps; `charts/*.svg`: standalone vector figures.','']
    if errors:lines+=['## Execution errors','', '```json',json.dumps(errors,indent=2),'```','']
    (directory/'report.md').write_text('\n'.join(lines),encoding='utf-8',newline='\n')
    if (directory/'status.json').exists():
        status=read(directory/'status.json');status.update(phase='restored' if restored.get('restored') else 'recovery_pending',restoration=restored,summary={'flights':summary['flights'],'strict_passes':summary['strict_passes']})
        atomic_json(directory/'status.json',status)
    return summary
