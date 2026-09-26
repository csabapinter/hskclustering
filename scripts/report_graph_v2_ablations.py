#!/usr/bin/env python3
"""Freeze and report the focused k=10 graph ablations; no fitting or retuning.

python3 scripts/report_graph_v2_ablations.py STUDY --freeze
python3 scripts/report_graph_v2_ablations.py STUDY
Uses the standard library and existing core-study reporting helpers.
"""
import argparse
import csv
import hashlib
import json
import math
from collections import Counter
from pathlib import Path

import report_graph_v2 as core


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def factor(candidate):
    if candidate["source"]["kind"] == "archived":
        return "archived"
    g = candidate["source"]["config"]
    r = g["representation"]
    if isinstance(r, dict):
        return f'abtt{r["abtt"]}'
    if r == "center":
        return "center"
    if g["symmetrization"] == "mutual":
        return "mutual"
    return g["weight"]


def differences(row, reference, policy):
    regressions, improvements, losses = [], [], []
    for spec in policy["metrics"]:
        m, tolerance = spec["metric"], spec["absolute_tolerance"]
        if row[m] is None or reference[m] is None:
            regressions.append(m + ":missing")
            losses.append(float("inf"))
            continue
        delta = row[m] - reference[m]
        losses.append(max(0, -delta / tolerance))
        if delta < -tolerance:
            regressions.append(m)
        if delta > tolerance:
            improvements.append(m)
    return regressions, improvements, max(losses)


def supported(row, reference, policy):
    regressions, improvements, _ = differences(row, reference, policy)
    return row["guards_pass"] and not regressions and bool(improvements)


def graph_diagnostics(study, candidates, manifest):
    graphs = {}
    for c in candidates:
        if c["source"]["kind"] == "neighbors":
            graphs.setdefault(c["graph_key"], c)
    raw = next(c for c in graphs.values() if factor(c) == "cosine")
    raw_graph = core.read(Path(raw["graph_directory"]) / "graph.json")
    raw_edges = {(e["u"], e["v"]) for e in raw_graph["edges"]}
    anchors = set(manifest["review_anchors"])
    rows, neighbor_rows = [], []
    for c in graphs.values():
        directory = Path(c["graph_directory"])
        record = core.read(directory / "graph-record.json")
        m = record["metrics"]
        graph = core.read(directory / "graph.json")
        assert graph["tokens"] == raw_graph["tokens"]
        edges = {(e["u"], e["v"]) for e in graph["edges"]}
        rows.append(dict(factor=factor(c), nodes=m["nodes"], edges=m["edges"],
            isolates=m["isolates"], components=m["components"],
            largest_component_fraction=m["largest_component_fraction"],
            degree_median=m["degree"]["median"], degree_max=m["degree"]["max"],
            median_weight=m["weight"]["median"],
            directed_neighbor_reciprocity=m["neighbor_reciprocity"]["value"],
            incoming_neighbor_skewness=m["incoming_neighbor_skewness"]["value"],
            incoming_neighbor_max=m["incoming_neighbor_max"],
            edge_jaccard_with_raw=len(edges & raw_edges) / len(edges | raw_edges),
            raw_edges_retained_fraction=len(edges & raw_edges) / len(raw_edges)))
        with (directory / "neighbors.csv").open() as stream:
            for row in csv.DictReader(stream):
                if row["token"] in anchors:
                    neighbor_rows.append(dict(factor=factor(c), **row))
    core.write_csv(study / "graph-comparison.csv", rows)
    core.write_csv(study / "neighbor-review.csv", neighbor_rows)


def development(study):
    screen = study / "screening"
    assert core.read(screen / "experiment-status.json")["status"] == "complete"
    seal = core.read(study / "protocol-seal.json")
    for file, expected in seal["sha256"].items():
        assert sha(study / file) == expected, f"Changed sealed protocol: {file}"
    manifest = core.read(screen / "manifest.json")
    candidates = core.read(screen / "candidates.json")
    selection = core.read(screen / "development-selection.json")
    limits = core.read(screen / "search-limits.json")
    rows, _ = core.summarize(candidates, manifest, selection, limits)
    by_id = {c["id"]: c for c in candidates}
    for row in rows:
        row["factor"] = factor(by_id[row["id"]])
    reference = next(r for r in rows if r["factor"] == "cosine" and r["q"] == 0.1)
    policy = manifest["selection"]
    for row in rows:
        regressions, improvements, loss = differences(row, reference, policy)
        row.update(raw_regressions=";".join(regressions), raw_improvements=";".join(improvements),
                   raw_worst_regression_tolerances=loss)
    chosen, matched = [], []
    for group in core.read(study / "analysis-protocol.json")["shortlist"]["groups"]:
        variants = [r for r in rows if r["factor"] == group]
        eligible = [r for r in variants if r["guards_pass"] and r["pareto"]]
        eligible.sort(key=lambda r: (r["raw_worst_regression_tolerances"],
                                    -len([m for m in r["raw_improvements"].split(";") if m]),
                                    abs(math.log(r["q"] / 0.1)), r["id"]))
        if eligible:
            chosen.append(eligible[0])
        valid = [r for r in variants if r["valid"]]
        def count_difference(r):
            a, b = r["communities_median"], reference["communities_median"]
            return abs(a - b) / max(a, b, 1)
        if valid:
            closest = min(valid, key=lambda r: (count_difference(r), r["q"], r["id"]))
            matched.append({**closest, "count_relative_difference": count_difference(closest),
                            "count_match_within_tolerance": count_difference(closest) <= manifest["similar_community_count_relative_tolerance"]})
    controls = [r for r in rows if r["baseline"] or
                (r["factor"] in ("cosine", "local") and r["q"] == 0.1) or
                (by_id[r["id"]]["stage"] == "controls-unshifted" and r["q"] == 0.3)]
    selected = {r["id"]: r for r in controls + chosen}
    if selection["nominee"]:
        r = next(r for r in rows if r["id"] == selection["nominee"])
        selected[r["id"]] = r
    return manifest, candidates, rows, reference, chosen, matched, list(selected.values())


def audit_confirmation(study, candidates, manifest, plan):
    root = study / "confirmation"
    assert core.read(root / "experiment-status.json")["status"] == "complete"
    assert core.read(root / "plan.json") == plan
    assert {c["id"] for c in candidates} == set(plan["candidates"])
    tokens = sorted(line.split()[0] for line in Path(manifest["input"]).read_text().splitlines()[1:])
    token_set = set(tokens)
    subsets = {}
    for path in (root / "perturbation-graphs").glob("*/provenance.json"):
        p = core.read(path)
        if p["kind"] == "vocabulary_subsample":
            subsets[p["source_graph_key"], p["seed"]] = {tokens[i] for i in p["indices"]}
    checksums, checked = [], set()
    for c in candidates:
        original = core.read(plan["screening"] + "/candidates/" + c["id"] + "/candidate.json")
        for field in ["source", "q", "resolution", "theta", "graph_key"]:
            assert c[field] == original[field], (c["id"], field)
        assert c["score"]["valid"]
        assert [r["seed"] for r in c["runs"]] == manifest["confirmation_seeds"]
        assert len(c["perturbations"]) == 2 * len(manifest["confirmation_perturbation_seeds"])
        assert all(p["error"] is None for p in c["perturbations"])
        representative = next(r for r in c["runs"] if r["seed"] == c["repeats"]["representative_seed"])
        assert sha(c["representative_assignments"]) == sha(representative["assignments"])
        paths = [(Path(r["assignments"]), token_set) for r in c["runs"]]
        for p in c["perturbations"]:
            assert p["seed"] in manifest["confirmation_perturbation_seeds"]
            stress = Path(c["directory"]) / "stress"
            paths.append((stress / f"comparator-{p['seed']}" / "communities.csv", token_set))
            expected = subsets[c["graph_key"], p["seed"]] if p["kind"] == "vocabulary_subsample" else token_set
            paths.append((stress / f"{p['kind']}-{p['seed']}" / "communities.csv", expected))
        paths += [(p, token_set) for p in (Path(c["directory"]) / "sensitivity").glob("*/communities.csv")]
        for path, expected in paths:
            if path in checked:
                continue
            with path.open() as stream:
                data = list(csv.DictReader(stream))
            actual = [r["token"] for r in data]
            assert len(actual) == len(set(actual)) and set(actual) == expected, path
            sizes = Counter(r["community"] for r in data)
            m = core.read(path.with_name("metrics.json"))
            assert m["tokens"] == len(data) and m["communities"] == len(sizes), path
            assert m["all_communities_connected"], path
            assert abs(m["largest_community_share"] - max(sizes.values()) / len(data)) < 1e-12, path
            assert abs(m["nonsingleton_coverage"] - (1 - sum(s == 1 for s in sizes.values()) / len(data))) < 1e-12, path
            checked.add(path)
            checksums.append(dict(path=str(path), sha256=sha(path), tokens=len(data)))
    assert not [a for a in core.read(root / "attempts.json") if a["status"] == "failed"]
    assert not (root / "communities.csv").exists()
    core.write_csv(study / "confirmation-assignment-checksums.csv", checksums)
    core.write_json(study / "confirmation-validation.json", dict(
        status="passed", settings=len(candidates), verified_fit_assignments=len(checked),
        frozen_settings_unchanged=True, reserved_seed_cohorts_verified=True,
        full_and_subsample_vocabularies_verified=True, sizes_and_coverage_recomputed=True,
        representatives_match_seed=True, no_default_exported=True))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("study", type=Path)
    parser.add_argument("--freeze", action="store_true")
    args = parser.parse_args()
    study = args.study
    manifest, candidates, rows, reference, chosen, matched, selected = development(study)
    screen = study / "screening"
    plan_path = study / "confirmation-plan.json"
    plan = dict(screening=str(screen), manifest_sha256=sha(screen / "manifest.json"),
                provenance_sha256=sha(screen / "provenance.json"),
                candidates={r["id"]: sha(Path(next(c for c in candidates if c["id"] == r["id"])["directory"]) / "candidate.json") for r in selected})
    if args.freeze:
        assert not plan_path.exists() and not (study / "confirmation").exists(), "Confirmation was already frozen or started"
        core.validate_artifacts(study, candidates, manifest)
        core.write_json(plan_path, plan)
        core.write_json(study / "shortlist.json", dict(reference=reference, ablations=chosen,
            confirmation_settings=selected, protocol_sha256=sha(study / "analysis-protocol.json"),
            reporting_source_sha256={str(p): sha(p) for p in [Path(__file__), Path(core.__file__)]}))
        core.write_csv(study / "development-comparison.csv", rows)
        core.write_csv(study / "count-matched.csv", matched)
        graph_diagnostics(study, candidates, manifest)
        review = study / "development-review"
        review.mkdir()
        core.review_groups(review, candidates, [r for r in selected if not r["baseline"]], manifest)
        print(json.dumps({"frozen_settings": [r["name"] for r in selected]}, ensure_ascii=False))
        return
    assert core.read(plan_path) == plan, "Frozen shortlist changed; never retune on confirmation"
    for path, expected in core.read(study / "shortlist.json")["reporting_source_sha256"].items():
        assert sha(path) == expected, "Reporting implementation changed after shortlist freeze"
    confirmed = core.read(study / "confirmation" / "confirmation.json")
    audit_confirmation(study, confirmed, manifest, plan)
    dev_selection = core.read(screen / "development-selection.json")
    limits = core.read(screen / "search-limits.json")
    confirmed_rows, _ = core.summarize(confirmed, manifest, dev_selection, limits)
    confirmed_by_id = {r["id"]: r for r in confirmed_rows}
    raw_confirmation = confirmed_by_id[reference["id"]]
    decisions = []
    for group in core.read(study / "analysis-protocol.json")["shortlist"]["groups"]:
        dev = next((r for r in chosen if r["factor"] == group), None)
        if dev is None:
            variants = [r for r in rows if r["factor"] == group]
            decisions.append(dict(factor=group, decision="discard at screening", reason="No candidate passes both the frozen hard guards and Pareto rule",
                                  guard_failures=sorted({r["guard_failures"] for r in variants})))
            continue
        con = confirmed_by_id[dev["id"]]
        regressions, improvements, _ = differences(con, raw_confirmation, manifest["selection"])
        keep = supported(dev, reference, manifest["selection"]) and supported(con, raw_confirmation, manifest["selection"])
        decisions.append(dict(factor=group, id=dev["id"], name=dev["name"],
            decision="keep experimental improvement" if keep else "do not adopt; retain as a measured trade-off",
            development_pass=supported(dev, reference, manifest["selection"]), confirmation_pass=supported(con, raw_confirmation, manifest["selection"]),
            confirmation_regressions=regressions, confirmation_improvements=improvements,
            confirmation_guard_failures=con["guard_failures"], endpoint_limited=dev["endpoint_limited"]))
    baseline = next(r for r in confirmed_rows if r["baseline"])
    nominee = confirmed_by_id.get(dev_selection["nominee"])
    default_accepted = bool(nominee and not nominee["endpoint_limited"] and supported(nominee, baseline, manifest["selection"]))
    core.write_json(study / "decisions.json", dict(ablations=decisions,
        frozen_default_nominee=dev_selection["nominee"], default_nominee_confirmation_accepted=default_accepted,
        archived_files_unchanged=True, no_new_default_exported=True,
        note="Experimental decisions compare with raw cosine; default acceptance compares only the frozen nominee with the archived baseline. No substitution or retuning."))
    core.write_csv(study / "confirmation-comparison.csv", confirmed_rows)
    core.review_groups(study, confirmed, [r for r in confirmed_rows if not r["baseline"]], manifest)
    lines = ["# Focused k=10 graph ablations — 2026-09-26", "",
             "## Decision", ""]
    for d in decisions:
        lines += [f"- **{d['factor']}**: {d['decision']}. " +
                  ("Confirmation regressions versus raw cosine: " + (", ".join(d["confirmation_regressions"]) or "none") + "." if "id" in d else d["reason"] + ".")]
    lines += ["", f"Frozen default nominee confirmed under the unchanged archived-baseline rule: **{default_accepted}**. Archived files are unchanged; this report does not silently install a different default.", "",
              "## Fresh confirmation", "", "Ten reserved solver seeds and ten paired perturbations of each kind per frozen setting. Values below are cohort medians; Largest is the median largest-community size, not a hard cap.", ""]
    lines += core.comparison_table(confirmed_rows)
    lines += ["", "## Development shortlist", "", "The raw cosine reference and local comparator were fixed at q=0.1; each ablation's q was selected by the sealed rule before confirmation.", ""]
    lines += core.comparison_table(selected)
    lines += ["", "## Comparisons at similar granularity", "", "Closest median community counts to raw cosine q=0.1; matches outside 10% are explicitly marked in count-matched.csv. This diagnoses whether apparent gains simply reflect finer partitioning.", ""]
    lines += core.comparison_table([reference] + matched)
    lines += ["", "## Graph structure", "",
              "Edge overlap is measured against raw cosine k=10. Directed-neighbor statistics describe the lists before union/mutual filtering; mutual pruning therefore leaves their values unchanged. Weight scales differ by representation/kernel and are not quality scores.", "",
              "| Graph | Edges | Isolates | Components | Max degree | Incoming-neighbor max | Edge Jaccard with raw |",
              "|---|---:|---:|---:|---:|---:|---:|"]
    with (study / "graph-comparison.csv").open() as stream:
        for g in csv.DictReader(stream):
            lines.append(f"| {g['factor']} | {g['edges']} | {g['isolates']} | {g['components']} | {float(g['degree_max']):.0f} | {g['incoming_neighbor_max']} | {float(g['edge_jaccard_with_raw']):.4f} |")
    lines += ["", "The original 18 anchors' full top-10 directed lists are in neighbor-review.csv. Cosines/distances there use each graph's fitted representation, so their magnitudes are not directly comparable across representations. Local weighting and mutual pruning reuse raw neighbor rankings; their effect is in edge weights or retention."]
    lines += ["", "## Scope and interpretation", "",
              "Six graphs: raw cosine/local union, centered cosine, ABTT(1) cosine, ABTT(3) cosine, and raw mutual cosine; all k=10. Only one factor changes at a time. The archived shifted-weight baseline and unshifted threshold-0.70 control provide continuity with the earlier study. No combinations, new embedding model or solver algorithm change were tested.", "",
              "Each graph starts with q=0.01, 0.03, 0.1, 0.3 and has up to two endpoint expansions. All attempted q values, endpoint flags, source-space coherence, coverage, size bands, repeats and perturbations are retained in development-comparison.csv. Full graph diagnostics include isolates, components, reciprocity, incoming-neighbor skewness and degree/weight distributions.", "",
              "The existing baseline calibration is rerun on this build. Its tolerances remain descriptive sensitivity scales. Applying them to raw-cosine comparisons is an explicit conservative common yardstick, not a kNN-specific uncertainty estimate. All seven metrics and the existing coverage/concentration guards must pass in both development and confirmation to support an experimental improvement. Mixed trade-offs do not displace the raw reference.", "",
              "The same 18-anchor desk review is exported in FIXED_GROUPS.md and fixed-anchor-review.csv. It supplies qualitative context, not a human benchmark or an estimated accuracy rate. Source-space metrics reuse the graph's input embeddings. Neither stable partitions nor higher silhouette independently establish semantic usefulness; single-vector polysemy persists.", "",
              "See analysis-protocol.json and protocol-seal.json for pre-fit rules; confirmation-plan.json and shortlist.json for the frozen subset; decisions.json for the machine-readable outcome; validation.json and confirmation-validation.json for artifact audits; LEARNING_REVIEW.md for the example-group inspection.", ""]
    (study / "COMPARISON.md").write_text("\n".join(lines))
    print(json.dumps({"decisions": decisions, "default_accepted": default_accepted}, ensure_ascii=False))


if __name__ == "__main__":
    main()
