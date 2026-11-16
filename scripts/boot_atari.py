"""Isolated, write-protected Hatari boot audit. Never opens a desktop window."""
import json,os,subprocess,time,sys
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor
home=Path.home();project=Path(__file__).resolve().parents[1];out=project/'docs/atari-boots';out.mkdir(exist_ok=True)
items=[i for i in json.loads((home/'.local/share/cabin/library.json').read_text()) if i['kind']=='atari']
if len(sys.argv)>1:items=[i for i in items if i['folder'] in sys.argv[1:]]
plan_path=project/'data/atari-boot-plan.json';plan=json.loads(plan_path.read_text()) if plan_path.exists() else {}
def run(item):
 name=item['folder'];fifo=out/(name+'.fifo');fifo.unlink(missing_ok=True);log=(out/(name+'.log')).open('w');cfg=out/(name+'.cfg');cfg.write_text('# Isolated audit\n')
 disks=item['disks'][:];
 if name=='space-crusade-voyage-beyond':disks.insert(0,str(home/'games/atari-st/space-crusade/051.MSA'))
 cmd=['hatari','--configfile',str(cfg),'--tos',str(home/'games/atari-st/roms/emutos-192k-1.4/etos192uk.img') if item['folder']!='laser-squad' else item['rom'],'--machine','st','--memsize','1','--monitor','rgb','--disk-a',disks[0],'--disk-b',disks[1] if len(disks)>1 else 'none','--protect-floppy','on','--sound','off','--fast-forward','on','--confirm-quit','off','--cmd-fifo',str(fifo),'--joy0','none','--joy1','none']
 p=subprocess.Popen(cmd,env=dict(os.environ,SDL_VIDEODRIVER='dummy',SDL_AUDIODRIVER='dummy'),stdout=log,stderr=log)
 def send(s):
  fd=os.open(fifo,os.O_WRONLY|os.O_NONBLOCK);os.write(fd,(s+'\n').encode());os.close(fd)
 try:
  time.sleep(9)
  if p.poll() is not None:return name+' exited '+str(p.returncode)
  send('hatari-debug screenshot '+str(out/(name+'-boot.png')));time.sleep(.3)
  for key in plan.get(name,[]):
   send('hatari-event keydown '+key);time.sleep(.2);send('hatari-event keyup '+key);time.sleep(7)
  if plan.get(name):send('hatari-debug screenshot '+str(out/(name+'-game.png')));time.sleep(.5)
  return name+' captured'
 except Exception as e:return name+': '+str(e)
 finally:
  if p.poll() is None:
   p.terminate()
   try:p.wait(timeout=3)
   except subprocess.TimeoutExpired:p.kill();p.wait()
  log.close();fifo.unlink(missing_ok=True);cfg.unlink(missing_ok=True)
with ThreadPoolExecutor(max_workers=6) as pool:
 for result in pool.map(run,items):print(result,flush=True)
