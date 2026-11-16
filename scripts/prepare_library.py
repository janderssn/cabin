from media_metadata import has_subtitles
#!/usr/bin/env python3
"""Build Cabin's offline catalog; keep sources and originals intact."""
from pathlib import Path
import concurrent.futures, hashlib, json, re, subprocess, urllib.request
HOME=Path.home(); ROOT=HOME/'.local/share/cabin'; ASSETS=ROOT/'assets'; ASSETS.mkdir(parents=True,exist_ok=True)
MAG=HOME/'magazines/flight'; MAG.mkdir(parents=True,exist_ok=True)
items=[]
def get(url,path,limit=120_000_000):
 if path.exists():return path
 req=urllib.request.Request(url,headers={'User-Agent':'Cabin-offline-library/0.1'})
 with urllib.request.urlopen(req,timeout=45) as response: data=response.read(limit+1)
 if len(data)>limit:raise ValueError('Download too large')
 path.write_bytes(data);return path

def magazine(entry):
 slug,title,subtitle,url=entry;p=MAG/(slug+'.pdf')
 try:
  get(url,p)
  if not p.read_bytes().startswith(b'%PDF-'):raise ValueError('Not a PDF')
  info=subprocess.check_output(['pdfinfo',str(p)],text=True);pages=int(re.search(r'Pages:\s+(\d+)',info)[1])
  cover=ASSETS/(slug+'.png')
  if not cover.exists():subprocess.run(['pdftoppm','-f','1','-singlefile','-scale-to','1000','-png',str(p),str(cover.with_suffix(''))],check=True,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL)
  r=dict(id='mag-'+slug,title=title,subtitle=subtitle,kind='read',tag='Magazine',description=subtitle+'. A complete, locally stored issue to read at your own pace.',path=str(p),image=str(cover),gallery=[str(cover)],source=url,meta=f'{pages} pages',bytes=p.stat().st_size,pages=pages,verified=True)
  print('Magazine:',title,pages,'pages',flush=True);return r
 except Exception as e:print('FAILED magazine',title,str(e),flush=True);return None
mags=[
 ('nasa-history-summer-2025','NASA History','News & Notes · Summer 2025','https://www.nasa.gov/wp-content/uploads/2025/06/newsnotes-42-2.pdf'),
 ('air-space-jul-aug-2025','Air & Space Forces','July / August 2025 · Aviation & space','https://www.airandspaceforces.com/app/uploads/2025/07/JulyAugust2025_Fullissue.pdf'),
 ('air-space-nov-dec-2025','Air & Space Forces','November / December 2025 · Aviation & space','https://www.airandspaceforces.com/app/uploads/2025/11/NovDec_2025_Full_Issue.pdf'),
 ('st-format-80','ST Format','Issue 80 · Retro computing','https://www.atarimania.com/mags/pdf/atari-st-format-issue-080.pdf'),
 ('st-review-11','Atari ST Review','Issue 11 · Games, reviews & nostalgia','https://www.atarimania.com/mags/pdf/atari_st_review-issue_11.pdf')]
with concurrent.futures.ThreadPoolExecutor(max_workers=3) as pool:items.extend(x for x in pool.map(magazine,mags) if x)
# Video frames are extracted from the downloaded films, not unrelated artwork.
vroot=HOME/'videos/flight/documentaries'
video_copy={
 'WM8kjDF0IJU':('Apollo 13: Home Safe','NASA · Spaceflight','The mission that became a remarkable rescue. Astronauts and flight controllers revisit Apollo 13 with original mission footage.'),
 'Qj_62FWfH78':('The Stuff of Dreams','NASA JPL · Voyager','Follow Voyager to Jupiter and Saturn, and the people who kept an extraordinary mission alive.'),
 'Bd8_vO5zrjo':('Midway: The Japanese Perspective','Montemayor · Part 1 of 3','A detailed, map-led account of the carrier battle, told from the Japanese perspective.'),
 'BXjydKPcX60':('Midway: Hiryu’s Counterstrike','Montemayor · Part 2 of 3','The battle continues as Hiryu launches its counterattack. Watch after part one.'),
 'WHO6xrSF7Sw':('Midway: The American Perspective','Montemayor · Part 3 of 3','The American decisions and strategic consequences that complete the Midway trilogy.'),
 '-dAB72gsIiE':('The Vietnam War','Free Documentary – History · Complete series','A long-form documentary collection covering the conflict, its turning points, and its aftermath.')}
for v in json.loads((vroot/'catalog.json').read_text()):
 vid=re.search(r'\[([^\]]+)\]',v['file'])[1];title,sub,desc=video_copy[vid];p=vroot/v['file'];gallery=[]
 for i,t in enumerate([max(20,v['seconds']*.15),v['seconds']*.48,v['seconds']*.75]):
  target=ASSETS/(vid+f'-{i}.jpg')
  if not target.exists():subprocess.run(['ffmpeg','-v','error','-ss',str(t),'-i',str(p),'-frames:v','1','-vf','scale=960:-1','-y',str(target)],check=True)
  gallery.append(str(target))
 items.append(dict(id='video-'+vid,title=title,subtitle=sub,kind='watch',tag='Space' if vid in ('WM8kjDF0IJU','Qj_62FWfH78') else ('Vietnam' if vid.startswith('-') else 'WWII'),description=desc,path=str(p),image=gallery[0],gallery=gallery,source=v['source'],meta=f'{v["seconds"]//3600}h {v["seconds"]%3600//60:02}m' if v['seconds']>=3600 else f'{v["seconds"]//60} min',bytes=v['bytes'],seconds=v['seconds'],verified=True,subtitles=has_subtitles(p)))
# Official puzzle screenshots; executable names come from the installed package.
puzzle_desc={'net':'Rotate the tiles to reconnect every part of the network. Small boards turn into surprisingly satisfying challenges.','pattern':'Use the row and column clues to reveal a hidden pixel picture. A quiet, absorbing nonogram puzzle.','bridges':'Connect numbered islands with bridges without crossing them. Clean rules, wonderfully tricky deductions.','loopy':'Draw a single closed loop around a grid of clues. Every line is a small deduction.','lightup':'Place bulbs to illuminate every white square without bulbs lighting one another.','samegame':'Clear groups of matching blocks and plan ahead for a high score.','solo':'Classic Sudoku with adjustable difficulty and a fresh board whenever you want.','flood':'Fill the entire board with one colour in as few moves as possible.'}
def native(p):
 slug=p.name.removeprefix('puzzles-');img=ASSETS/('puzzle-'+slug+'.png');url='https://www.chiark.greenend.org.uk/~sgtatham/puzzles/'+slug+'-web.png'
 try:get(url,img,5_000_000)
 except Exception as e:print('Puzzle image:',slug,e,flush=True)
 title={'net':'Net','samegame':'Same Game','lightup':'Light Up','solo':'Sudoku','pattern':'Pattern'}.get(slug,slug.title())
 desktop=Path('/usr/share/applications')/(p.name+'.desktop');comment='A compact logic puzzle with adjustable difficulty and built-in instructions.'
 if desktop.exists():
  m=re.search(r'^Comment=(.+)',desktop.read_text(),re.M)
  if m:comment=m[1]
 return dict(id='puzzle-'+slug,title=title,subtitle='Simon Tatham’s Puzzle Collection',kind='puzzles',tag='Logic',description=puzzle_desc.get(slug,comment),path=str(p),image=str(img) if img.exists() else '',gallery=[str(img)] if img.exists() else [],source='https://www.chiark.greenend.org.uk/~sgtatham/puzzles/'+slug+'.html',meta='Quick play',bytes=0,verified=True)
with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:items.extend(pool.map(native,sorted(Path('/usr/bin').glob('puzzles-*'))))
groot=HOME/'games/atari-st';games=json.loads((groot/'CORE-COLLECTION.json').read_text())
slugs={'midwinter-ii':'midwinter-ii-flames-of-freedom','phantasie-iii':'phantasie-iii-the-wrath-of-nikademus','battletech':'battletech-the-crescent-hawks-inception','sundog':'sundog-frozen-legacy','conflict':'conflict-the-middle-east-political-simulator','space-crusade-voyage-beyond':'space-crusade-the-voyage-beyond'}
descriptions={'laser-squad':'Turn-based squad tactics: spend action points, plan firing lines, and lead your team through hostile territory. Arrow keys are mapped for this copy.','midwinter':'Explore a frozen island, recruit resistance fighters, and oppose an invading army in this open-ended survival strategy classic.','midwinter-ii':'An ambitious island-hopping blend of exploration, vehicles, and resistance operations.','super-cars-ii':'Top-down racing with armed cars, upgrades, and a lot of chaos.','stunt-car-racer':'Race a stunt car across elevated tracks and enormous jumps.','phantasie-iii':'Build an adventuring party and explore a classic fantasy role-playing world.','battletech':'A BattleTech role-playing adventure in the Crescent Hawk’s Inception.','sabre-team':'Lead a modern rescue team through tactical missions. F2 or Tab selects the game, then Space begins.','lemmings':'Guide a crowd of tiny walkers to safety by assigning the right skills at exactly the right time.','puzznic':'Shift blocks under gravity to make matching pieces disappear.','loopz':'Fit pieces together to complete loops before the board fills.','tetris':'The falling-block classic. Clear lines and keep the stack under control.','atomino':'Join atoms into complete molecules in this chemistry-themed logic game.','klax':'Catch coloured tiles and arrange matching rows on a small board.'}
def atari(g):
 slug=slugs.get(g['folder'],g['folder']);url='https://www.atarilegend.com/games/'+slug;gallery=[]
 try:
  page=urllib.request.urlopen(url,timeout=20).read().decode()
  links=list(dict.fromkeys(re.findall(r'https://www\.atarilegend\.com/games/[^"\s<>]+/screenshot-\d+\.png',page)))[:3]
  for i,link in enumerate(links):
   target=ASSETS/('atari-'+g['folder']+f'-{i}.png');get(link,target,8_000_000);gallery.append(str(target))
 except Exception as e:print('Atari image:',g['folder'],str(e)[:90],flush=True)
 if not gallery:
  for candidate in [groot/'verification/puzzles'/(g['folder']+'-game.png'),groot/g['folder']/'verification/title-screen.png']:
   if candidate.exists():gallery=[str(candidate)];break
 return dict(id='atari-'+g['folder'],title=g['title'],subtitle='Atari ST · Hatari',kind='atari',tag='Retro',description=descriptions.get(g['folder'],'A classic from your curated Atari ST collection. Launch directly with the saved Hatari setup.'),path=str(groot/g['files'][0]['path']),image=gallery[0] if gallery else '',gallery=gallery,source=url,meta='Atari ST',bytes=sum(f['bytes'] for f in g['files']),verified=g.get('boot_tested',False),notes=g.get('boot_test_notes','Compatibility has not been checked for this copy.'),folder=g['folder'],disks=[str(groot/f['path']) for f in g['files'] if Path(f['path']).suffix.lower() in ['.st','.msa','.stx']],rom=str(groot/g['rom']) if 'rom' in g else str(HOME/'.local/share/laser-squad-st/emutos.img'))
with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:items.extend(pool.map(atari,games))
items.sort(key=lambda x:(x['kind'],x['title'].lower()))
(ROOT/'library.json').write_text(json.dumps(items,indent=2,ensure_ascii=False)+'\n')
(MAG/'sources.json').write_text(json.dumps([dict(title=i['title'],file=i['path'],url=i['source'],sha256=hashlib.sha256(Path(i['path']).read_bytes()).hexdigest()) for i in items if i['kind']=='read'],indent=2))
print('Saved',len(items),'offline entries to',ROOT/'library.json',flush=True)
