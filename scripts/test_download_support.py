import unittest
from download_support import run_sources
class DownloadTests(unittest.TestCase):
    def test_failure_does_not_abort_later_source(self):
        completed=[]; skipped=[]
        def prepare(entry):
            if entry['id']=='bad':raise OSError('HTTP 403')
            completed.append(entry['id'])
        run_sources([{'id':'good1'},{'id':'bad'},{'id':'good2'}],prepare,skipped)
        self.assertEqual(completed,['good1','good2'])
        self.assertEqual(skipped[0]['id'],'bad')
        self.assertEqual(len(skipped),1)
if __name__=='__main__':unittest.main()
