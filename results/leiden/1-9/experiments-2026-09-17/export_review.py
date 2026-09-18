"""Export study-friendly CSVs and fixed-anchor comparisons from completed runs."""
import argparse
import collections
import csv
import json
from pathlib import Path
import shutil

OUT=Path(__file__).resolve().parent
ROOT=OUT.parents[3]
ANCHORS=['苹果','牛奶','咖啡','衬衫','足球','老师','焦虑','政策','合同','银行','软件','感冒','火车','妈妈','为什么','公斤','花','行']


def read_csv(path):
    with path.open() as f:return list(csv.DictReader(f))


def write_csv(path,rows):
    with path.open('w',newline='') as f:
        fields=list(dict.fromkeys(k for r in rows for k in r))
        writer=csv.DictWriter(f,fieldnames=fields)
        writer.writeheader();writer.writerows(rows)


def main():
    parser=argparse.ArgumentParser()
    parser.add_argument('--recommend',required=True)
    args=parser.parse_args()
    dictionary=collections.defaultdict(list)
    for row in read_csv(ROOT/'data/hsk-data.csv'):
        for spelling in row['Simplified'].split('|'):
            dictionary[spelling.strip()].append(row)
    anchors=[];coverage=[]
    for directory in sorted((OUT/'runs').iterdir()):
        if not (directory/'clusters.csv').exists():continue
        parts={r['token']:r['community'] for r in read_csv(directory/'communities.csv')}
        groups={r['community']:r for r in read_csv(directory/'clusters.csv')}
        for word in ANCHORS:
            row=groups[parts[word]]
            anchors.append(dict(run=directory.name,anchor=word,community=row['community'],size=row['size'],
                                representatives=row['representatives'],sample=row['deterministic_sample'],
                                boundary_words=row['boundary_words']))
        levels=collections.defaultdict(list)
        for token in parts:
            level=min([r['Level'] for r in dictionary[token]],key=lambda v:int(v.split('-')[0]))
            levels[level].append(token)
        for level,words in levels.items():
            sizes=[int(groups[parts[t]]['size']) for t in words]
            coverage.append(dict(run=directory.name,earliest_hsk_level=level,words=len(words),
                                 singletons=sum(s==1 for s in sizes),words_in_size_3_to_30=sum(3<=s<=30 for s in sizes)))
    write_csv(OUT/'anchor-review.csv',anchors)
    write_csv(OUT/'level-coverage.csv',coverage)
    directory=OUT/'runs'/args.recommend
    record=json.loads((directory/'run.json').read_text())
    assert record['returncode']==0
    shutil.copyfile(directory/'communities.csv',OUT/'recommended.communities.csv')
    groups=read_csv(directory/'clusters.csv')
    stability={r['community']:r for r in read_csv(directory/'cluster-stability.csv')}
    for group in groups:
        group.update({k:v for k,v in stability[group['community']].items() if k not in ['community','size']})
    groups.sort(key=lambda r:-int(r['size']))
    write_csv(OUT/'recommended-clusters.csv',groups)
    word_stability={r['token']:r for r in read_csv(directory/'word-stability.csv')}
    words=read_csv(directory/'words.csv')
    for row in words:
        token=row['token']
        row['hsk_levels']='; '.join(sorted(set(r['Level'] for r in dictionary[token]),key=lambda v:int(v.split('-')[0])))
        row['pinyin']='; '.join(dict.fromkeys(r['Pinyin'] for r in dictionary[token]))
        translations=[];sources=[]
        for source_row in dictionary[token]:
            for field in ['Translation','Translation-CC-Cedict','Translation-Drkameleon']:
                if source_row[field]:
                    translations.append(source_row[field]);sources.append(field);break
        row['dictionary_translation']='; '.join(dict.fromkeys(translations))
        row['dictionary_translation_source']='; '.join(dict.fromkeys(sources))
        row['repeat_cluster_match_fraction']=word_stability[token]['best_cluster_match_fraction']
    words.sort(key=lambda r:(int(r['community']),-float(r['mean_cosine_to_own_others'])))
    write_csv(OUT/'recommended-word-details.csv',words)
    record['selection_rule']='Practical balance of coverage, usable group sizes, inspected semantics, and repeat stability; representative maximizes mean ARI to other four runs of this configuration.'
    (OUT/'recommendation.json').write_text(json.dumps(record,indent=2)+'\n')
    print('Exported',args.recommend,len(groups),'groups',len(words),'words')


if __name__=='__main__':main()
