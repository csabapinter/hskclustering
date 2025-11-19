### summary
- experimenting with community detection using graphrs and linfa crates
- main goal is to group Chinese words with similar context/meaning into communities to make studying them easier
- after running Leiden community detection, roughly 72% of words now belong to clearly distinguishable thematic communities

### input data I used 
- https://github.com/Embedding/Chinese-Word-Vectors
- sgns, 300d, word + character + ngram
- filtered only for HSK 1-3 level (newest HSK system) for testing purposes

### example usage
- build fully connected graph with `build_graph` and save it as graphml for next steps
- use --preprocess flag if embeddings are noisy or anisotropic, dominated by a few components
- check basic graph structure and statistics with `observe_graph`
- based on your observations, trim noisy edges with `filter_graph`
- check communities with `leiden_community`, iterate adjusting its parameters

### Leiden parameters and some notes about them
- quality_function: 
  - CPM for weight-aware resolution-agnostic clustering 
  - Modularity has a resolution limit.
- resolution: 
  - higher: more, smaller communities 
  - lower: fewer, larger ones
- gamma (CPM only): 
  - similar to resolution for further adjustments
  - higher vs. lower has similar effect on communities
- theta: controls randomness in Leiden’s refinement, leave at default unless tuning stability vs. exploration

