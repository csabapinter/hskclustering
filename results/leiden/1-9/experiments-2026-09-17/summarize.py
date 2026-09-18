"""Validate saved experiments and compute graph statistics and repeat stability."""
import collections
import csv
import itertools
import json
from pathlib import Path
import statistics
import subprocess
import xml.etree.ElementTree as ET

OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[3]


def write_csv(path, rows):
    if not rows:
        return
    fields = list(dict.fromkeys(key for row in rows for key in row))
    with path.open('w', newline='') as f:
        writer = csv.DictWriter(f, fieldnames=fields)
        writer.writeheader()
        writer.writerows(rows)


def assignments(path):
    with path.open() as f:
        return {r['token']: int(r['community']) for r in csv.DictReader(f)}


def choose2(n):
    return n * (n - 1) // 2


def ari(a, b):
    assert a.keys() == b.keys()
    left = collections.Counter(a.values())
    right = collections.Counter(b.values())
    joint = collections.Counter((a[t], b[t]) for t in a)
    x = sum(map(choose2, left.values()))
    y = sum(map(choose2, right.values()))
    z = sum(map(choose2, joint.values()))
    expected = x * y / choose2(len(a))
    denominator = (x + y) / 2 - expected
    return (z - expected) / denominator if denominator else 1.0


def read_graph(path):
    nodes = []
    edges = []
    for _, elem in ET.iterparse(path, events=('end',)):
        tag = elem.tag.rsplit('}', 1)[-1]
        if tag == 'node':
            nodes.append(elem.attrib['id'])
            elem.clear()
        elif tag == 'edge':
            edges.append((elem.attrib['source'], elem.attrib['target'], float(list(elem)[0].text)))
            elem.clear()
    return nodes, edges


def components(nodes, edges, partition=None):
    parent = {t: t for t in nodes}

    def find(t):
        while parent[t] != t:
            parent[t] = parent[parent[t]]
            t = parent[t]
        return t

    for u, v, _ in edges:
        if partition is None or partition[u] == partition[v]:
            ru, rv = find(u), find(v)
            if ru != rv:
                parent[rv] = ru
    return collections.Counter(find(t) for t in nodes), {t: find(t) for t in nodes}


def main():
    records = []
    parts = {}
    graphs = {}
    graph_rows = []
    for file in sorted((OUT / 'runs').glob('*/run.json')):
        record = json.loads(file.read_text())
        if record.get('returncode') != 0:
            continue
        directory = file.parent
        if not (directory / 'metrics.json').exists():
            subprocess.run([str(OUT / 'experiment_tools'), 'analyse', str(ROOT / 'data/input-embeddings.txt'),
                            str(directory / 'communities.csv'), str(directory)], check=True,
                           stdout=subprocess.DEVNULL)
        record.update(json.loads((directory / 'metrics.json').read_text()))
        partition = assignments(directory / 'communities.csv')
        assert len(partition) == 10936
        parts[record['name']] = partition
        # Saved run metadata keeps the original command paths.
        graph_path = record['graph'].replace('graphs/1-9-leiden/', 'results/leiden/1-9/', 1)
        if graph_path not in graphs:
            nodes, edges = read_graph(ROOT / graph_path)
            graphs[graph_path] = (nodes, edges)
            degree = collections.Counter()
            for u, v, _ in edges:
                degree[u] += 1
                degree[v] += 1
            sizes, _ = components(nodes, edges)
            ds = sorted(degree[t] for t in nodes)
            graph_rows.append(dict(graph=graph_path, nodes=len(nodes), edges=len(edges),
                                   mean_degree=statistics.mean(ds), median_degree=statistics.median(ds),
                                   p90_degree=ds[int(.9*(len(ds)-1))], max_degree=max(ds),
                                   isolates=sum(v == 0 for v in ds), connected_components=len(sizes),
                                   largest_connected_component=max(sizes.values())))
        nodes, edges = graphs[graph_path]
        assert set(nodes) == partition.keys()
        sizes = collections.Counter(partition.values())
        word_sizes = sorted(size for size in sizes.values() for _ in range(size))
        record['word_weighted_median_cluster_size'] = statistics.median(word_sizes)
        for low, high, field in [(1,2,'words_in_1_to_2'),(3,9,'words_in_3_to_9'),
                                 (10,30,'words_in_10_to_30'),(31,100,'words_in_31_to_100')]:
            record[field] = sum(size for size in sizes.values() if low <= size <= high)
        internal_weight = collections.Counter()
        internal_edges = collections.Counter()
        for u, v, w in edges:
            if partition[u] == partition[v]:
                internal_weight[partition[u]] += w
                internal_edges[partition[u]] += 1
        _, roots = components(nodes, edges, partition)
        cluster_roots = collections.defaultdict(set)
        for t in nodes:
            cluster_roots[partition[t]].add(roots[t])
        disconnected = sum(len(rs) != 1 for rs in cluster_roots.values())
        assert disconnected == 0, (record['name'], disconnected)
        graph_clusters = [dict(community=c, size=size, internal_edges=internal_edges[c],
                               weighted_density=internal_weight[c]/choose2(size) if size > 1 else '',
                               connected=True) for c, size in sorted(sizes.items())]
        write_csv(directory / 'graph-clusters.csv', graph_clusters)
        record['cpm_objective_same_graph_and_resolution_only'] = sum(internal_weight.values()) - record['resolution']*sum(map(choose2,sizes.values()))
        record['fraction_graph_edges_within_communities'] = sum(internal_edges.values())/len(edges)
        record['disconnected_communities'] = disconnected
        for key in ('command','started_utc','finished_utc','returncode','graph_sha256','assignments_sha256','random_seed'):
            record.pop(key, None)
        records.append(record)
    write_csv(OUT / 'run-summary.csv', records)
    write_csv(OUT / 'graph-summary.csv', graph_rows)
    pair_rows = []
    repeat_summary = []
    configs = collections.defaultdict(list)
    for r in records:
        configs[(r['embedding'],r['threshold'],r['resolution'])].append(r['name'])
    for (embedding,threshold,resolution),names in sorted(configs.items()):
        if len(names)<2:
            continue
        similarities = collections.defaultdict(list)
        scores = []
        for a,b in itertools.combinations(names,2):
            score=ari(parts[a],parts[b]);scores.append(score)
            similarities[a].append(score);similarities[b].append(score)
            pair_rows.append(dict(run_a=a,run_b=b,adjusted_rand_index=score))
        representative=max(names,key=lambda name:statistics.mean(similarities[name]))
        repeat_summary.append(dict(embedding=embedding,threshold=threshold,resolution=resolution,
                                   runs=len(names),mean_ari=statistics.mean(scores),min_ari=min(scores),
                                   max_ari=max(scores),representative_run=representative))
        reference=parts[representative]
        ref_groups=collections.defaultdict(set)
        for token,community in reference.items():ref_groups[community].add(token)
        stability=collections.defaultdict(list)
        word_agreement=collections.Counter()
        for name in names:
            if name==representative:continue
            other=parts[name];other_groups=collections.defaultdict(set)
            for token,community in other.items():other_groups[community].add(token)
            for community,members in ref_groups.items():
                overlaps=collections.Counter(other[t] for t in members)
                best=max(overlaps,key=lambda c:overlaps[c]/len(members|other_groups[c]))
                stability[community].append(overlaps[best]/len(members|other_groups[best]))
                for t in members & other_groups[best]:word_agreement[t]+=1
        write_csv(OUT/'runs'/representative/'cluster-stability.csv',[
            dict(community=c,size=len(m),mean_best_match_jaccard=statistics.mean(stability[c]),
                 min_best_match_jaccard=min(stability[c]),
                 fraction_members_in_best_match_every_repeat=sum(word_agreement[t]==len(names)-1 for t in m)/len(m))
            for c,m in sorted(ref_groups.items())])
        write_csv(OUT/'runs'/representative/'word-stability.csv',[
            dict(token=t,community=reference[t],best_cluster_match_fraction=word_agreement[t]/(len(names)-1))
            for t in sorted(reference)])
    write_csv(OUT / 'stability-pairs.csv',pair_rows)
    write_csv(OUT / 'stability-summary.csv',repeat_summary)
    config_rows=[]
    for (embedding,threshold,resolution),names in sorted(configs.items()):
        selected=[r for r in records if r['name'] in names]
        row=dict(embedding=embedding,threshold=threshold,resolution=resolution,runs=len(selected))
        for field in ['communities','largest','singletons','words_in_1_to_2','words_in_3_to_30',
                      'words_in_10_to_30','words_in_over_100','word_weighted_median_cluster_size',
                      'mean_pair_cosine_word_weighted_nonsingletons','mean_cosine_silhouette_original_300d']:
            values=[r[field] for r in selected]
            row[field+'_mean']=statistics.mean(values)
            row[field+'_min']=min(values)
            row[field+'_max']=max(values)
        config_rows.append(row)
    write_csv(OUT / 'config-summary.csv',config_rows)
    columns=['name','communities','median_size','largest','singletons','words_in_3_to_30',
             'words_in_over_100','mean_pair_cosine_word_weighted_nonsingletons',
             'mean_cosine_silhouette_original_300d']
    for r in records:
        print(' | '.join(str(round(r[c],4)) if isinstance(r[c],float) else str(r[c]) for c in columns))
    for r in repeat_summary:print('STABILITY',json.dumps(r))


if __name__=='__main__':
    main()
