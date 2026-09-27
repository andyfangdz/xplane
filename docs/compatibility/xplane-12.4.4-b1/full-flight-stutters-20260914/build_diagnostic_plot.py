"""Offline plot of the first verified, pre-request flag-on hitch."""
import json
from pathlib import Path
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt

HERE = Path(__file__).resolve().parent
RUN = Path('D:/X-Plane 12/Output/performance-tests/XPT_FLAG_ON_MICROPROFILE_20260914')
p = json.loads((RUN / 'microprofile/calm-02.analysis.json').read_text())
f = p['frames'][238]
origin = f['start_ms']
scopes = p['scopes']

fig, ax = plt.subplots(figsize=(11, 6.5), layout='constrained')
fig.patch.set_facecolor('#f7f9fb')
ax.set_facecolor('#ffffff')
selection = [
    ('Update / render (parent)', 'Main Thread', 'App Update', '#a7b6c9'),
    ('Main render stage', 'Main Thread', 'Main Frame - Main Window - main thread', '#df743c'),
    ('Presentation', 'Main Thread', 'Swapchain present', '#735cab'),
]
selection += [(f'Texture upload · worker {n}', f'Pool worker (background - {n})', 'Upload', '#148b86')
              for n in [6, 21, 13, 22, 8, 11]]
retained = []
for y, (label, thread, timer, color) in enumerate(selection):
    candidates = [s for s in scopes if s['thread'] == thread and s['timer'] == timer
                  and s['start_ms'] >= origin and s['start_ms'] < f['end_ms']]
    if y == 1:
        candidates = [max(candidates, key=lambda s: s['duration_ms'])]
    for s in candidates:
        ax.broken_barh([(s['start_ms']-origin, s['duration_ms'])], (y-.29, .58),
                       facecolors=color)
        retained.append(s)
        ax.annotate(f"{s['duration_ms']:.2f} ms", (s['end_ms']-origin, y),
                    xytext=(5, 0), textcoords='offset points', va='center', fontsize=9,
                    color='#25364a')
end = next(s['end_ms'] for s in retained if s['timer'].startswith('Main Frame')) - origin
ax.axvline(end, color='#455b70', linestyle='--', linewidth=1, alpha=.7)
ax.set_yticks(range(len(selection)), [s[0] for s in selection])
ax.invert_yaxis()
ax.set_xlim(-1, 60)
ax.set_xlabel('Elapsed milliseconds from profiler frame start')
ax.grid(axis='x', alpha=.16)
ax.spines[['top', 'right', 'left']].set_visible(False)
ax.tick_params(axis='y', length=0, pad=8)
fig.suptitle('Flag enabled: render-stage pause overlaps texture uploads',
             x=.03, ha='left', fontsize=16, fontweight='bold', color='#162d46')
ax.set_title('53.0 ms native interval · second flight · profiler requested 123.7 ms after hitch ended',
             loc='left', fontsize=10, color='#455b70', pad=20)
fig.supxlabel('Scope lengths include waiting. Overlap supports a synchronization hypothesis; it does not prove a dependency.',
              fontsize=9, color='#455b70')
fig.savefig(HERE / 'flag-on-hitch-detail.png', dpi=170)
fig.savefig(HERE / 'flag-on-hitch-detail.svg')
(HERE / 'flag-on-hitch-scopes.json').write_text(json.dumps({
    'source_run': str(RUN), 'profiler_frame': f, 'selected_scopes': retained,
    'alignment': 'Frame 238 matches the unique 52.9878 ms native hitch in the retrospective capture. Profiler and native boundaries differ; the plot uses profiler-relative time.'}, indent=2), encoding='utf-8')
