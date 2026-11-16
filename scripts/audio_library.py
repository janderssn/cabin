"""Import local music, podcasts and audiobooks without network requests."""
from pathlib import Path
import hashlib,json,subprocess
EXTENSIONS={'.mp3','.m4a','.m4b','.ogg','.opus','.flac','.wav','.aac'}
def scan(folder):
    folder=Path(folder);folder.mkdir(parents=True,exist_ok=True);items=[];skipped=[]
    for path in sorted(folder.rglob('*')):
        if path.suffix.lower() not in EXTENSIONS or not path.is_file():continue
        try:
            p=json.loads(subprocess.check_output(['ffprobe','-v','error','-show_format','-show_streams','-of','json',str(path)],stderr=subprocess.DEVNULL))
            if not any(s['codec_type']=='audio' for s in p['streams']):raise ValueError('No audio stream')
            seconds=round(float(p['format']['duration']));tags=p['format'].get('tags',{});title=tags.get('title',path.stem.replace('_',' '));parts=str(path.relative_to(folder)).lower()
            tag='Audiobook' if path.suffix.lower()=='.m4b' or 'audiobook' in parts else ('Podcast' if 'podcast' in parts else 'Music')
            cover=next((p for n in ['cover.jpg','cover.png','folder.jpg'] if (p:=path.parent/n).is_file()),None)
            items.append(dict(id='audio-'+hashlib.sha256(str(path.relative_to(folder)).encode()).hexdigest()[:16],title=title,subtitle=tags.get('artist',tags.get('album',tag)),kind='listen',tag=tag,description='Local '+tag.lower()+'. Playback resumes from your saved position.',path=str(path.resolve()),source='Local audio collection',image=str(cover) if cover else '',gallery=[str(cover)] if cover else [],seconds=seconds,bytes=path.stat().st_size,meta=f'{seconds//60} min',verified=True))
        except Exception as e:skipped.append({'file':str(path),'error':str(e)})
    return items,skipped

def main():
    home=Path.home();catalog=home/'.local/share/cabin/library.json';existing=json.loads(catalog.read_text());audio,skipped=scan(home/'music/flight');items=[i for i in existing if i['kind']!='listen']+audio
    temp=catalog.with_suffix('.json.tmp');temp.write_text(json.dumps(items,indent=2,ensure_ascii=False)+'\n');temp.replace(catalog)
    print(json.dumps({'imported':len(audio),'skipped':skipped}));return 1 if skipped else 0
if __name__=='__main__':raise SystemExit(main())
