import hashlib,json,struct
from pathlib import Path
H=Path(__file__).resolve().parent
d=json.loads((H/'dxgi-worst.json').read_text())
symbols=json.loads((H/'resolved-symbols.json').read_text())
out=[]
for file,record in symbols.items():
    p=Path(file);data=p.read_bytes(); pe=struct.unpack_from('<I',data,0x3c)[0];opt=pe+24
    timestamp=struct.unpack_from('<I',data,pe+8)[0]
    size=struct.unpack_from('<I',data,opt+56)[0]
    checksum=struct.unpack_from('<I',data,opt+64)[0]
    images=[i for i in d['images'] if i['file'].lower()==file.lower()]
    assert images and all((i['checksum'] & 0xffffffff)==checksum and i['size']==size and (i['timestamp']==0 or (i['timestamp'] & 0xffffffff)==timestamp) for i in images),file
    digest=hashlib.sha256(data).hexdigest()
    assert digest.lower()==record['binary_sha256'].lower()
    out.append(dict(file=file,sha256=digest,size=size,checksum=checksum,timestamp=timestamp,
                    trace_timestamps=sorted({i['timestamp'] for i in images}),pdb=record['pdb'],pdb_sha256=record['pdb_sha256']))
(H/'symbol-image-verification.json').write_text(json.dumps(out,indent=2)+'\n',encoding='utf-8')
print('Verified',len(out),'symbolized module images against trace size/checksum and available timestamp')
