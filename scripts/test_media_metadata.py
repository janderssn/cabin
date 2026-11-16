import tempfile,unittest
from pathlib import Path
from media_metadata import has_subtitles
class SubtitleTests(unittest.TestCase):
 def test_brackets_in_video_name_are_literal(self):
  with tempfile.TemporaryDirectory() as d:
   p=Path(d)/'Film [abc].mkv';p.with_suffix('.en.vtt').write_text('WEBVTT\n\n00:00.000 --> 00:01.000\nHello\n')
   self.assertTrue(has_subtitles(p))
if __name__=='__main__':unittest.main()
