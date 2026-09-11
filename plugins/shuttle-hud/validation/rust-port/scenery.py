from pathlib import Path
import sys,json,hashlib
R=Path(__file__).parent;p=R.parents[1]/'Custom Scenery/scenery_packs.ini'
original=(R/'scenery_packs.ini.original').read_bytes()
needle=b'SCENERY_PACK D:\\XPME\\XPME_North_America/'
assert original.count(needle)==1
test=original.replace(needle,b'SCENERY_PACK_DISABLED D:\\XPME\\XPME_North_America/')
assert p.read_bytes() in [original,test],'Unexpected scenery configuration: preserve and inspect'
out={'disable':test,'restore':original}[sys.argv[1]];p.write_bytes(out)
(R/'scenery-session.json').write_text(json.dumps({'mode':sys.argv[1],'sha256':hashlib.sha256(out).hexdigest()},indent=2))
print('Scenery:',sys.argv[1])
