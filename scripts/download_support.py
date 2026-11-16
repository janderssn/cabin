"""Shared best-effort source processing; failures never suppress later sources."""
import json
import sys

def run_sources(entries, prepare, skipped):
    for entry in entries:
        try:
            prepare(entry)
        except Exception as error:
            failure = {'id': entry.get('id', ''), 'source': entry.get('url', entry.get('id', '')), 'error': str(error)}
            skipped.append(failure)
            print('Skipped source: ' + json.dumps(failure), file=sys.stderr, flush=True)
