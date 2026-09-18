"""Produce GMM-only experiment tables and deterministic linguistic review samples."""
import collections
import csv
import hashlib
import itertools
import json
from pathlib import Path
import statistics
import subprocess

OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[3]
ANCHORS = ['猫', '狗', '鸟', '牛奶', '米饭', '咖啡', '医生', '感冒', '医院', '学校',
           '数学', '老师', '银行', '合同', '工资', '火车', '飞机', '足球', '篮球',
           '软件', '电脑', '政策', '法律', '快乐', '悲伤', '为什么', '可以', '花', '行', '苹果']


def read_csv(path):
    with path.open() as stream:
        return list(csv.DictReader(stream))


def write_csv(path, rows):
    if not rows:
        return
    fields = list(dict.fromkeys(key for row in rows for key in row))
    with path.open('w', newline='') as stream:
        writer = csv.DictWriter(stream, fieldnames=fields)
        writer.writeheader()
        writer.writerows(rows)


def ari(left, right):
    assert left.keys() == right.keys()
    choose = lambda n: n * (n - 1) / 2
    a = sum(choose(n) for n in collections.Counter(left.values()).values())
    b = sum(choose(n) for n in collections.Counter(right.values()).values())
    c = sum(choose(n) for n in collections.Counter((left[t], right[t]) for t in left).values())
    if len(left) < 2:
        return 1.0
    expected = a * b / choose(len(left))
    denominator = (a + b) / 2 - expected
    return (c - expected) / denominator if denominator else 1.0


def main():
    jobs = [json.loads(p.read_text()) for p in sorted((OUT / 'jobs').glob('*.json'))]
    completed = [job for job in jobs if job.get('returncode') == 0]
    pending = [OUT / 'runs' / job['name'] for job in completed
               if not (OUT / 'runs' / job['name'] / 'review-validation.json').exists()]
    if pending:
        subprocess.run([str(ROOT / 'target/release/examples/gmm_review'), *map(str, pending)], cwd=ROOT, check=True)
    rows, anchors, sample, assignments = [], [], [], {}
    configurations = collections.defaultdict(list)
    for job in jobs:
        row = dict(run=job['name'], stage=job['stage'], **job['configuration'], returncode=job.get('returncode'))
        directory = OUT / 'runs' / job['name']
        if job.get('returncode') != 0:
            row['error'] = job.get('error', 'See fit log and candidates.csv')
            rows.append(row)
            continue
        run = json.loads((directory / 'run.json').read_text())
        metrics = json.loads((directory / 'metrics.json').read_text())
        metrics.pop('interpretation')
        row.update(run['selected']['scores'], fit_seconds=run['candidates'][0]['elapsed_seconds'], **metrics)
        rows.append(row)
        groups = {r['community']: r for r in read_csv(directory / 'clusters.csv')}
        parts = {r['token']: r['community'] for r in read_csv(directory / 'communities.csv')}
        assignments[job['name']] = parts
        for token in ANCHORS:
            if token not in parts:
                continue
            group = groups[parts[token]]
            anchors.append(dict(run=job['name'], anchor=token, community=group['community'], size=group['size'],
                representatives=group['representatives'], sample=group['deterministic_sample'], boundary_words=group['boundary_words'],
                soft_alternatives=group['strongest_members_outside_hard_cluster']))
        nonempty = [g for g in groups.values() if int(g['size'])]
        smallest = min(nonempty, key=lambda g: (int(g['size']), int(g['community'])))
        largest = max(nonempty, key=lambda g: (int(g['size']), -int(g['community'])))
        weakest = min(nonempty, key=lambda g: (float(g['mean_pair_cosine'] or 0), int(g['community'])))
        dispersed = sorted(nonempty, key=lambda g: hashlib.sha256(('gmm-review:' + g['community']).encode()).digest())[:3]
        selected = {}
        for reason, group in [('largest', largest), ('smallest', smallest), ('lowest_original_space_cohesion', weakest),
                              *[('deterministic_component_sample', g) for g in dispersed]]:
            entry = selected.setdefault(group['community'], dict(run=job['name'], selection_reason='', **group,
                semantic_themes='', semantic_relevance_notes='', linguistic_review_status='unreviewed'))
            entry['selection_reason'] += ('; ' if entry['selection_reason'] else '') + reason
        sample.extend(selected.values())
        key = tuple(job['configuration'][k] for k in ['dimensions', 'whiten', 'clusters', 'regularization'])
        configurations[key].append(row)
    rows.sort(key=lambda r: (r['dimensions'], r['whiten'], r['clusters'], r['regularization'], r['seed']))
    write_csv(OUT / 'run-summary.csv', rows)
    write_csv(OUT / 'anchor-review.csv', anchors)
    write_csv(OUT / 'linguistic-review-sample.csv', sample)
    pairs, stability = [], []
    for config, group in sorted(configurations.items()):
        if len(group) < 2:
            continue
        values = []
        for a, b in itertools.combinations(group, 2):
            value = ari(assignments[a['run']], assignments[b['run']])
            values.append(value)
            pairs.append(dict(left=a['run'], right=b['run'], adjusted_rand_index=value))
        stability.append(dict(zip(['dimensions','whiten','clusters','regularization'], config),
            repeats=len(group), mean_ari=statistics.mean(values), min_ari=min(values), max_ari=max(values),
            mean_largest_share=statistics.mean(r['largest_share'] for r in group),
            mean_pair_cosine=statistics.mean(r['mean_pair_cosine'] for r in group),
            mean_max_membership=statistics.mean(r['mean_max_membership'] for r in group)))
    write_csv(OUT / 'stability-pairs.csv', pairs)
    write_csv(OUT / 'stability-summary.csv', stability)
    print('Summarized', len(rows), 'jobs;', len(completed), 'successful;', len(sample), 'review samples', flush=True)


if __name__ == '__main__':
    main()
