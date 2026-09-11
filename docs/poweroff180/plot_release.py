"""Plot retained release assessments; validate_evidence.py checks their traces."""
from collections import Counter
import json
from pathlib import Path

import matplotlib
matplotlib.use('Agg')
import matplotlib.pyplot as plt
from matplotlib.lines import Line2D

HERE = Path(__file__).resolve().parent
EVIDENCE = HERE / 'verification'
ORDER = ['calm', 'head5', 'head10', 'head15', 'tail5', 'cross_left10', 'cross_right10']
LABELS = ['Calm', 'Headwind 5 kt', 'Headwind 10 kt', 'Headwind 15 kt',
          'Tailwind 5 kt', 'Left crosswind 10 kt', 'Right crosswind 10 kt']
CAMPAIGNS = {
    'XPT_RUST_ALL_HUD_20260911': ('HUD capture', '#7a4a99', 'o'),
    'XPT_RUST_ALL_MATRIX_20260911': ('Wind matrix', '#087f8c', 's'),
    'XPT_RUST_ALL_REPEAT_20260911': ('Repeat session', '#c06a24', '^'),
}


def main():
    trials = []
    for path in sorted((EVIDENCE / 'release').glob('*/cards/*/assessment.json')):
        assessment = json.loads(path.read_text(encoding='utf-8-sig'))
        config = json.loads((path.parent / 'effective-config.json').read_text(encoding='utf-8-sig'))
        if not assessment['passed'] or not assessment['measurement_valid']:
            raise RuntimeError(f'Failed release assessment: {path}')
        trials.append((config['card_name'], path.parents[2].name, assessment['metrics']))
    if len(trials) != 14 or Counter(name for name, _, _ in trials) != Counter({name: 2 for name in ORDER}):
        raise RuntimeError('The overview requires two passing flights in all seven winds')

    plt.rcParams.update({'font.family': 'DejaVu Sans', 'font.size': 10,
                         'axes.spines.top': False, 'axes.spines.right': False,
                         'axes.spines.left': False, 'svg.hashsalt': 'poweroff180-rust-release'})
    fig, axes = plt.subplots(1, 3, figsize=(12.8, 5.4), sharey=True)
    fig.patch.set_facecolor('#fafbf9')
    metrics = [
        ('touchdown_ft', 'Touchdown distance', 'Feet from threshold', (970, 1230), (1000, 1200), [1000, 1100, 1200]),
        ('kias', 'Touchdown airspeed', 'KIAS', (62, 68), (63, 67), [63, 65, 67]),
        ('physical_sink_fpm', 'Physical descent at contact', 'Feet per minute, positive down', (0, 220), (0, 200), [0, 100, 200]),
    ]
    for ax, (metric, title, label, bounds, accepted, ticks) in zip(axes, metrics):
        ax.set_facecolor('#fafbf9')
        ax.axvspan(*accepted, color='#e3eddf', zorder=0)
        for boundary in accepted:
            ax.axvline(boundary, color='#9aaa93', linestyle='--', linewidth=0.8, zorder=1)
        for row, wind in enumerate(ORDER):
            records = [trial for trial in trials if trial[0] == wind]
            for offset, (_, campaign, values) in zip([-0.1, 0.1], records):
                _, color, marker = CAMPAIGNS[campaign]
                ax.scatter(values[metric], row + offset, s=48, color=color, marker=marker,
                           edgecolors='white', linewidths=0.55, zorder=3)
        ax.set(xlabel=label, xlim=bounds, xticks=ticks, ylim=(6.6, -0.6))
        ax.set_yticks(range(7), LABELS)
        ax.tick_params(axis='y', length=0, pad=8)
        ax.grid(axis='y', color='#d6ded8', linewidth=0.6, zorder=0)
        ax.spines['bottom'].set_color('#a1aaa5')
        ax.set_title(title, loc='left', fontsize=11, fontweight='bold', pad=12)
    fig.suptitle('Rust power-off 180: 14 / 14 landings passed', x=0.04, y=0.98,
                 ha='left', fontsize=18, fontweight='bold', color='#24352e')
    fig.text(0.04, 0.915, 'Two flights per wind · TorqueSim SR20 · KCDW RW22 · Full fuel · 200 lb in each front seat',
             fontsize=10, color='#4b5b53')
    handles = [Line2D([], [], color=color, marker=marker, linestyle='', markersize=6, label=label)
               for label, color, marker in CAMPAIGNS.values()]
    fig.legend(handles=handles, loc='lower center', ncol=3, frameon=False, bbox_to_anchor=(0.5, 0.04))
    fig.text(0.04, 0.015, 'Green bands: unchanged acceptance limits. Each point is one retained flight; sink uses the last airborne vertical velocity.',
             fontsize=8.5, color='#526057')
    fig.subplots_adjust(left=0.17, right=0.98, top=0.79, bottom=0.2, wspace=0.22)
    for extension in ['png', 'svg']:
        fig.savefig(EVIDENCE / f'release-overview.{extension}', dpi=160,
                    facecolor=fig.get_facecolor(), metadata={'Date': None} if extension == 'svg' else None)
    plt.close(fig)


if __name__ == '__main__':
    main()
