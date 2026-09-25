#!/usr/bin/env python3
"""Reproduce the frozen core-study shortlist and fixed-anchor inspection exports.

Run from the repository root: python3 scripts/report_graph_v2.py STUDY_DIRECTORY
Uses only the Python standard library. Does not fit or select a new default.
"""
import argparse
import csv
import datetime
import hashlib
import json
import statistics
import subprocess
from collections import Counter, defaultdict
from pathlib import Path


def read(path):
    return json.loads(Path(path).read_text())


def write_json(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n")


def write_csv(path, rows):
    if not rows:
        return
    with path.open("w", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)


def median(values):
    values = [v for v in values if v is not None]
    return statistics.median(values) if values else None


def name(candidate):
    config = candidate["source"]["config"]
    if candidate["baseline"]:
        return "seeded baseline"
    if candidate["source"]["kind"] == "archived":
        suffix = f'q={candidate["q"]:.6g}' if config["unshift"] else f'r={candidate["resolution"]:.6g}'
        return f'control t={config["threshold"]} {"unshifted" if config["unshift"] else "stored"} {suffix}'
    return f'{config["weight"]} k={config["k"]} q={candidate["q"]:.6g}'


def summarize(candidates, manifest, development, limits):
    policy = manifest["selection"]
    baseline = next(c for c in candidates if c["baseline"])
    bm = baseline["score"]["metrics"]
    tolerances = {m["metric"]: m["absolute_tolerance"] for m in policy["metrics"]}
    rows = []
    for c in candidates:
        score = c["score"]
        cfg = c["source"]["config"]
        graph = read(Path(c["graph_directory"]) / "graph-record.json")
        ms = [r["metrics"] for r in c["runs"] if r["metrics"] is not None]
        regressions, improvements, normalized = [], [], []
        for metric, tolerance in tolerances.items():
            if score["metrics"][metric] is None or bm[metric] is None:
                regressions.append(metric + ":missing")
                normalized.append(float("inf"))
                continue
            delta = score["metrics"][metric] - bm[metric]
            normalized.append(max(0, -delta / tolerance))
            if delta < -tolerance:
                regressions.append(metric)
            if delta > tolerance:
                improvements.append(metric)
        guard_failures = []
        if not score["valid"]:
            guard_failures.append("invalid/incomplete")
        if score["minimum_coverage"] < policy["minimum_nonsingleton_coverage"]:
            guard_failures.append("coverage")
        if score["maximum_largest_share"] > policy["maximum_largest_community_share"]:
            guard_failures.append("concentration")
        if score["minimum_communities"] < policy["minimum_communities"]:
            guard_failures.append("community_count")
        if policy["reject_all_singletons"] and score["minimum_coverage"] == 0:
            guard_failures.append("all_singletons")
        if any(score["metrics"][m] is None for m in tolerances):
            guard_failures.append("missing_metric")
        row = dict(id=c["id"], name=name(c), stage=c["stage"], baseline=c["baseline"],
                   k=cfg.get("k"), weight=cfg.get("weight"), q=c["q"], resolution=c["resolution"],
                   edges=graph["metrics"]["edges"], isolates=graph["metrics"]["isolates"],
                   valid=score["valid"], pareto=c["id"] in development["pareto"],
                   guards_pass=not guard_failures, guard_failures=";".join(guard_failures),
                   endpoint_limited=c["graph_key"] in limits["endpoint_limited_graphs"],
                   improvements=";".join(improvements), regressions=";".join(regressions),
                   improved_metrics=len(improvements), worst_regression_tolerances=max(normalized),
                   communities_median=median(m["communities"] for m in ms),
                   communities_min=min((m["communities"] for m in ms), default=None),
                   communities_max=max((m["communities"] for m in ms), default=None),
                   coverage_min=score["minimum_coverage"], largest_share_max=score["maximum_largest_share"],
                   largest_size_median=median(m["largest_community_share"] * m["tokens"] for m in ms),
                   fraction_3_to_30=median(m["fraction_in_sizes_3_to_30"] for m in ms),
                   fraction_over_100=median(m["fraction_in_sizes_over_100"] for m in ms),
                   build_seconds=graph["cost"]["elapsed_seconds"],
                   solver_seconds_mean=statistics.mean(r["solver_seconds"] for r in c["runs"]),
                   runtime_proxy_seconds=score["runtime_seconds"],
                   representative_seed=c["repeats"]["representative_seed"],
                   representative_assignments=c["representative_assignments"])
        row.update(score["metrics"])
        row["repeat_ari_min"] = c["repeats"]["ari"]["min"]
        row["repeat_ari_max"] = c["repeats"]["ari"]["max"]
        for metric, kind in [("edge_removal", "edge_removal"), ("subsample", "vocabulary_subsample")]:
            stress = [p for p in c["perturbations"] if p["kind"] == kind and p["agreement"]]
            row[metric + "_coverage_change"] = median(p["perturbed_nonsingleton_coverage"] - p["comparator_nonsingleton_coverage"] for p in stress)
        rows.append(row)
    shortlist = []
    for weight in ["cosine", "snn", "local"]:
        eligible = [r for r in rows if r["stage"] == "core" and r["weight"] == weight and r["guards_pass"] and r["pareto"]]
        eligible.sort(key=lambda r: (r["worst_regression_tolerances"], -r["improved_metrics"], r["runtime_proxy_seconds"], r["id"]))
        if eligible:
            shortlist.append(eligible[0])
    return rows, shortlist


def review_groups(study, candidates, shortlist, manifest):
    baseline = next(c for c in candidates if c["baseline"])
    selected_ids = [baseline["id"]] + [r["id"] for r in shortlist]
    partitions = [("archived recommendation", manifest["baseline"]["recommended_assignments"], None)]
    for identity in selected_ids:
        c = next(c for c in candidates if c["id"] == identity)
        seed = c["repeats"]["representative_seed"]
        partitions.append((name(c), c["representative_assignments"], Path(c["directory"]) / f"seed-{seed}" / "words.csv"))
    # The old archive has its own per-word metrics; exported assignments remain authoritative.
    archived_words = Path(manifest["baseline"]["recommended_assignments"]).with_name("recommended-word-details.csv")
    if archived_words.exists():
        partitions[0] = (*partitions[0][:2], archived_words)
    rows = []
    for label, assignments, words_path in partitions:
        groups, lookup, words = defaultdict(list), {}, {}
        with Path(assignments).open() as stream:
            for row in csv.DictReader(stream):
                groups[row["community"]].append(row["token"])
                lookup[row["token"]] = row["community"]
        if words_path:
            with words_path.open() as stream:
                for row in csv.DictReader(stream):
                    token = row.get("token", row.get("word"))
                    words[token] = row
        def metric(token, field, default):
            saved = words.get(token, {})
            aliases = {"cohesion": "mean_cosine_to_own_others", "margin": "cosine_margin"}
            value = saved.get(field, saved.get(aliases.get(field)))
            return float(value) if value not in (None, "") else default
        seen = {}
        for anchor in manifest["review_anchors"]:
            community = lookup[anchor]
            members = sorted(groups[community])
            centers = sorted(members, key=lambda t: (-metric(t, "cohesion", -2), t))[:8]
            boundary = sorted(members, key=lambda t: (metric(t, "margin", 2), t))[:6] if len(members) > 1 else members
            sample = sorted(members, key=lambda t: hashlib.sha256((anchor + "\0" + t).encode()).hexdigest())[:8]
            rows.append(dict(partition=label, anchor=anchor, community=community, size=len(members),
                             duplicate_of_anchor=seen.get(community, ""), centers=" ".join(centers),
                             fixed_sample=" ".join(sample), boundary=" ".join(boundary), members=" ".join(members)))
            seen.setdefault(community, anchor)
    write_csv(study / "fixed-anchor-review.csv", rows)
    lines = ["# Fixed-anchor groups", "", "The 18 anchors were fixed before screening. Centers and boundaries use saved per-word geometry; the full member lists are in [the CSV](fixed-anchor-review.csv). These are contextual groups, not synonym lists.", ""]
    for anchor in manifest["review_anchors"]:
        lines += [f"## {anchor}", "", "| Partition | Size | Centers | Fixed sample | Boundary |", "|---|---:|---|---|---|"]
        for r in rows:
            if r["anchor"] == anchor:
                lines.append(f'| {r["partition"]} | {r["size"]} | {r["centers"]} | {r["fixed_sample"]} | {r["boundary"]} |')
        lines.append("")
    (study / "FIXED_GROUPS.md").write_text("\n".join(lines))
    return rows


def fmt(value, digits=4):
    return "—" if value is None else f"{value:.{digits}f}"


def comparison_table(rows):
    lines = ["| Configuration | Groups | Cohesion | Silhouette | Margin | Repeat ARI | Edge ARI | Subsample ARI | Coverage | Largest | Graph setup s | Solver s |",
             "|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|"]
    for r in rows:
        cells = [r["name"], fmt(r["communities_median"], 0)]
        cells += [fmt(r[m]) for m in ["cohesion", "silhouette", "margin", "repeat_ari", "edge_removal_ari", "subsample_ari"]]
        cells += [fmt(100 * r["nonsingleton_coverage"], 2) + "%", fmt(r["largest_size_median"], 0), fmt(r["build_seconds"], 2), fmt(r["solver_seconds_mean"], 2)]
        lines.append("| " + " | ".join(cells) + " |")
    return lines


def make_report(study, candidates, rows, shortlist, manifest, cal, protocol, development, review):
    baseline = next(r for r in rows if r["baseline"])
    archived = read(study / "screening" / "archived-recommended" / "metrics.json")
    archive_ari = [r["agreement"]["ari"] for r in read(study / "calibration" / "archive-agreements.json")]
    status = read(study / "screening" / "experiment-status.json")
    attempts = read(study / "screening" / "attempts.json")
    failed = [a for a in attempts if a["status"] == "failed"]
    core = [r for r in rows if r["stage"] == "core"]
    acceptance = [r for r in core if r["guards_pass"] and not r["regressions"] and r["improved_metrics"] > 0]
    nominee_name = next((r["name"] for r in rows if r["id"] == development["nominee"]), "none")
    lines = ["# Calibrated graph v2 comparison — 2026-09-25", "",
             f"**{len(acceptance)} raw core configurations pass the frozen replacement comparison.** The development nominee is **{nominee_name}**. It still needs fresh-seed confirmation, so the archived recommendation remains the default.", "",
             f"Completed **{len(candidates)} settings**, including **{len(core)} raw core settings**, with three common screening seeds each, ten paired edge-removal and ten vocabulary-subsample replicates per setting. **{len(failed)} failed attempts** out of {len(attempts)} recorded attempts. Centering, ABTT, mutual-neighbor ablations and fresh-seed confirmation remain deferred.", "",
             "The shortlist below identifies development starting points; it is not evidence of independently validated semantic superiority.", "",
             "## Baseline calibration", "",
             f"Used {len(cal['plan']['solver_seeds'])} seeded baseline fits and {len(cal['plan']['perturbation_seeds'])} paired replicates of each perturbation kind, on the archived shifted-cosine threshold 0.70 graph, resolution/gamma 0.20, theta 0.30. Calibration seeds are separate from screening and reserved confirmation seeds. The seeded v2 solver is used for both graph families; the archived assignment is also evaluated directly to expose any backend/partition difference.", "",
             "| Measurement | Baseline median | Observed min–max | Frozen absolute tolerance |", "|---|---:|---:|---:|"]
    for spec in cal["policy"]["metrics"]:
        metric = spec["metric"]
        key = "repeat_ari_dependent_pairs" if metric == "repeat_ari" else metric
        d = cal["baseline_distributions"][key]
        lines.append(f"| {metric} | {fmt(d['median'],6)} | {fmt(d['min'],6)}–{fmt(d['max'],6)} | {fmt(spec['absolute_tolerance'],6)} |")
    policy = cal["policy"]
    lines += ["", f"Every screening repeat must retain at least **{policy['minimum_nonsingleton_coverage']:.4%} nonsingleton coverage**; its largest group must contain no more than **{policy['maximum_largest_community_share']:.4%}** of the vocabulary. At least two groups are required and all-singleton partitions are rejected.", "",
              cal["method"], "",
              "[Frozen manifest](calibration/screening-manifest.json), [calibration evidence](calibration/calibration.json), and [pre-screening analysis protocol](analysis-protocol.json) preserve the numeric rules and fixed review sample. The runner verifies the evidence, study, input, metadata, archived graph/assignment, and compiled source hashes before screening.", "",
              f"The original archived representative has {archived['communities']} groups, cohesion {archived['cohesion']['value']:.4f}, silhouette {archived['silhouette']['value']:.4f}, coverage {archived['nonsingleton_coverage']:.2%}, and largest group {round(archived['largest_community_share'] * archived['tokens'])}. Its metrics are a single partition, whereas the screening baseline below summarizes fresh common-seed repeats.", "",
              f"Agreement between the archived assignment and the 30 seeded baseline fits: median ARI **{statistics.median(archive_ari):.4f}**, range {min(archive_ari):.4f}–{max(archive_ari):.4f}. This bridge check separates the unchanged archived partition from the baseline distribution under the common seeded solver; it is not a semantic quality score.", "",
              "## Development shortlist", "", protocol["shortlist"]["rule"], ""]
    lines += comparison_table([baseline] + shortlist)
    lines += ["", "Partition scores are medians across three fits; solver time is the mean, and graph setup is measured once per graph. Largest is median largest-group size; the stricter max-over-repeats guard is in the comparison CSV. Cohesion/margin exclude singleton words; silhouette assigns them zero. ARI pair values are dependent diagnostics.", "",
              "Archived graph setup loads, filters and exports an existing graph; it excludes that graph's original all-pairs construction. V2 graph setup includes exact neighbor construction from embeddings. These setup costs therefore have different starting points. The solver timings compare the same seeded backend; cosine/local k=10 fit faster than the baseline here, while the SNN shortlist entry fits more slowly.", ""]
    for r in shortlist:
        lines += [f"- **{r['name']}** (`{r['id'][:12]}`): improved beyond tolerance: {r['improvements'].replace(';', ', ') or 'none'}; regressed beyond tolerance: {r['regressions'].replace(';', ', ') or 'none'}. Worst regression: {r['worst_regression_tolerances']:.2f} tolerance units. Graph build {r['build_seconds']:.2f}s, {r['edges']:,} edges; endpoint-limited search: {'yes' if r['endpoint_limited'] else 'no'}."]
    absent = [w for w in ["cosine", "snn", "local"] if not any(r["weight"] == w for r in shortlist)]
    if absent:
        lines += ["", "No Pareto core candidate passes the frozen hard limits in these weight families: " + ", ".join(absent) + "."]
    lines += ["", "### Perturbation coverage, null controls and local sensitivity", "",
              "| Configuration | Edge-removal coverage change | Subsample coverage change | Cohesion above null mean | k −20% ARI | k +20% ARI | r −20% ARI | r +20% ARI |",
              "|---|---:|---:|---:|---:|---:|---:|---:|"]
    for r in [baseline] + shortlist:
        c = next(c for c in candidates if c["id"] == r["id"])
        null = read(Path(c["directory"]) / "null-control.json")
        sensitivity = {(s["parameter"], s["direction"]): s["agreement"]["ari"] if s["agreement"] else None for s in c["sensitivity"]}
        cells = [r["name"], fmt(100 * r["edge_removal_coverage_change"], 3) + " pp", fmt(100 * r["subsample_coverage_change"], 3) + " pp", fmt(null["observed_minus_null_mean"]["value"])]
        cells += [fmt(sensitivity.get((p, d))) for p, d in [("k", "decrease"), ("k", "increase"), ("resolution", "decrease"), ("resolution", "increase")]]
        lines.append("| " + " | ".join(cells) + " |")
    lines += ["", "Coverage changes use paired comparisons; vocabulary-subsample coverage uses only retained words. Sensitivity changes k or resolution while holding other settings fixed and uses the same solver seed. The 100 size-preserving label permutations test association with source geometry, not independent semantic correctness.", ""]
    strata_rows = []
    for r in [baseline] + shortlist:
        c = next(c for c in candidates if c["id"] == r["id"])
        strata = read(Path(c["directory"]) / "strata.json")
        for s in strata["strata"]:
            strata_rows.append({"configuration": r["name"], **s})
    write_json(study / "shortlist-strata.json", strata_rows)
    lines += ["HSK and baseline-degree summaries are saved in [shortlist-strata.json](shortlist-strata.json). All 10,936 tokens have HSK metadata; 143 have ambiguous mappings. POS is unavailable for every token, so POS comparisons cannot be made. HSK levels are descriptive strata, never community labels.", ""]
    lines += ["", f"Core candidates satisfying the no-regression/at-least-one-improvement rule: **{len(acceptance)}** before endpoint restrictions. Automated development decision: {development['reason']}", "",
              "## Archived controls", "",
              "These controls separate neighbor-graph changes from retuning or reweighting the existing threshold graph. Stored thresholds 0.70, 0.725 and 0.75 correspond to unshifted cosine cutoffs 0.40, 0.45 and 0.50. The unshifted controls retain exactly the corresponding edges, change their weights and retune resolution. Expansions are retained in the CSV.", ""]
    controls = [r for r in rows if r["stage"] != "core" and (r["q"] is None or r["q"] in manifest["q"])]
    lines += comparison_table(controls)
    control_improvements = [r for r in rows if r["stage"] != "core" and not r["baseline"] and r["guards_pass"] and not r["regressions"] and r["improved_metrics"] > 0]
    lines += ["", "Archived controls passing the no-regression/at-least-one-improvement rule before endpoint restrictions: " + (", ".join(r["name"] for r in control_improvements) or "none") + "."]
    if development["nominee"]:
        nominee = next(r for r in rows if r["id"] == development["nominee"])
        lines += ["", f"Frozen development nominee: **{nominee['name']}**. It has not been confirmed or exported as a new default."]
    lines += ["",
              "## Full initial core grid", "", "The starting q values were 0.01, 0.03, 0.1 and 0.3; endpoint expansion is capped at two rounds with factor three. Every attempted expansion and archived threshold/resolution control is included in [comparison.csv](comparison.csv). Endpoint limits mean the resolution search is incomplete; they are not evidence of a best q.", ""]
    initial = [r for r in core if r["q"] in manifest["q"]]
    initial.sort(key=lambda r: (r["weight"], r["k"], r["q"]))
    lines += comparison_table(initial)
    lines += ["", "## Fixed learning-material inspection", "",
              f"Exported {len(review)} anchor/partition views from the same 18 anchors used in the archive. Several anchors may share one group; the CSV records these duplicates. See [all fixed groups](FIXED_GROUPS.md), [all member lists](fixed-anchor-review.csv), and [qualitative assessment](LEARNING_REVIEW.md). The sample deliberately includes broad themes, function words, singleton/polysemous words and unstable abstract topics. It is a convenience sample, not a random estimate of semantic accuracy.", "",
              "A [supplementary control view](control-review/FIXED_GROUPS.md) uses the same anchors for the archived controls passing the development rule. It was added after the control results were known, for interpretation; it does not alter the predeclared core shortlist or its 90-view review panel.", "",
              "## Costs, validity and limits", "",
              f"Screening wall time: {status['cost']['elapsed_seconds'] / 60:.1f} minutes. Process peak RSS: {status['cost']['process_peak_memory_bytes'] / 2**30:.2f} GiB (process lifetime high-water mark). Artifacts: {status['cost']['artifact_bytes'] / 2**30:.2f} GiB. Graph construction is cached within the study; runtime comparisons report construction plus mean solver time separately from metrics/export overhead.", "",
              "Every valid fit has complete vocabulary assignments and connected induced communities. The stored default is checked against the archived recommendation. Source-space cohesion, silhouette and margins measure the same embedding geometry used to construct the graphs; no external labels, independent embeddings or learner outcomes are available. Coverage, concentration and robustness therefore remain separate constraints. Size bands are diagnostics, not accuracy scores.", "",
              "The study also exports size-preserving null permutations, HSK/baseline-degree strata, ±20% k/resolution sensitivity, and similar-edge/community-count comparisons. Perturbation ARI measures sensitivity to graph/token removal; it does not quantify uncertainty in the original embedding training data. The many development comparisons make fresh-seed confirmation necessary before accepting a default.", "",
              "## Follow-up boundaries", "",
              "Use the shortlisted raw configurations as starting points for one-factor comparisons of centering, ABTT(1), ABTT(3) and mutual neighbors, retuning q for each graph. Keep the archived baseline and any passing unshifted-weight control as references. Resolve flagged resolution endpoints before nominating a default. Freeze the final variants before consuming the reserved solver seeds 1000–1009 and perturbation seeds 20000–20009; confirmation must accept or reject that frozen choice without retuning. This report does not launch those later stages.", "",
              "See [shortlist.json](shortlist.json), [validation.json](validation.json), [screening outputs](screening/REPORT.md), and the reproduction commands in [README.md](README.md).", ""]
    (study / "COMPARISON.md").write_text("\n".join(lines))


def validate_artifacts(study, candidates, manifest):
    screen = study / "screening"
    with Path(manifest["input"]).open() as stream:
        count, _ = map(int, next(stream).split())
        tokens = sorted(line.split()[0] for line in stream if line.strip())
    assert len(tokens) == count == len(set(tokens))
    token_set = set(tokens)
    perturbation_tokens = {}
    for path in (screen / "perturbation-graphs").glob("*/provenance.json"):
        p = read(path)
        if p["kind"] == "vocabulary_subsample":
            perturbation_tokens[(p["source_graph_key"], p["seed"])] = {tokens[i] for i in p["indices"]}
    checked, observed_seeds, checksums = set(), set(), []
    metrics_checked = 0
    for c in candidates:
        paths = [(Path(r["assignments"]), token_set) for r in c["runs"] if r["assignments"]]
        observed_seeds.update(r["seed"] for r in c["runs"])
        for p in c["perturbations"]:
            observed_seeds.add(p["seed"])
            root = Path(c["directory"]) / "stress"
            paths.append((root / f"comparator-{p['seed']}" / "communities.csv", token_set))
            expected = perturbation_tokens[(c["graph_key"], p["seed"])] if p["kind"] == "vocabulary_subsample" else token_set
            paths.append((root / f"{p['kind']}-{p['seed']}" / "communities.csv", expected))
        paths += [(p, token_set) for p in (Path(c["directory"]) / "sensitivity").glob("*/communities.csv")]
        for path, expected in paths:
            if path in checked:
                continue
            with path.open() as stream:
                data = list(csv.DictReader(stream))
            actual_tokens = [r["token"] for r in data]
            assert len(actual_tokens) == len(set(actual_tokens)), f"Duplicate tokens: {path}"
            assert set(actual_tokens) == expected, f"Wrong vocabulary: {path}"
            sizes = Counter(r["community"] for r in data)
            metrics = read(path.with_name("metrics.json"))
            assert metrics["tokens"] == len(data) and metrics["communities"] == len(sizes), path
            assert metrics["all_communities_connected"], path
            assert abs(metrics["largest_community_share"] - max(sizes.values()) / len(data)) < 1e-12, path
            coverage = 1 - sum(s == 1 for s in sizes.values()) / len(data)
            assert abs(metrics["nonsingleton_coverage"] - coverage) < 1e-12, path
            metrics_checked += 1
            if metrics_checked % 500 == 0:
                print(f"Audited {metrics_checked} fit assignment files", flush=True)
            checked.add(path)
            checksums.append({"path": str(path), "sha256": hashlib.sha256(path.read_bytes()).hexdigest(), "tokens": len(data)})
        for r in c["runs"]:
            assert r["error"] is None, c["id"]
        assert len(c["runs"]) == len(manifest["screening_seeds"])
        assert len(c["perturbations"]) == 2 * len(manifest["development_perturbation_seeds"])
        assert all(p["error"] is None for p in c["perturbations"])
        representative = next(r for r in c["runs"] if r["seed"] == c["repeats"]["representative_seed"])
        assert Path(c["representative_assignments"]).read_bytes() == Path(representative["assignments"]).read_bytes()
    reserved = set(manifest["confirmation_seeds"] + manifest["confirmation_perturbation_seeds"])
    assert not observed_seeds & reserved
    assert read(screen / "confirmation.json") == []
    assert (screen / "communities.csv").read_bytes() == Path(manifest["baseline"]["recommended_assignments"]).read_bytes()
    initial_grid = {(g["k"], g["weight"], q) for g in manifest["graphs"] for q in manifest["q"]}
    completed_grid = {(c["source"]["config"]["k"], c["source"]["config"]["weight"], c["q"]) for c in candidates if c["stage"] == "core"}
    assert initial_grid <= completed_grid
    failures = [a for a in read(screen / "attempts.json") if a["status"] == "failed"]
    assert not failures, failures[:3]
    evidence = manifest["calibration"]
    assert hashlib.sha256(Path(evidence["report"]).read_bytes()).hexdigest() == evidence["report_sha256"]
    seal = read(study / "protocol-seal.json")
    for filename, digest in seal["sha256"].items():
        assert hashlib.sha256((study / filename).read_bytes()).hexdigest() == digest, filename
    result = dict(status="passed", vocabulary_tokens=count, candidate_settings=len(candidates),
                  initial_core_settings=len(initial_grid), verified_fit_assignment_files=len(checked),
                  size_and_coverage_metrics_recomputed=metrics_checked, failed_attempts=len(failures),
                  complete_subsample_vocabulary=True, all_recorded_communities_connected=True,
                  representatives_match_selected_seed=True, reserved_confirmation_seeds_unused=True,
                  archived_default_byte_identical=True,
                  protocol_and_calibration_hashes_verified=True,
                  report_generator_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                  note="Connectivity is checked by the Rust solver and metrics on each induced community; this audit checks saved flags and independently recomputes assignment coverage and sizes.")
    write_json(study / "validation.json", result)
    write_csv(study / "assignment-checksums.csv", checksums)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("study", type=Path)
    parser.add_argument("--seal", action="store_true", help="Hash the analysis protocol and calibrated plan before screening; requires a new seal and no screening directory")
    args = parser.parse_args()
    study = args.study
    if args.seal:
        assert not (study / "protocol-seal.json").exists(), "Protocol seal already exists"
        assert not (study / "screening").exists(), "Seal the protocol before screening"
        files = ["analysis-protocol.json", "calibration/plan.json", "calibration/screening-manifest.json"]
        write_json(study / "protocol-seal.json", {
            "created_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
            "sha256": {f: hashlib.sha256((study / f).read_bytes()).hexdigest() for f in files},
            "base_commit": subprocess.check_output(["git", "rev-parse", "HEAD"], text=True).strip(),
            "rustc": subprocess.check_output(["rustc", "--version"], text=True).strip(),
            "platform": subprocess.check_output(["uname", "-sm"], text=True).strip(),
        })
        return
    screen = study / "screening"
    assert read(screen / "experiment-status.json")["status"] == "complete", "Screening is incomplete"
    manifest = read(screen / "manifest.json")
    cal = read(study / "calibration" / "calibration.json")
    protocol = read(study / "analysis-protocol.json")
    assert protocol["practical_review"]["anchors"] == manifest["review_anchors"]
    assert manifest["selection"] == cal["policy"]
    assert not any(manifest[k] for k in ["ablations", "combinations", "confirmation"])
    candidates = read(screen / "candidates.json")
    validate_artifacts(study, candidates, manifest)
    development = read(screen / "development-selection.json")
    rows, shortlist = summarize(candidates, manifest, development, read(screen / "search-limits.json"))
    write_csv(study / "comparison.csv", rows)
    controls = [r for r in rows if r["stage"] != "core" and not r["baseline"] and r["guards_pass"] and not r["regressions"] and r["improved_metrics"] > 0]
    write_json(study / "shortlist.json", {"protocol": protocol["shortlist"], "candidates": shortlist, "control_references_passing_development_rule": controls, "default": "archived-recommended", "confirmation_performed": False})
    review = review_groups(study, candidates, shortlist, manifest)
    if controls:
        control_directory = study / "control-review"
        control_directory.mkdir(exist_ok=True)
        review_groups(control_directory, candidates, controls, manifest)
    make_report(study, candidates, rows, shortlist, manifest, cal, protocol, development, review)
    print(json.dumps({"settings": len(rows), "shortlist": [r["name"] for r in shortlist]}, ensure_ascii=False))


if __name__ == "__main__":
    main()
