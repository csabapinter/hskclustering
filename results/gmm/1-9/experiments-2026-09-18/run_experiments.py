"""Run a recorded GMM stage using the repository binary and Python's standard library."""
import argparse
import concurrent.futures
import datetime
import hashlib
import json
from pathlib import Path
import subprocess
import time

OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[3]


def sha256(path):
    digest = hashlib.sha256()
    with Path(path).open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def write_json(path, value):
    temporary = path.with_suffix(path.suffix + '.tmp')
    temporary.write_text(json.dumps(value, ensure_ascii=False, indent=2) + '\n')
    temporary.replace(path)


def tag(job):
    return (f"pca{job['dimensions']}-{'white' if job['whiten'] else 'plain'}"
            f"-k{job['clusters']}-reg{job['regularization']:g}-seed{job['seed']}")


def run_one(job, stage):
    name = tag(job)
    directory = OUT / 'runs' / name
    record = OUT / 'jobs' / (name + '.json')
    if record.exists():
        previous = json.loads(record.read_text())
        if previous.get('returncode') == 0 and (directory / 'run.json').exists():
            print('EXISTS', name, flush=True)
            return previous
        raise RuntimeError(f'Incomplete/failed job exists: {name}; preserve it and use an explicit new job')
    command = [str(ROOT / 'target/release/gmm_cluster'), '--input', str(ROOT / 'data/input-embeddings.txt'),
               '--output', str(directory), '--pca-components', str(job['dimensions']),
               '--clusters', str(job['clusters']), '--regularization', str(job['regularization']),
               '--seeds', str(job['seed']), '--max-iterations', str(job.get('max_iterations', 300)),
               '--tolerance', str(job.get('tolerance', 0.001)), '--criterion', 'bic']
    if job['whiten']:
        command.append('--whiten')
    metadata = dict(name=name, stage=stage, configuration=job, command=command, started_utc=now())
    write_json(record, metadata)
    print('START', name, flush=True)
    started = time.monotonic()
    with (OUT / 'logs' / (name + '.log')).open('w') as log:
        try:
            result = subprocess.run(command, cwd=ROOT, stdout=log, stderr=log, timeout=900)
            metadata['returncode'] = result.returncode
        except subprocess.TimeoutExpired:
            metadata.update(returncode=124, error='Per-fit 900-second timeout')
    metadata.update(finished_utc=now(), elapsed_seconds=time.monotonic() - started)
    if metadata['returncode'] == 0:
        metadata['assignments_sha256'] = sha256(directory / 'communities.csv')
    write_json(record, metadata)
    print('DONE' if metadata['returncode'] == 0 else 'FAILED', name,
          round(metadata['elapsed_seconds'], 2), 'seconds', flush=True)
    return metadata


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('stage', help='JSON stage manifest, relative to this directory')
    parser.add_argument('--workers', type=int, default=2)
    args = parser.parse_args()
    stage_path = OUT / args.stage
    stage = json.loads(stage_path.read_text())
    jobs = stage['jobs']
    assert args.workers > 0 and len({tag(job) for job in jobs}) == len(jobs)
    for name in ['runs', 'jobs', 'logs']:
        (OUT / name).mkdir(exist_ok=True)
    provenance_path = OUT / 'provenance.json'
    fingerprint = dict(
        git_commit=subprocess.check_output(['git', 'rev-parse', 'HEAD'], cwd=ROOT, text=True).strip(),
        input_sha256=sha256(ROOT / 'data/input-embeddings.txt'),
        dictionary_sha256=sha256(ROOT / 'data/hsk-data.csv'),
        cargo_lock_sha256=sha256(ROOT / 'Cargo.lock'),
        binary_sha256=sha256(ROOT / 'target/release/gmm_cluster'))
    if provenance_path.exists():
        previous = json.loads(provenance_path.read_text())
        assert all(previous[key] == value for key, value in fingerprint.items()), 'Experiment inputs or binary changed'
    else:
        write_json(provenance_path, dict(created_utc=now(), **fingerprint,
            rustc=subprocess.check_output(['rustc', '--version'], text=True).strip(),
            scope='GMM only; semantic relevance without a target granularity; no cross-method comparison',
            covariance='full', input_tokens=10936))
    with concurrent.futures.ThreadPoolExecutor(max_workers=args.workers) as pool:
        futures = [pool.submit(run_one, job, stage['stage']) for job in jobs]
        results = [future.result() for future in concurrent.futures.as_completed(futures)]
    results.sort(key=lambda row: row['name'])
    write_json(OUT / (stage_path.stem + '-execution.json'),
               dict(stage=stage['stage'], completed_utc=now(), jobs=results))
    print('STAGE COMPLETE', stage['stage'], len(results), 'jobs;',
          sum(row['returncode'] != 0 for row in results), 'failures', flush=True)


if __name__ == '__main__':
    main()
