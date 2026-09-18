"""Enrich the recorded GMM review shortlist with dictionary and soft-alternative data."""
import collections
import csv
import json
from pathlib import Path

OUT = Path(__file__).resolve().parent
ROOT = OUT.parents[3]


def read_csv(path):
    with path.open() as stream:
        return list(csv.DictReader(stream))


def write_csv(path, rows):
    with path.open('w', newline='') as stream:
        writer = csv.DictWriter(stream, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)


def main():
    selections = json.loads((OUT / 'review-selection.json').read_text())['runs']
    dictionary = collections.defaultdict(list)
    for row in read_csv(ROOT / 'data/hsk-data.csv'):
        for spelling in row['Simplified'].split('|'):
            dictionary[spelling.strip()].append(row)
    index = []
    ambiguous = []
    for selected in selections:
        directory = OUT / 'runs' / selected['run']
        groups = {r['community']: r for r in read_csv(directory / 'clusters.csv')}
        words = read_csv(directory / 'words.csv')
        members = collections.defaultdict(set)
        for row in words:
            members[row['community']].add(row['token'])
        other_partitions = []
        for path in sorted((OUT / 'jobs').glob('*.json')):
            job = json.loads(path.read_text())
            if job.get('returncode') != 0 or job['name'] == selected['run']:
                continue
            if all(job['configuration'][key] == value for key, value in selected['configuration'].items()):
                other_partitions.append({r['token']: r['community'] for r in read_csv(OUT / 'runs' / job['name'] / 'communities.csv')})
        assert len(other_partitions) == 4, f"Expected four completed repeat fits for {selected['run']}"
        word_matches = collections.Counter()
        stability = []
        for community, group in groups.items():
            own = members[community]
            similarities = []
            matched_every = set(own)
            for partition in other_partitions:
                if not own:
                    continue
                assert partition.keys() == {row['token'] for row in words}
                sizes = collections.Counter(partition.values())
                overlaps = collections.Counter(partition[t] for t in own)
                best = max(overlaps, key=lambda h: (overlaps[h] / (len(own) + sizes[h] - overlaps[h]), -int(h)))
                similarities.append(overlaps[best] / (len(own) + sizes[best] - overlaps[best]))
                matching = {t for t in own if partition[t] == best}
                matched_every.intersection_update(matching)
                word_matches.update(matching)
            item = dict(community=community, size=len(own),
                mean_best_match_jaccard=sum(similarities) / len(similarities) if similarities else '',
                min_best_match_jaccard=min(similarities) if similarities else '',
                fraction_members_in_best_match_every_repeat=len(matched_every) / len(own) if own else '')
            stability.append(item)
            group.update({key: value for key, value in item.items() if key not in ['community', 'size']})
        write_csv(directory / 'cluster-stability.csv', stability)
        write_csv(directory / 'review-clusters.csv', list(groups.values()))
        for row in words:
            entries = dictionary[row['token']]
            row['hsk_levels'] = '; '.join(sorted({r['Level'] for r in entries}, key=lambda v: int(v.split('-')[0])))
            row['pinyin'] = '; '.join(dict.fromkeys(r['Pinyin'] for r in entries))
            translations, sources = [], []
            for entry in entries:
                for field in ['Translation', 'Translation-CC-Cedict', 'Translation-Drkameleon']:
                    if entry[field]:
                        translations.append(entry[field])
                        sources.append(field)
                        break
            row['dictionary_translation'] = '; '.join(dict.fromkeys(translations))
            row['dictionary_translation_source'] = '; '.join(dict.fromkeys(sources))
            second = row['second_community']
            row['second_component_representatives'] = groups[second]['representatives'] if second else ''
            row['repeat_cluster_match_fraction'] = word_matches[row['token']] / 4
        words.sort(key=lambda row: (int(row['community']), -float(row['mean_cosine_to_own_others'] or -1), row['token']))
        write_csv(directory / 'word-details.csv', words)
        for row in sorted(words, key=lambda row: (float(row['membership_margin']), row['token']))[:30]:
            ambiguous.append(dict(run=selected['run'], **row,
                primary_component_representatives=groups[row['community']]['representatives']))
        metrics = json.loads((directory / 'metrics.json').read_text())
        index.append(dict(run=selected['run'], reason=selected['reason'],
            catalogue=f"runs/{selected['run']}/review-clusters.csv", words=f"runs/{selected['run']}/word-details.csv",
            probabilities=f"runs/{selected['run']}/memberships.csv", largest_share=metrics['largest_share'],
            tiny_hard_components=metrics['hard_components_size_1_to_5'], mean_max_membership=metrics['mean_max_membership']))
    write_csv(OUT / 'review-index.csv', index)
    write_csv(OUT / 'ambiguous-words.csv', ambiguous)
    print('Exported', len(index), 'review candidates;', len(ambiguous), 'ambiguous-word examples')


if __name__ == '__main__':
    main()
