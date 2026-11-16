"""Fetch English manual or automatic captions for every downloaded film."""
from pathlib import Path
import json,subprocess
from concurrent.futures import ThreadPoolExecutor
home=Path.home();folder=home/'videos/flight/documentaries';root=home/'.local/share/cabin';project=Path(__file__).resolve().parents[1]
def available(path):
    sidecars=[p for p in (p for p in path.parent.iterdir() if p.name.startswith(path.stem+'.')) if p.suffix in {'.vtt','.srt','.ass','.ssa'} and p.stat().st_size>0]
    tracks=json.loads(subprocess.check_output(['ffprobe','-v','error','-select_streams','s','-show_streams','-of','json',str(path)]))['streams']
    return bool(sidecars or tracks)
def fetch(path):
    if available(path):return {'file':str(path),'available':True,'status':'already present'}
    info=path.with_suffix('.info.json')
    if not info.exists():return {'file':str(path),'available':False,'status':'no source metadata'}
    j=json.loads(info.read_text());url=j.get('webpage_url','https://www.youtube.com/watch?v='+j['id'])
    cmd=['yt-dlp','--no-playlist','--skip-download','--write-subs','--write-auto-subs','--sub-langs','en,en-orig','--sub-format','vtt/srt/best','--retries','2','--socket-timeout','30','-o',str(path.with_suffix(''))+'.%(ext)s',url]
    result=subprocess.run(cmd,capture_output=True,text=True)
    ok=available(path)
    return {'file':str(path),'available':ok,'status':'downloaded' if ok else 'unavailable or download failed','detail':(result.stderr+result.stdout)[-1000:] if not ok else ''}
with ThreadPoolExecutor(max_workers=3) as pool:results=list(pool.map(fetch,sorted(folder.glob('*.mkv'))))
lookup={r['file']:r['available'] for r in results}
items=json.loads((root/'library.json').read_text())
for item in items:
    if item['kind']=='watch':item['subtitles']=lookup.get(item['path'],False)
(root/'library.json.tmp').write_text(json.dumps(items,indent=2,ensure_ascii=False)+'\n');(root/'library.json.tmp').replace(root/'library.json');(project/'data/library.json').write_text(json.dumps(items,indent=2,ensure_ascii=False)+'\n');(project/'data/subtitle-report.json').write_text(json.dumps(results,indent=2)+'\n')
for r in results:print(Path(r['file']).name,':',r['status'],flush=True)
print('Subtitles available:',sum(r['available'] for r in results),'/',len(results),flush=True)
