"""Download the starter audio listed in data/audio-sources.json into ~/music/flight.

Manually invoked; the app itself never uses the network. Run audio_library.py
afterwards to add the files to the catalogue. A failed source is reported and
skipped, and finished files are never downloaded twice.
"""
from pathlib import Path
import json
import re
import subprocess
import sys
import urllib.request
import xml.etree.ElementTree as ET

ROOT = Path.home() / 'music/flight'
SOURCES = Path(__file__).resolve().parents[1] / 'data/audio-sources.json'
ITUNES = '{http://www.itunes.com/dtds/podcast-1.0.dtd}'


def safe(name):
    return re.sub(r'[^\w .,&\'()-]+', ' ', name).strip()[:90] or 'untitled'


def fetch(url, path):
    if path.exists() and path.stat().st_size > 0:
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    part = path.with_name(path.name + '.part')
    try:
        subprocess.run(['curl', '--fail', '--location', '--silent', '--show-error', '--max-time', '900',
                        '--user-agent', 'cabin-audio-fetch', '--output', str(part), '--', url], check=True)
        part.replace(path)
    finally:
        part.unlink(missing_ok=True)


def read(url):
    request = urllib.request.Request(url, headers={'User-Agent': 'cabin-audio-fetch'})
    with urllib.request.urlopen(request, timeout=30) as response:
        return response.read()


def tag(path, **tags):
    """Set title/artist/album without re-encoding, so the shelf shows real names."""
    tmp = path.with_name('tagging-' + path.name)
    args = [x for k, v in tags.items() for x in ('-metadata', f'{k}={v}')]
    try:
        subprocess.run(['ffmpeg', '-v', 'error', '-y', '-i', str(path), '-map', '0:a', '-c', 'copy', *args, str(tmp)], check=True)
        tmp.replace(path)
    finally:
        tmp.unlink(missing_ok=True)


def podcast(entry):
    channel = ET.fromstring(read(entry['feed'])).find('channel')
    folder = ROOT / 'podcasts' / safe(entry['title'])
    image = channel.find(ITUNES + 'image')
    if image is not None and image.get('href'):
        fetch(image.get('href'), folder / 'cover.jpg')
    for item in channel.findall('item')[:entry.get('episodes', 3)]:
        enclosure, title = item.find('enclosure'), item.findtext('title', 'Episode')
        if enclosure is None:
            continue
        path = folder / (safe(title) + '.mp3')
        fresh = not path.exists()
        fetch(enclosure.get('url'), path)
        if fresh:
            tag(path, title=title, artist=entry['title'], album=entry['title'])
        print('Podcast:', entry['title'], '-', title, flush=True)


def audiobook(entry):
    meta = json.loads(read('https://archive.org/metadata/' + entry['archive_id']))
    base = f'https://archive.org/download/{entry["archive_id"]}/'
    folder = ROOT / 'audiobooks' / safe(entry['title'])
    files = meta['files']
    tracks = sorted((f for f in files if f.get('format') == '64Kbps MP3'), key=lambda f: f['name'])
    cover = next((f for f in files if f.get('format') in ('JPEG', 'Item Image') and f['name'].lower().endswith('.jpg')), None)
    if cover:
        fetch(base + urllib.request.quote(cover['name']), folder / 'cover.jpg')
    for number, track in enumerate(tracks, 1):
        title = track.get('title') or f'Part {number}'
        path = folder / f'{number:02d} {safe(title)}.mp3'
        fresh = not path.exists()
        fetch(base + urllib.request.quote(track['name']), path)
        if fresh:
            tag(path, title=f'{entry["title"]}: {number:02d} {title}', artist=entry['author'], album=entry['title'])
        print('Audiobook:', entry['title'], number, 'of', len(tracks), flush=True)


def main():
    sources, skipped = json.loads(SOURCES.read_text()), []
    for kind, handler in (('podcasts', podcast), ('audiobooks', audiobook)):
        for entry in sources.get(kind, []):
            try:
                handler(entry)
            except Exception as exc:        # one dead source must not stop the rest
                skipped.append({'title': entry['title'], 'error': str(exc)})
                print('Skipped:', entry['title'], exc, flush=True)
    print(json.dumps({'skipped_sources': skipped}), flush=True)
    return 1 if skipped else 0


if __name__ == '__main__':
    sys.exit(main())
