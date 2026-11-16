from pathlib import Path
import json,subprocess
def has_subtitles(path):
    path=Path(path)
    if any(p.suffix in {'.vtt','.srt','.ass','.ssa'} and p.stat().st_size>0 for p in (p for p in path.parent.iterdir() if p.name.startswith(path.stem+'.'))):return True
    probe=subprocess.run(['ffprobe','-v','error','-select_streams','s','-show_streams','-of','json',str(path)],capture_output=True,text=True)
    return probe.returncode==0 and bool(json.loads(probe.stdout).get('streams'))
