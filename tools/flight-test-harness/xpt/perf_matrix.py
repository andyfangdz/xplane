"""Aggregate every fresh-process A/B attempt, including failures."""
import argparse
import statistics
from pathlib import Path
from .config import atomic_json
from .perf_report import read_json

def build(directory):
    m=read_json(directory/'matrix.json');rows=[];paired={}
    for a in m['attempts']:
        run=directory.parent/a['run'];report=read_json(run/'performance-report.json',{})
        loading=read_json(run/'load-timing.json',{})
        summaries=list(report.get('variants',{}).values())
        mean=statistics.mean(s['median_mean_ms'] for s in summaries) if summaries else None
        row={**a,'valid':report.get('complete',False) and a.get('exit_code')==0,'mean_ms':mean,
             'load_seconds':loading.get('seconds_to_paused_plugin_ready'),
             'focus_wait_seconds':loading.get('focus_wait_seconds'),
             'load_seconds_including_focus_wait':loading.get('seconds_to_gated_scene'),
             'errors':report.get('errors',{}),'report':str(run/'PERFORMANCE.md')}
        rows.append(row)
        if row['valid']:paired.setdefault(a['repeat'],{})[a['variant']]=row
    differences=[]
    for repeat,pair in paired.items():
        if 0 in pair and 1 in pair:
            differences.append({'repeat':repeat,'mean_ms_B_minus_A':pair[1]['mean_ms']-pair[0]['mean_ms'],
                'load_seconds_B_minus_A':pair[1]['load_seconds']-pair[0]['load_seconds'] if all(pair[v]['load_seconds'] is not None for v in (0,1)) else None})
    result={'compare':m['compare'],'expected_runs':m['expected_runs'],'attempts':rows,'paired_differences':differences,
            'complete':len(rows)==m['expected_runs'] and all(r['valid'] for r in rows)}
    atomic_json(directory/'comparison.json',result)
    lines=['# Fresh-process performance comparison','',
           'Each attempt starts a fresh simulator; AB/BA order alternates. Load time ends at the paused, plugin-ready scene and excludes the subsequent focus wait. Startup dialogs still contaminate load timing. Cache contents and OS caches are retained; these are not cold-disk benchmarks.','',
           '| Run | Valid | Mean interval ms | Load seconds |','|---|---|---:|---:|']
    for r in rows:lines.append(f"| [{r['run']}]({r['report']}) | {r['valid']} | {r['mean_ms']} | {r['load_seconds']} |")
    lines+=['','Paired B − A differences are saved in comparison.json. Positive values mean B was slower. Failed attempts are excluded from paired arithmetic and remain visible above.']
    (directory/'COMPARISON.md').write_text('\n'.join(lines)+'\n',encoding='utf-8')
    return result

if __name__=='__main__':
    p=argparse.ArgumentParser();p.add_argument('--run',type=Path,required=True);args=p.parse_args();result=build(args.run);print(result)
    if not result['complete']:raise SystemExit(1)
