import tempfile,unittest,wave
from pathlib import Path
from audio_library import scan
class AudioTests(unittest.TestCase):
 def test_import_audio_and_skip_corrupt_file(self):
  with tempfile.TemporaryDirectory() as d:
   folder=Path(d);p=folder/'podcast_sample.wav'
   with wave.open(str(p),'wb') as w:w.setnchannels(1);w.setsampwidth(2);w.setframerate(8000);w.writeframes(b'\0\0'*8000)
   (folder/'broken.mp3').write_bytes(b'not audio')
   items,skipped=scan(folder);self.assertEqual(len(items),1);self.assertEqual(items[0]['seconds'],1);self.assertEqual(items[0]['tag'],'Podcast');self.assertEqual(len(skipped),1)
if __name__=='__main__':unittest.main()
