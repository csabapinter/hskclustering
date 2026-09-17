"""Run the existing release binaries, preserving each randomized result separately."""
import argparse
import concurrent.futures
import datetime
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

ROOT = Path(__file__).resolve().parents[3]
OUT = Path(__file__).resolve().parent


def label(value):
    return format(value, '.3f').rstrip('0').rstrip('.')


def sha256(path):
    h = hashlib.sha256()
    with Path(path).open('rb') as f:
        for block in iter(lambda: f.read(1024 * 1024), b''):
            h.update(block)
    return h.hexdigest()


def run_one(job):
    tag, threshold, resolution, repeat, graph = job
    name = f'{tag}-t{label(threshold)}-r{label(resolution)}-run{repeat:02d}'
    directory = OUT / 'runs' / name
    directory.mkdir(parents=True, exist_ok=True)
    record = directory / 'run.json'
    if record.exists() and json.loads(record.read_text()).get('returncode') == 0:
        print('EXISTS', name, flush=True)
        return
    command = [str(ROOT / 'target/release/leiden_community'), '--input', str(graph),
               '--output', str(directory / 'communities.csv'), '--quality', 'cpm',
               '--resolution', str(resolution), '--gamma', str(resolution), '--theta', '0.3']
    metadata = dict(name=name, embedding=tag, threshold=threshold, raw_cosine_cutoff=2*threshold-1,
                    resolution=resolution, gamma=resolution, theta=0.3, quality='CPM',
                    repeat=repeat, graph=str(graph.relative_to(ROOT)), graph_sha256=sha256(graph),
                    command=command, random_seed=None,
                    started_utc=datetime.datetime.now(datetime.timezone.utc).isoformat())
    record.write_text(json.dumps(metadata, indent=2) + '\n')
    print('START', name, flush=True)
    started = time.monotonic()
    with (directory / 'stderr.log').open('w') as log:
        result = subprocess.run(command, cwd=ROOT, stdout=log, stderr=log)
    metadata.update(returncode=result.returncode, elapsed_seconds=time.monotonic()-started,
                    finished_utc=datetime.datetime.now(datetime.timezone.utc).isoformat())
    if result.returncode == 0:
        metadata['assignments_sha256'] = sha256(directory / 'communities.csv')
    record.write_text(json.dumps(metadata, indent=2) + '\n')
    print('DONE' if result.returncode == 0 else 'FAILED', name,
          round(metadata['elapsed_seconds'], 1), 'seconds', flush=True)
    if result.returncode:
        raise RuntimeError(f'{name} failed; see {directory / "stderr.log"}')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--embedding', default='raw')
    parser.add_argument('--thresholds', type=float, nargs='+', default=[0.7, 0.725, 0.75])
    parser.add_argument('--resolutions', type=float, nargs='+', default=[0.05, 0.1, 0.2])
    parser.add_argument('--start-repeat', type=int, default=1)
    parser.add_argument('--repeats', type=int, default=1)
    parser.add_argument('--workers', type=int, default=3)
    args = parser.parse_args()
    (OUT / 'graphs').mkdir(exist_ok=True)
    jobs = []
    for threshold in args.thresholds:
        if args.embedding == 'raw' and threshold == 0.7:
            graph = OUT.parent / 'hsk1-9-thresh0.70.graphml'
        else:
            graph = OUT / 'graphs' / f'{args.embedding}-t{label(threshold)}.graphml'
            if not graph.exists():
                if args.embedding != 'raw':
                    raise RuntimeError(f'First prepare {graph}')
                command = [str(ROOT / 'target/release/filter_graph'),
                           '--input', str(OUT.parent / 'hsk1-9-thresh0.70.graphml'),
                           '--output', str(graph), '--threshold', str(threshold)]
                with graph.with_suffix('.log').open('w') as log:
                    subprocess.run(command, cwd=ROOT, stdout=log, stderr=log, check=True)
        for resolution in args.resolutions:
            for repeat in range(args.start_repeat, args.start_repeat + args.repeats):
                jobs.append((args.embedding, threshold, resolution, repeat, graph))
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.workers) as executor:
        futures = [executor.submit(run_one, job) for job in jobs]
        for future in concurrent.futures.as_completed(futures):
            future.result()


if __name__ == '__main__':
    main()
