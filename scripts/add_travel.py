"""Prepare the curated nature-travel shelf. Downloads magazines, uses local films.
Run yt-dlp on videos/flight/documentaries/nature-travel-sources.txt first.
Original superseded downloads are preserved but removed from the active catalogs.
"""
from pathlib import Path
import urllib.request
import json
import re
import subprocess
import hashlib
import sys
from download_support import run_sources
skipped = []

home = Path.home()
project = Path(__file__).resolve().parents[1]
root = home / '.local/share/cabin'
assets = root / 'assets'
mag = home / 'magazines/flight'
vroot = home / 'videos/flight/documentaries'
picks = json.loads((project / 'data/travel-picks.json').read_text())
if not picks.get('travel_enabled', True):
    picks['videos'] = []
    picks['magazines'] = []
items = json.loads((root / 'library.json').read_text())
# Keep a recovery copy; publish every successful source even if another fails.
backup = root / 'library-before-nature-travel.json'
if not backup.exists():
    backup.write_text(json.dumps(items, indent=2, ensure_ascii=False) + '\n')
byid = {i['id']: i for i in items if i.get('tag') != 'Travel' and i['id'] not in {'video-Bd8_vO5zrjo', 'video-BXjydKPcX60', 'video-WHO6xrSF7Sw'}}

def get(url, path):
    if path.exists():
        return
    temporary = path.with_suffix(path.suffix + '.part')
    try:
        subprocess.run(['curl', '--fail', '--location', '--max-time', '120', '--max-filesize', '500000000', '--output', str(temporary), url], check=True)
    except Exception:
        temporary.unlink(missing_ok=True)
        raise
    temporary.replace(path)

def prepare_magazine(entry):
    slug = entry['id']
    path = mag / (slug + '.pdf')
    get(entry['url'], path)
    assert path.read_bytes().startswith(b'%PDF-'), path
    info = subprocess.check_output(['pdfinfo', str(path)], text=True)
    pages = int(re.search(r'Pages:\s+(\d+)', info)[1])
    cover = assets / (slug + '.png')
    if not cover.exists():
        subprocess.run(['pdftoppm', '-f', '1', '-singlefile', '-scale-to', '1000', '-png', str(path), str(cover.with_suffix(''))], check=True)
    item = dict(id='mag-' + slug, title=entry['title'], subtitle=entry['subtitle'], kind='read', tag=entry.get('tag', 'Travel'), description=entry['description'], path=str(path), image=str(cover), gallery=[str(cover)], source=entry['url'], meta=f'{pages} pages', pages=pages, bytes=path.stat().st_size, verified=True)
    byid[item['id']] = item
    print('Prepared magazine:', item['title'], pages, 'pages', flush=True)

run_sources(picks['magazines'] + json.loads((project / 'data/retro-magazines.json').read_text()), prepare_magazine, skipped)

def prepare_video(entry):
    vid = entry['id']
    path = next(p for p in vroot.glob('*.mkv') if '[' + vid + ']' in p.name)
    metadata = json.loads(path.with_suffix('.info.json').read_text())
    probe = json.loads(subprocess.check_output(['ffprobe', '-v', 'error', '-show_format', '-show_streams', '-of', 'json', str(path)]))
    duration = float(probe['format']['duration'])
    assert any(s['codec_type'] == 'audio' for s in probe['streams'])
    assert any(s['codec_type'] == 'video' for s in probe['streams'])
    for position in (0, max(0, duration - 5)):
        subprocess.run(['ffmpeg', '-v', 'error', '-ss', str(position), '-i', str(path), '-t', '2', '-f', 'null', '-'], check=True)
    gallery = []
    for n, fraction in enumerate((.2, .5, .8)):
        dest = assets / (vid + f'-{n}.jpg')
        if not dest.exists():
            subprocess.run(['ffmpeg', '-v', 'error', '-ss', str(duration * fraction), '-i', str(path), '-frames:v', '1', '-vf', 'scale=960:-1', '-y', str(dest)], check=True)
        gallery.append(str(dest))
    poster_source = next((path.with_suffix(ext) for ext in ('.webp', '.jpg', '.png') if path.with_suffix(ext).exists()), None)
    if poster_source:
        dest = assets / (vid + '-poster.jpg')
        subprocess.run(['ffmpeg', '-v', 'error', '-i', str(poster_source), '-frames:v', '1', '-y', str(dest)], check=True)
        gallery.insert(0, str(dest))
    item = dict(id='video-' + vid, title=entry['title'], subtitle=entry['subtitle'], kind='watch', tag='Travel', description=entry['description'], path=str(path), image=gallery[0], gallery=gallery, source=metadata['webpage_url'], meta=f'{int(duration)//60} min', seconds=round(duration), bytes=path.stat().st_size, verified=True)
    byid[item['id']] = item
    print('Verified film:', item['title'], round(duration), 'seconds', flush=True)

run_sources(picks['videos'], prepare_video, skipped)

items = list(byid.values())
temporary = root / 'library.json.tmp'
temporary.write_text(json.dumps(items, indent=2, ensure_ascii=False) + '\n')
temporary.replace(root / 'library.json')
(project / 'data/library.json').write_text(json.dumps(items, indent=2, ensure_ascii=False) + '\n')
(project / 'data/content-sources.json').write_text(json.dumps([dict(id=i['id'], title=i['title'], kind=i['kind'], source=i['source']) for i in items], indent=2, ensure_ascii=False) + '\n')
(mag / 'sources.json').write_text(json.dumps([dict(title=i['title'], file=i['path'], url=i['source'], sha256=hashlib.sha256(Path(i['path']).read_bytes()).hexdigest()) for i in items if i['kind'] == 'read'], indent=2))
films = [i for i in items if i['kind'] == 'watch']
(vroot / 'catalog.json').write_text(json.dumps([dict(title=i['title'], file=i['path'], seconds=i['seconds'], source=i['source'], bytes=i['bytes']) for i in films], indent=2))
(vroot / 'flight-playlist.m3u').write_text('#EXTM3U\n' + '\n'.join(i['path'] for i in films) + '\n')
print('Catalog:', len(items), 'items;', len(films), 'films;', round(sum(i['seconds'] for i in films)/60), 'minutes', flush=True)

print(json.dumps({"skipped_sources": skipped, "skipped_count": len(skipped)}), flush=True)
sys.exit(1 if skipped else 0)
