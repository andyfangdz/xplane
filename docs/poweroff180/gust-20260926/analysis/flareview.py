"""Print the last approach seconds of a trace at 0.25 s spacing."""
import csv, gzip, sys

def rows(path):
    opener = gzip.open if path.endswith('.gz') else open
    with opener(path, 'rt') as f:
        for r in csv.DictReader(f):
            yield r

path = sys.argv[1]; start_h = float(sys.argv[2]) if len(sys.argv) > 2 else 120
data = [r for r in rows(path)]
contact = next((i for i, r in enumerate(data) if r['contact_latched'] == '1'), len(data) - 1)
begin = next(i for i, r in enumerate(data) if int(float(r['phase_id'])) == 7 and float(r['agl_ft']) < start_h)
last_t = -1
print('   t    h     x   ias   gs   vvi  pitch  pcmd  des_vy wind  predTD  slip  flap')
for r in data[begin:contact + 1]:
    t = float(r['sim_time'])
    if t - last_t < 0.25 and r is not data[contact]:
        continue
    last_t = t
    f = lambda k: float(r[k])
    print(f"{t:7.2f} {f('agl_ft'):5.1f} {f('runway_along_ft'):5.0f} {f('ias_kias'):5.1f} {f('groundspeed_kt'):5.1f} {f('vvi_fpm'):5.0f} "
          f"{f('pitch_deg'):5.1f} {f('pitch_command'):5.1f} {f('desired_vertical_fps'):6.1f} {f('wind_speed_mps')*1.9438:5.1f} "
          f"{f('flare_predicted_touchdown_ft'):6.0f} {f('slip_command_deg'):5.1f} {f('flap_command'):4.2f}")
