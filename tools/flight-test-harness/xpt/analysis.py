"""Assess native evidence without using supervisor polling as flight truth."""
from __future__ import annotations
import math
from .protocol import PHASES

def assess(document, rows):
    cfg=document['effective_config'];p=cfg['parameters'];a=cfg['acceptance'];terminal=document['terminal']
    reasons=[];invalid=[]
    if document.get('schema_version',1)>=2:
        # Native CSV v2 appends actual inner-loop authority to the stable v1
        # snapshot. Older retained flights remain readable without inventing it.
        required={'attitude_armed':1,'attitude_active':1,'attitude_release_reason':0,
                  'attitude_override_roll':1,'attitude_override_pitch':1,'attitude_override_yaw':1}
        if not rows or any(any(r.get(k)!=v for k,v in required.items()) for r in rows):
            invalid.append('Attitude authority not continuously established')
    cut=next((r for r in rows if r['cut_sim_time']>=0),None)
    if terminal['phase']!='complete':
        invalid.append('Native abort: '+terminal['reason'])
    if cut is None:
        invalid.append('No abeam power cut')
        return {'measurement_valid':False,'passed':False,'reasons':reasons+invalid,'metrics':{},'native_samples':len(rows)}
    after=[r for r in rows if r['sim_time']>cut['cut_sim_time']+.25]
    if not after or any(r['override_path']!=0 or r['throttle_ratio']>.01 for r in after):
        invalid.append('Idle throttle or released flight path not sustained')
    if any(abs(r['mass_kg']*2.20462262185-p['mass_target_lb'])>p['mass_tolerance_lb'] for r in after):
        invalid.append('Mass outside tolerance')
    if cut['entry_gate_s']<p['entry_gate_s']:
        invalid.append('Entry gate not established')
    wind=[r for r in after if r['agl_ft']>25]
    if any(abs(r['wind_speed_mps']*1.94384449-p['wind_speed_kt'])>p['wind_tolerance_kt'] for r in wind):
        invalid.append('Wind outside tolerance')
    contacts=[r for r in rows if r['contact_latched']==1]
    dt=[r['control_dt_s'] for r in rows if r['control_dt_s']>0]
    metrics={'entry_kias':cut['ias_kias'],'entry_agl_ft':cut['agl_ft'],
             'native_dt_min_s':min(dt) if dt else None,'native_dt_max_s':max(dt) if dt else None,
             'native_steps':terminal['native_steps'],'max_bank_deg':max(abs(r['bank_deg']) for r in rows)}
    if contacts:
        contact=contacts[0];end=contact['first_sim_time']
        approach=[r for r in rows if r['sim_time']<=end]
        flare=[r for r in approach if r['roundout_sim_time']>=0]
        final=[r for r in approach if int(r['phase_id'])==7 and r['runway_along_ft']>=a['short_final_start_ft']]
        rates=[abs((b['pitch_deg']-r['pitch_deg'])/(b['sim_time']-r['sim_time'])) for r,b in zip(flare,flare[1:]) if b['sim_time']>r['sim_time']]
        metrics.update(touchdown_ft=contact['first_along_ft'],cross_ft=contact['first_cross_ft'],
            contact_bracket_ft=[contact['last_airborne_along_ft'],contact['first_along_ft']],
            kias=contact['first_kias'],physical_sink_fpm=-contact['first_physical_fpm'],indicated_sink_fpm=-contact['first_indicated_fpm'],
            max_flare_pitch_deg=max((r['pitch_deg'] for r in flare),default=0),
            max_flare_pitch_rate_deg_s=max(rates,default=0),
            roundout_entry_kias=flare[0]['ias_kias'] if flare else None,
            roundout_entry_agl_ft=flare[0]['agl_ft'] if flare else None,
            roundout_entry_along_ft=flare[0]['runway_along_ft'] if flare else None,
            flare_duration_s=end-flare[0]['sim_time'] if flare else None,
            short_final_max_cross_ft=max((abs(r['runway_cross_ft']) for r in final),default=999),
            post_contact_max_agl_ft=terminal['post_contact_max_agl_ft'],post_contact_max_g=terminal['post_contact_max_g'])
        if not flare:invalid.append('No recorded roundout')
        if contact['first_sim_time']<=contact['last_airborne_sim_time']:invalid.append('Invalid native contact time bracket')
        if not a['touchdown_min_ft']<=metrics['touchdown_ft']<=a['touchdown_max_ft']:reasons.append('Touchdown distance')
        if abs(metrics['cross_ft'])>a['touchdown_max_cross_ft'] or metrics['short_final_max_cross_ft']>a['short_final_max_cross_ft']:reasons.append('Alignment')
        if metrics['physical_sink_fpm']>a['maximum_sink_fpm']:reasons.append('Physical sink')
        if not a['minimum_touchdown_kias']<=metrics['kias']<=a.get('maximum_touchdown_kias',math.inf):reasons.append('Touchdown speed')
        if flare and metrics['roundout_entry_agl_ft']>a.get('maximum_roundout_agl_ft',math.inf):reasons.append('Roundout height')
        if metrics['max_flare_pitch_deg']>a['maximum_pitch_deg'] or metrics['max_flare_pitch_rate_deg_s']>a['maximum_pitch_rate_deg_s']:reasons.append('Flare motion')
        if metrics['post_contact_max_agl_ft']>a['maximum_post_contact_agl_ft']:reasons.append('Rebound')
    else:
        reasons.append('No touchdown')
    transitions=[];previous=None
    for row in rows:
        phase=PHASES[int(row['phase_id'])]
        if phase!=previous:
            transitions.append({'phase':phase,'seconds_from_cut':row['sim_time']-cut['cut_sim_time'],
                                'along_ft':row['runway_along_ft'],'cross_ft':row['runway_cross_ft'],'agl_ft':row['agl_ft']})
            previous=phase
    return {'measurement_valid':not invalid,'passed':not reasons and not invalid,'reasons':reasons+invalid,
            'metrics':metrics,'transitions':transitions,'native_samples':len(rows)}
