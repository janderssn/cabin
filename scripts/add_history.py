from pathlib import Path
import json, subprocess, zipfile, re, shutil
from html import unescape
home=Path.home(); project=Path(__file__).resolve().parents[1];root=home/'.local/share/cabin';assets=root/'assets';vroot=home/'videos/flight/documentaries';books=home/'books/flight';books.mkdir(parents=True,exist_ok=True)
items=json.loads((root/'library.json').read_text());byid={i['id']:i for i in items}
for ident,title,author,desc in [('40549','The Private Life of the Romans','Harold Whetstone Johnston · 1903','An illustrated classic on Roman households, education, clothing, meals, baths and daily life. Historical scholarship from 1903; some interpretations are dated.'),('44624','Sweden','Victor Alfred Nilsson · 1899','An illustrated historical survey of Sweden. A nineteenth-century work whose interpretations and language reflect its period, rather than current scholarship.')]:
 dest=books/ident;dest.mkdir(exist_ok=True);archive=books/(ident+'.zip')
 if not archive.exists():subprocess.run(['curl','-fL','--max-time','120','-o',str(archive),f'https://www.gutenberg.org/cache/epub/{ident}/pg{ident}-h.zip'],check=True)
 with zipfile.ZipFile(archive) as z:
  for name in z.namelist():
   if Path(name).is_absolute() or '..' in Path(name).parts:raise ValueError('Unsafe archive path')
  z.extractall(dest)
 html=next(p for p in dest.rglob('*') if p.suffix in ('.html','.htm'))
 text=html.read_text();original=html.with_suffix('.original.html');shutil.copy2(html,original)
 css='''<style>html{background:#151b23;color:#e8edf4}body{max-width:46rem;margin:auto!important;padding:2rem!important;font:22px/1.75 Georgia,serif!important;background:#151b23!important;color:#e8edf4!important}p{line-height:1.75!important}a{color:#a5c5ff}img{max-width:100%;height:auto}h1,h2,h3{line-height:1.3}table{max-width:100%}pre{white-space:pre-wrap} .pg-boilerplate{font-size:17px}</style>'''
 html.write_text(text.replace('</head>',css+'</head>',1))
 candidates=re.findall(r'<img[^>]+src=[\"\']([^\"\']+)',text,re.I);image=''
 for relative in candidates:
  source=html.parent/unescape(relative)
  if source.is_file():
   target=assets/(ident+'-book'+source.suffix);shutil.copy2(source,target);image=str(target);break
 byid['book-'+ident]=dict(id='book-'+ident,title=title,subtitle=author,kind='read',tag='Roman history' if ident=='40549' else 'Swedish history',description=desc,path=str(html),image=image,gallery=[image] if image else [],source=f'https://www.gutenberg.org/ebooks/{ident}',meta='Illustrated book',bytes=sum(p.stat().st_size for p in dest.rglob('*') if p.is_file()),verified=True)
 print('Added book',title,flush=True)
for vid,title,sub,desc in [('Zu9EWhKzsVg','Rome: Origins of an Empire','Mary Beard · Empire Without Limit · Part 1','Mary Beard explores the origins, foundational myths and expansion of Rome.'),('RVUy_bm4V1k','Rome: Life Inside the Empire','Mary Beard · Empire Without Limit · Part 2','The physical world and everyday life of the Roman Empire, explored through its surviving evidence.'),('Phqk_vXby1w','Vasa: A Sunken Treasure','Swedish history · The Vasa Museum','A close look at the preserved Swedish warship with Fred Hocker, the Vasa Museum’s director of research.')]:
 path=next(p for p in vroot.glob('*.mkv') if '['+vid+']' in p.name);metadata=json.loads(path.with_suffix('.info.json').read_text());probe=json.loads(subprocess.check_output(['ffprobe','-v','error','-show_format','-show_streams','-of','json',str(path)]));duration=float(probe['format']['duration']);assert {'video','audio'}<={s['codec_type'] for s in probe['streams']}
 for t in [0,max(0,duration-5)]:subprocess.run(['ffmpeg','-v','error','-ss',str(t),'-i',str(path),'-t','2','-f','null','-'],check=True)
 gallery=[]
 for n,fraction in enumerate([.2,.5,.8]):
  out=assets/(vid+f'-{n}.jpg');subprocess.run(['ffmpeg','-v','error','-ss',str(duration*fraction),'-i',str(path),'-frames:v','1','-vf','scale=960:-1','-y',str(out)],check=True);gallery.append(str(out))
 poster=next((path.with_suffix(ext) for ext in ['.webp','.jpg','.png'] if path.with_suffix(ext).exists()),None)
 if poster:
  out=assets/(vid+'-poster.jpg');subprocess.run(['ffmpeg','-v','error','-i',str(poster),'-frames:v','1','-y',str(out)],check=True);gallery.insert(0,str(out))
 byid['video-'+vid]=dict(id='video-'+vid,title=title,subtitle=sub,kind='watch',tag='Swedish history' if vid=='Phqk_vXby1w' else 'Roman history',description=desc,path=str(path),image=gallery[0],gallery=gallery,source=metadata['webpage_url'],meta=f'{round(duration)//60} min',seconds=round(duration),bytes=path.stat().st_size,verified=True)
 print('Verified film',title,flush=True)
items=list(byid.values());(root/'library.json.tmp').write_text(json.dumps(items,indent=2,ensure_ascii=False)+'\n');(root/'library.json.tmp').replace(root/'library.json');(project/'data/library.json').write_text(json.dumps(items,indent=2,ensure_ascii=False)+'\n');(project/'data/content-sources.json').write_text(json.dumps([dict(id=i['id'],title=i['title'],kind=i['kind'],source=i['source']) for i in items],indent=2,ensure_ascii=False)+'\n')
films=[i for i in items if i['kind']=='watch'];(vroot/'catalog.json').write_text(json.dumps([dict(title=i['title'],file=i['path'],seconds=i['seconds'],source=i['source'],bytes=i['bytes']) for i in films],indent=2));(vroot/'flight-playlist.m3u').write_text('#EXTM3U\n'+'\n'.join(i['path'] for i in films)+'\n');print('Total',len(items),flush=True)
