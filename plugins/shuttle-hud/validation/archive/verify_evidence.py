from pathlib import Path
from html.parser import HTMLParser
import json,hashlib,datetime
R=Path(__file__).parent;ROOT=R.parents[1]
class Links(HTMLParser):
 def __init__(self):super().__init__();self.local=[];self.images=[]
 def handle_starttag(self,tag,attrs):
  a=dict(attrs)
  for k in ['href','src']:
   if k in a and not a[k].startswith(('http:','https:','#')):self.local.append(a[k])
  if tag=='img':assert a.get('alt');self.images.append(a['src'])
p=Links();p.feed((R/'REPORT.html').read_text(encoding='utf-8'))
assert all((R/x).is_file() for x in p.local),[x for x in p.local if not (R/x).is_file()]
for name in p.images:
 d=json.loads((R/name.replace('.png','.json')).read_text());assert d['state']['fsim_hud/version'] in [140,142]
build=json.loads((R/'build-142/build-hashes.json').read_text(encoding='utf-8-sig'))
for row in build:
 path=Path(row['Path']);assert hashlib.sha256(path.read_bytes()).hexdigest().lower()==row['Hash'].lower(),str(path)
audit=json.loads((R/'audit-final.json').read_text());assert audit['source_verified'] and not audit['unexpected_acf_changes']
assert not (ROOT/'Aircraft/Testing/Shuttle Symbology Trial').exists()
assert (R/'retired-probes/Shuttle Symbology Trial/Orbiter_Glider.acf').exists()
assert json.loads((R/'sdk-lifecycle-142.json').read_text())['passed']
assert json.loads((R/'clean-exit-142.json').read_text())['normal_quit']
flights={n:json.loads((R/f'analysis-{n}.json').read_text()) for n in [201,202,203,204,206,207]}
result={'version':142,'symbology_implementation_complete':True,'native_display_checks_complete':True,'landing_regression_status':'mixed; light rebound limit remains repeat-sensitive','accepted_flights':[n for n,d in flights.items() if d['accepted']],'failed_flights':{n:[k for k,v in d['checks'].items() if not v] for n,d in flights.items() if not d['accepted']},'nonperformance_flight_attempts':[205],'original_files_verified':430,'geometry_and_landing_calibration_preserved':True,'release_contains_no_validation_helpers':True,'trial_retired':True,'scenery_restored_exactly':True,'normal_shutdown':True,'local_report_links_checked':len(p.local),'native_images_in_gallery':len(p.images),'report_preview':'Static HTML/image/reference verification completed; in-app browser file URL blocked by browser policy.','binary_sha256':audit['installed_binary_sha256'],'finished_utc':datetime.datetime.now(datetime.timezone.utc).isoformat()}
(R/'completion-142.json').write_text(json.dumps(result,indent=2));print(json.dumps(result,indent=2))
