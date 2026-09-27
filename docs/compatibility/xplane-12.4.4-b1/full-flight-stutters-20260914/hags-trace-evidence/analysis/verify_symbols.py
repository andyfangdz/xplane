import hashlib, json, struct
from pathlib import Path

H=Path(__file__).resolve().parent
def read(p):return json.loads(p.read_text(encoding='utf-8-sig'))
summary=read(H/'trace-summary.json')
probes=[read(H/(c['label']+'.json')) for c in summary['cases']]
symbols=read(H/'resolved-symbols.json'); out=[]
for file,record in symbols.items():
    if 'error' in record:
        out.append(dict(file=file,error=record['error']));continue
    p=Path(file);data=p.read_bytes();pe=struct.unpack_from('<I',data,0x3c)[0];opt=pe+24
    timestamp=struct.unpack_from('<I',data,pe+8)[0]
    size=struct.unpack_from('<I',data,opt+56)[0]
    checksum=struct.unpack_from('<I',data,opt+64)[0]
    images=[i for d in probes for i in d['images'] if i['file'].lower()==file.lower()]
    assert images and all((i['checksum'] & 0xffffffff)==checksum and i['size']==size
        and (i['timestamp']==0 or (i['timestamp'] & 0xffffffff)==timestamp) for i in images),file
    digest=hashlib.sha256(data).hexdigest()
    assert digest.lower()==record['binary_sha256'].lower()
    assert hashlib.sha256(Path(record['pdb']).read_bytes()).hexdigest().lower()==record['pdb_sha256'].lower()
    out.append(dict(file=file,sha256=digest,size=size,checksum=checksum,timestamp=timestamp,
        trace_timestamps=sorted({i['timestamp'] for i in images}),pdb=record['pdb'],pdb_sha256=record['pdb_sha256']))
(H/'symbol-image-verification.json').write_text(json.dumps(out,indent=2)+'\n',encoding='utf-8')
print('Verified',len([r for r in out if 'error' not in r]),'public symbol image identities against this trace')
