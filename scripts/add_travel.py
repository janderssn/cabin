from pathlib import Path
import urllib.request, json, re, subprocess, hashlib
root=Path.home()/'.local/share/cabin';assets=root/'assets';mag=Path.home()/'magazines/flight';vroot=Path.home()/'videos/flight/documentaries'
items=json.loads((root/'library.json').read_text());byid={i['id']:i for i in items}
def get(u,p):
 if not p.exists():
  with urllib.request.urlopen(u,timeout=45) as r:b=r.read(120_000_001)
  assert len(b)<120_000_001;p.write_bytes(b)
entries=[('hokkaido-tokachi','Slow Travel Hokkaido','Tokachi · The people behind the good taste','https://slow-life-hokkaido.com/cms/wp-content/uploads/2024/11/BehindTheGoodTasteHokkaidoTokachi.pdf'),('hokkaido-vol5','Slow Life Hokkaido','Volume 5 · Japan’s northern island','https://slow-life-hokkaido.com/cms/wp-content/uploads/2019/09/Slow-Life-HOKKAIDO-vol5.pdf'),('travel-magazine-sep2024','The Travel Magazine','September 2024 · Journeys around the world','https://bvtravel.imgix.net/travel-magazine/travelmag092024.pdf')]
for slug,title,sub,url in entries:
 try:
  p=mag/(slug+'.pdf');get(url,p);assert p.read_bytes().startswith(b'%PDF-');info=subprocess.check_output(['pdfinfo',str(p)],text=True);pages=int(re.search(r'Pages:\s+(\d+)',info)[1]);cover=assets/(slug+'.png')
  if not cover.exists():subprocess.run(['pdftoppm','-f','1','-singlefile','-scale-to','1000','-png',str(p),str(cover.with_suffix(''))],check=True,stderr=subprocess.DEVNULL)
  item=dict(id='mag-'+slug,title=title,subtitle=sub,kind='read',tag='Travel',description=sub+'. A complete issue to browse offline, with destination stories, photography, and travel inspiration.',path=str(p),image=str(cover),gallery=[str(cover)],source=url,meta=f'{pages} pages',pages=pages,bytes=p.stat().st_size,verified=True)
  byid[item['id']]=item;print('Added magazine',title,pages,flush=True)
 except Exception as e:print('Magazine error',slug,e,flush=True)
videos=[('Qe5T3Jxhem0','Lisbon','Rick Steves’ Europe · Portugal','Explore Lisbon’s neighborhoods, food, and history with Rick Steves.'),('0IvZOZtjMYM','Portugal’s Heartland','Rick Steves’ Europe · Portugal','Travel beyond Lisbon through Portugal’s towns, landscapes, and traditions.'),('npGwq_ZxVKk','A Train Ride into Japan’s Past','DW Documentary · Japan','A railway journey into Japan, discovering places and traditions along the way.')]
for vid,title,sub,desc in videos:
 try:
  p=next(p for p in vroot.glob('*.mkv') if '['+vid+']' in p.name);j=json.loads(p.with_suffix('.info.json').read_text());gallery=[]
  for n,t in enumerate([j['duration']*.2,j['duration']*.5,j['duration']*.8]):
   dest=assets/(vid+f'-{n}.jpg')
   if not dest.exists():subprocess.run(['ffmpeg','-v','error','-ss',str(t),'-i',str(p),'-frames:v','1','-vf','scale=960:-1','-y',str(dest)],check=True)
   gallery.append(str(dest))
  byid['video-'+vid]=dict(id='video-'+vid,title=title,subtitle=sub,kind='watch',tag='Travel',description=desc,path=str(p),image=gallery[0],gallery=gallery,source=j['webpage_url'],meta=f'{j["duration"]//60} min',seconds=j['duration'],bytes=p.stat().st_size,verified=True)
  print('Added film',title,flush=True)
 except Exception as e:print('Video not ready:',vid,e,flush=True)
for item in byid.values():
 if item['kind']!='watch':continue
 try:
  p=Path(item['path']);j=json.loads(p.with_suffix('.info.json').read_text());thumbs=[t for t in j.get('thumbnails',[]) if t.get('width',0)>=1000 and 'jpg' in t['url']]
  if thumbs:
   target=assets/(item['id']+'-poster.jpg');get(thumbs[-1]['url'],target);item['image']=str(target)
   if str(target) not in item['gallery']:item['gallery'].insert(0,str(target))
 except Exception as e:print('Poster:',item['id'],e,flush=True)
(root/'library.json').write_text(json.dumps(list(byid.values()),indent=2,ensure_ascii=False)+'\n')
(mag/'sources.json').write_text(json.dumps([dict(title=i['title'],file=i['path'],url=i['source'],sha256=hashlib.sha256(Path(i['path']).read_bytes()).hexdigest()) for i in byid.values() if i['kind']=='read'],indent=2))
print('Catalog:',len(byid),'items',flush=True)
