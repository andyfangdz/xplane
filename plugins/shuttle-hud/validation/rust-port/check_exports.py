from pathlib import Path
import sys,re,json
R=Path(r'D:/X-Plane 12/Output/shuttle-rust-20260910');sys.path.insert(0,r'D:/X-Plane 12/Support/flight-test-harness/vendor');from flight_test.api import XPlaneApi
old=Path('V:/src/xplane/plugins/shuttle-hud/reference/cpp/shuttle_hud.cpp').read_text()
expected={name:{'kind':kind,'writable':bool(w)} for kind,name,w in re.findall(r'r([if])\("(fsim_hud/[^\"]+)",&\w+(,true)?\)',old)}
with XPlaneApi(port=8144,timeout=5) as a:
 a.refresh_catalogs();actual={n:v for n,v in a.datarefs.items() if n.startswith('fsim_hud/')}
 print('expected',len(expected),'actual',len(actual),'sample',actual['fsim_hud/version'])
 for name,spec in expected.items():
  assert name in actual,name
  assert bool(actual[name]['is_writable'])==spec['writable'],name
  assert actual[name]['value_type']=={'i':'int','f':'float'}[spec['kind']],name
 names=re.search(r'const char\*names\[\]=\{([^}]+)\}',old)
 if names is None:
  names=re.search(r'const char\*names\[8\]=\{([^}]+)\}',old)
 assert names,'C++ command list not found'
 expected_commands=re.findall(r'"([^"]+)"',names.group(1))
 assert len(expected_commands)==8,expected_commands
 for name in expected_commands:assert name in a.commands,name
 out={'expected':expected,'native_catalog':actual,'additional':sorted(set(actual)-set(expected))}
 out.update(commands={name:a.commands[name] for name in expected_commands},passed=True)
 (R/'dataref-compatibility.json').write_text(json.dumps(out,indent=2))
