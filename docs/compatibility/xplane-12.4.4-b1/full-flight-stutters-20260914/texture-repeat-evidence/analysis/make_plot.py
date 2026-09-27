from pathlib import Path
import json
import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt

here = Path(__file__).resolve().parent
result = json.loads((here / 'results.json').read_text())
plt.rcParams.update({'font.family': 'DejaVu Sans', 'font.size': 11,
    'axes.spines.top': False, 'axes.spines.right': False})
fig, axes = plt.subplots(1, 2, figsize=(12, 5.6))
colors = {5: '#ad5834', 4: '#247b80'}
labels = {5: 'Maximum textures', 4: 'One notch lower'}
for texture in (5, 4):
    trials = [r for r in result['trials'] if r['texture_resolution'] == texture]
    x = [r['pair'] + (-.18 if texture == 5 else .18) for r in trials]
    rates = [r['aggregate']['over_50ms_per_minute'] for r in trials]
    maxima = [r['aggregate']['max_ms'] for r in trials]
    for ax, values, fmt in [(axes[0], rates, '.2f'), (axes[1], maxima, '.0f')]:
        bars = ax.bar(x, values, width=.32, color=colors[texture], label=labels[texture])
        ax.bar_label(bars, labels=[format(v, fmt) for v in values], padding=5, fontsize=11)
for ax in axes:
    ax.set_xticks([1, 2, 3], ['Pair 1\nMax → Lower', 'Pair 2\nLower → Max', 'Pair 3\nMax → Lower'])
    ax.set_axisbelow(True)
    ax.grid(axis='y', alpha=.18)
    ax.set_ylim(0, max(1, ax.get_ylim()[1]) * 1.2)
axes[0].set_ylabel('Native intervals >50 ms per minute')
axes[0].set_title('Stutter frequency', loc='left', weight='bold')
axes[1].set_ylabel('Longest native interval (ms)')
axes[1].set_title('Worst pause in each launch', loc='left', weight='bold')
axes[1].axhline(50, color='#59636c', linewidth=1, linestyle='--', alpha=.6)
fig.suptitle('Repeated texture-quality comparison', x=.08, ha='left', fontsize=20, weight='bold')
fig.legend(*axes[0].get_legend_handles_labels(), loc='upper left', bbox_to_anchor=(.072, .93), ncol=2, frameon=False)
fig.text(.08, .055, 'Two flights per fresh process · flag on · 10 GB disk cache · HAGS off · full scenery\n'
         'First five unpaused seconds excluded. No system tracing or video. Native timing is not display/VR timing.',
         fontsize=9, color='#4d5860', linespacing=1.55)
fig.subplots_adjust(left=.08, right=.98, top=.77, bottom=.23, wspace=.28)
fig.savefig(here / 'texture-comparison.png', dpi=160, facecolor='white')
fig.savefig(here / 'texture-comparison.svg', facecolor='white')
