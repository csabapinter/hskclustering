# HSK 1–9 Leiden baseline — 2026-09-17

This run uses every token in `data/input-embeddings.txt`: 10,936 distinct spellings
with 300-dimensional SGNS vectors, covering all HSK 1–9 bands. No PCA whitening.
The HSK 1–3 archive was not modified.

## Outputs

| File | Contents |
| --- | --- |
| `hsk1-9-full.graphml` | 10,936 nodes, 59,792,580 edges; 5,373,498,608 bytes. |
| `hsk1-9-thresh0.70.graphml` | Same 10,936 nodes, 303,267 edges; 27,503,450 bytes. |
| `hsk1-9-thresh0.70.communities.csv` | Exactly one assignment for each of the 10,936 tokens. |
| `missing-embeddings.csv` | 36 source vocabulary entries without a matched embedding. |

The complete GraphML is kept locally and ignored by Git because it is a
regenerable multi-GB artifact. The thresholded graph and CSV are small enough to
inspect independently.

Edge weights are `(cosine + 1) / 2`, using L2-normalized input vectors. The filtered
graph retains weights >= 0.70 and every input node. Its three isolated nodes are
`一次性`, `零` and `〇`.

## Clustering

- Quality function: CPM.
- Resolution: 0.05.
- Theta: 0.3.
- Gamma: 0.05.
- Backend: graphrs 0.12.0 with the [documented numerical patch](../../vendor/README.md).
- Result: 508 communities; median size 9.5; largest 450; 74 singletons.
- Successful Leiden execution: about 240 seconds, using the release build.

The stock crate first panicked during refinement with a floating-point sampling
range overflow. The local patch stabilizes its sampling weights without changing
the intended probabilities or any baseline setting. The patched run completed.
The upstream RNG has no exposed seed; future runs may produce different
partitions, so the counts above describe this saved CSV.

## Reproduction

From the repository root:

```sh
cargo run --release --locked --bin build_graph
cargo run --release --locked --bin build_graph -- --threshold 0.70
cargo run --release --locked --bin leiden_community
```

Alternatively, `cargo run --release --locked --bin filter_graph` derives the
thresholded file from the complete export. Both routes were checked to produce
exactly the same node set, endpoints and edge weights (XML whitespace may differ).

Validation used Rust 1.95.0 on aarch64-apple-darwin. The full GraphML was parsed
through the streaming filter, which counted all 59,792,580 edges. A separate XML
comparison verified equality with the direct thresholded export. The saved CSV
has no duplicate or missing tokens, and a connectivity check found that every
community induces a connected subgraph (including singleton communities).

## Coverage

`data/hsk-data.csv` contains 11,092 entries. 36 have no matching spelling in the
embedding input, listed in `missing-embeddings.csv`. Multiple source entries can
share spellings, and entries can contain alternative spellings, so source entry
counts do not directly equal graph node counts. No missing vectors were fabricated
and no source vocabulary or embeddings were edited.

## SHA-256

```text
5dcd7e46ec00a70d92adc4510638c73b0642bd2f09afea053bea38bd60cacafa  data/input-embeddings.txt
17b592a3a9d42d4ffacb0686d326c7efbfe380327c3671563108b984af948e24  data/hsk-data.csv
91ad80d95eae860f13ad3ca98928aa9977b040aa428d50a2412c0b7666615b1d  Cargo.lock
624f07be031bce30e56a2989f1c8af8c69b8a231d751cf8026b4b81334bf111b  hsk1-9-full.graphml
64fee58e649365ce1c4d4aec95b5102e77afcc1a565be9ff6d398658dd8fcaa5  hsk1-9-thresh0.70.graphml
797476e0e04a5ad0d980b26d3bf7973d6bda4e1a543d5da16038654b17e51b27  hsk1-9-thresh0.70.communities.csv
4de1e78c96ecdca8d4792e963821113c6cea87deaa76aaf6f8dab04547722065  vendor/graphrs-0.12.0/src/algorithms/community/leiden/mod.rs
```
