"""Validate the experiment archive against stage manifests and native outputs."""
import collections
import csv
import hashlib
import json
import math
from pathlib import Path
import subprocess

OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[3]


def sha256(path):
    digest = hashlib.sha256()
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b''):
            digest.update(block)
    return digest.hexdigest()


def tag(job):
    return (f"pca{job['dimensions']}-{'white' if job['whiten'] else 'plain'}"
            f"-k{job['clusters']}-reg{job['regularization']:g}-seed{job['seed']}")


def csv_rows(path):
    with path.open() as stream:
        return list(csv.DictReader(stream))


def write_checksums():
    paths = sorted(p for p in OUT.rglob('*') if p.is_file() and p.name != 'checksums.sha256')
    paths += [ROOT / 'examples/gmm_review.rs']
    names = [str(path.relative_to(ROOT)) for path in paths]
    result = subprocess.run(['git', 'check-ignore', '--no-index', '-z', '--stdin'],
                            cwd=ROOT, input='\0'.join(names) + '\0', text=True,
                            capture_output=True)
    if result.returncode not in (0, 1):
        raise RuntimeError(result.stderr)
    ignored = set(result.stdout.split('\0'))
    with (OUT / 'checksums.sha256').open('w') as stream:
        for path, name in zip(paths, names):
            if name not in ignored:
                stream.write(sha256(path) + '  ' + name + '\n')


def main():
    provenance = json.loads((OUT / 'provenance.json').read_text())
    assert provenance['input_sha256'] == sha256(ROOT / 'data/input-embeddings.txt')
    assert provenance['dictionary_sha256'] == sha256(ROOT / 'data/hsk-data.csv')
    assert provenance['binary_sha256'] == sha256(ROOT / 'target/release/gmm_cluster')
    assert provenance['cargo_lock_sha256'] == sha256(ROOT / 'Cargo.lock')
    expected = {}
    for path in sorted(OUT.glob('stage*.json')):
        if path.name.endswith('-execution.json'):
            continue
        stage = json.loads(path.read_text())
        for config in stage['jobs']:
            name = tag(config)
            assert name not in expected
            expected[name] = config
    actual = {p.stem: json.loads(p.read_text()) for p in (OUT / 'jobs').glob('*.json')}
    assert actual.keys() == expected.keys()
    with (ROOT / 'data/input-embeddings.txt').open() as stream:
        n, d = map(int, next(stream).split())
        tokens = {line.split()[0] for line in stream if line.strip()}
    failures, checked, pcas = [], [], {}
    max_sum_error = 0.0
    for name, job in sorted(actual.items()):
        assert job['configuration'] == expected[name]
        assert 'returncode' in job, f'Job still running: {name}'
        directory = OUT / 'runs' / name
        if job['returncode'] != 0:
            failures.append(dict(run=name, returncode=job['returncode']))
            continue
        run = json.loads((directory / 'run.json').read_text())
        assert run['method'] == 'gmm' and run['tokens'] == n
        assert run['input_sha256'] == provenance['input_sha256']
        config = expected[name]
        assert run['pca_components'] == config['dimensions'] and run['whiten'] == config['whiten']
        for key in ['clusters', 'regularization', 'seed']:
            assert run['selected']['config'][key] == config[key]
        scores = run['selected']['scores']
        k, dim = config['clusters'], config['dimensions']
        parameters = k - 1 + k * dim + k * dim * (dim + 1) // 2
        assert scores['parameters'] == parameters
        assert math.isclose(scores['aic'], 2 * parameters - 2 * scores['log_likelihood'], rel_tol=1e-12)
        assert math.isclose(scores['bic'], math.log(n) * parameters - 2 * scores['log_likelihood'], rel_tol=1e-12)
        rows = csv_rows(directory / 'communities.csv')
        assignments = {r['token']: int(r['community']) for r in rows}
        assert len(rows) == len(assignments) == n and assignments.keys() == tokens
        assert all(0 <= g < k for g in assignments.values())
        assert sha256(directory / 'communities.csv') == job['assignments_sha256']
        groups = csv_rows(directory / 'clusters.csv')
        assert len(groups) == k and sum(int(g['size']) for g in groups) == n
        for group in groups:
            members = group['all_members'].split(' | ') if group['all_members'] else []
            assert len(members) == int(group['size'])
            assert all(assignments[t] == int(group['community']) for t in members)
        review = json.loads((directory / 'review-validation.json').read_text())
        assert review['tokens_verified'] == n and review['hard_assignments_equal_argmax']
        assert review['all_probabilities_finite_in_unit_interval'] and review['cluster_sizes_sum_to_tokens']
        assert review['assignments_sha256'] == job['assignments_sha256']
        assert review['memberships_sha256'] == sha256(directory / 'memberships.csv')
        assert abs(review['soft_mass_sum'] - n) < 1e-6
        max_sum_error = max(max_sum_error, review['max_probability_sum_error'])
        model = json.loads((directory / 'model.json').read_text())
        assert model['weights']['dim'] == [k] and abs(sum(model['weights']['data']) - 1) < 1e-8
        assert model['means']['dim'] == [k, dim]
        assert model['covariances']['dim'] == [k, dim, dim]
        for field in ['weights', 'means', 'covariances', 'precisions', 'precisions_chol']:
            assert all(math.isfinite(v) for v in model[field]['data'])
        pca = json.loads((directory / 'pca.json').read_text())
        assert len(pca['mean']) == d and len(pca['components']) == dim
        assert all(len(c) == d for c in pca['components'])
        assert len(pca['explained_variance']) == dim and all(v > 0 for v in pca['explained_variance'])
        assert pca['whiten'] == config['whiten'] and 0 < pca['retained_variance_ratio'] <= 1
        key = (dim, config['whiten'])
        digest = sha256(directory / 'pca.json')
        if key in pcas:
            assert pcas[key]['sha256'] == digest
        else:
            residual = max(abs(sum(a * b for a, b in zip(left, right)) - (i == j))
                           for i, left in enumerate(pca['components']) for j, right in enumerate(pca['components']))
            assert residual < 1e-8
            pcas[key] = dict(dimensions=dim, whiten=config['whiten'], sha256=digest, max_orthonormality_error=residual)
        checked.append(name)
    repeats = collections.Counter((j['dimensions'], j['whiten'], j['clusters'], j['regularization'])
                                  for j in expected.values())
    report = dict(expected_jobs=len(expected), completed_jobs=len(actual), successful_fits=len(checked),
        failed_fits=failures, tokens_per_fit=n, max_probability_sum_error=max_sum_error,
        pca_representations=list(pcas.values()), configurations_with_five_seeds=sum(v == 5 for v in repeats.values()),
        input_and_dictionary_hashes_unchanged=True, model_parameters_finite=True,
        information_criteria_independently_recomputed=True, all_assignments_cover_exact_vocabulary=True,
        all_cluster_catalogues_match_assignments=True, probability_exports_checked_by_review_tool=True,
        independent_geometry_check='independent-geometry-check.json',
        scope='GMM-only numerical/artifact validation; no claim of semantic accuracy')
    (OUT / 'validation.json').write_text(json.dumps(report, indent=2) + '\n')
    write_checksums()
    print(json.dumps({k: report[k] for k in ['expected_jobs','successful_fits','failed_fits','configurations_with_five_seeds','max_probability_sum_error']}, indent=2))


if __name__ == '__main__':
    main()
