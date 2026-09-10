from pathlib import Path
import shutil,json,hashlib,sys,urllib.request
R=Path(__file__).parent;ROOT=R.parent.parent
S=ROOT/'Support/Shuttle-HUD';D=ROOT/'Aircraft/OrgForum/Space Shuttle F-SIM HUD'
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
assert not (R/'baseline-136').exists(),'Baseline already captured'
shutil.copytree(S,R/'baseline-136/support')
shutil.copytree(D,R/'baseline-136/aircraft')
(R/'baseline-hashes.json').write_text(json.dumps({str(p.relative_to(D)):sha(p) for p in D.rglob('*') if p.is_file()},indent=2),encoding='utf-8')
shutil.copy2(ROOT/'Custom Scenery/scenery_packs.ini',R/'scenery_packs.ini.original')
sources=R/'sources';sources.mkdir(exist_ok=True)
old=ROOT/'Output/shuttle-correction-20260910/sources'
manifest=json.loads((old/'manifest.json').read_text())
for row in manifest:
 assert sha(old/row['file'])==row['sha256']
 row['local_source']=str(old/row['file'])
(sources/'manuals.json').write_text(json.dumps(manifest,indent=2))
for name in ['hud','landing']:
 url=f'https://fsim.com/help/ios/{name}.html'
 with urllib.request.urlopen(url,timeout=25) as response:data=response.read()
 (sources/f'fsim-{name}.html').write_bytes(data)
pages=json.loads((old/'landing-procedures.json').read_text(encoding='utf-8'))
selected=[p for p in pages if 47<=p['pdf_page']<=52 or 134<=p['pdf_page']<=147]
(sources/'hud-manual-excerpts.json').write_text(json.dumps(selected,indent=2),encoding='utf-8')
sys.path.insert(0,str(ROOT/'Output/shuttle-flare-20260910/tools/pdf'))
import pypdfium2 as pdfium
doc=pdfium.PdfDocument(old/'landing-procedures.pdf')
for n in [47,48,49,50,51,52]:
 page=doc[n-1];page.render(scale=1.7).to_pil().save(sources/f'landing-manual-{n}.png');page.close()
doc.close()
print('Release 136 baseline preserved; primary HUD pages rendered.',flush=True)
