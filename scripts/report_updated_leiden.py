#!/usr/bin/env python3
"""Report the updated-vocabulary Rust Leiden study without fitting or selecting.

Usage: python3 scripts/report_updated_leiden.py STUDY [--selected CANDIDATE]

Only Python's standard library is required. Selection is an explicit argument;
the first row in the distribution-distance ranking is not an automatic winner.
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import statistics
from collections import Counter, defaultdict
from pathlib import Path


BANDS = ((1, 1), (2, 2), (3, 5), (6, 10), (11, 20), (21, 30),
         (31, 50), (51, 100), (101, math.inf))
BAND_NAMES = ("1", "2", "3–5", "6–10", "11–20", "21–30", "31–50", "51–100", ">100")


def read_json(path):
    with path.open(encoding="utf-8") as stream:
        return json.load(stream)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def number(value):
    if isinstance(value, dict):
        value = value.get("value")
    if value is None:
        return None
    value = float(value)
    if not math.isfinite(value):
        raise ValueError(f"Nonfinite report value: {value}")
    return value


def median(values):
    values = [number(value) for value in values]
    values = [value for value in values if value is not None]
    return statistics.median(values) if values else None


def metric(metrics, key):
    value = metrics
    for part in key.split("."):
        if not isinstance(value, dict):
            return None
        value = value.get(part)
    return number(value)


def quantile(values, fraction):
    values = sorted(values)
    position = (len(values) - 1) * fraction
    lower = math.floor(position)
    upper = math.ceil(position)
    return values[lower] + (values[upper] - values[lower]) * (position - lower)


def size_profile(sizes):
    sizes = list(sizes)
    if not sizes:
        raise ValueError("An empty assignment has no size profile")
    tokens = sum(sizes)
    return {
        "tokens": tokens,
        "communities": len(sizes),
        "mean": tokens / len(sizes),
        "median": quantile(sizes, 0.5),
        "q25": quantile(sizes, 0.25),
        "q75": quantile(sizes, 0.75),
        "q95": quantile(sizes, 0.95),
        "max": max(sizes),
        "group_fractions": [sum(lo <= size <= hi for size in sizes) / len(sizes)
                            for lo, hi in BANDS],
        "token_fractions": [sum(size for size in sizes if lo <= size <= hi) / tokens
                            for lo, hi in BANDS],
    }


def resolve(path, study):
    path = Path(path)
    if path.is_absolute() or path.exists():
        return path
    return study / path


def load_stage(study, names):
    for name in names:
        path = study / name / "summary.json"
        if path.exists():
            data = read_json(path)
            if isinstance(data, dict):
                data = data.get("candidates", data.get("summary"))
            if not isinstance(data, list):
                raise ValueError(f"Expected a candidate array in {path}")
            names_seen = [candidate["name"] for candidate in data]
            if len(names_seen) != len(set(names_seen)):
                raise ValueError(f"Duplicate candidate names in {path}")
            return path, data
    return None, []


def summarize(candidate, stage):
    metrics = candidate["representative_metrics"]
    runs = candidate.get("runs", [])
    stress = candidate.get("stress", [])
    row = {
        "stage": stage,
        "candidate": candidate["name"],
        "profile_distance": number(candidate["profile_distance"]),
        "profile_distance_basis": "representative_assignment",
        "q": number(candidate["q"]),
        "resolution": number(candidate["resolution"]),
        "theta": number(candidate["theta"]),
        "graph_key": candidate.get("graph_key", ""),
        "representative_seed": candidate["representative_seed"],
        "solver_runs": len(runs),
        "tokens": metric(metrics, "tokens"),
        "representative_communities": metric(metrics, "communities"),
        "communities_median": median([r["metrics"]["communities"] for r in runs]),
        "communities_min": min((r["metrics"]["communities"] for r in runs), default=None),
        "communities_max": max((r["metrics"]["communities"] for r in runs), default=None),
        "repeat_ari": number(candidate.get("repeat_ari")),
        "solver_seconds_median": median([r.get("solver_seconds") for r in runs]),
        "directory": str(candidate["directory"]),
        "guardrails_pass": bool(runs) and all(
            run["metrics"]["nonsingleton_coverage"] >= 0.95
            and run["metrics"]["largest_community_share"] <= 0.04
            and run["metrics"]["communities"] > 1
            and run["metrics"].get("all_communities_connected", False)
            for run in runs),
    }
    for key in ("sizes.mean", "sizes.q25", "sizes.median", "sizes.q75", "sizes.q95", "sizes.max",
                "singleton_token_fraction", "pair_token_fraction", "fraction_in_sizes_3_to_30",
                "fraction_in_sizes_over_100", "largest_community_share", "nonsingleton_coverage",
                "cohesion", "silhouette", "margin"):
        label = key.replace(".", "_")
        row[f"representative_{label}"] = metric(metrics, key)
        values = [metric(r["metrics"], key) for r in runs]
        values = [value for value in values if value is not None]
        row[f"{label}_median"] = median(values)
        row[f"{label}_min"] = min(values, default=None)
        row[f"{label}_max"] = max(values, default=None)
    row["all_communities_connected"] = metrics.get("all_communities_connected")
    for kind in ("edge_removal", "vocabulary_subsample"):
        # Accept either the core experiment names or the shorter study names.
        aliases = {kind}
        if kind == "vocabulary_subsample":
            aliases.update(("subsample", "vocabulary_subsampling"))
        elif kind == "edge_removal":
            aliases.update(("edge", "edge_drop"))
        values = [number(item.get("ari")) for item in stress if item.get("kind") in aliases]
        row[f"{kind}_ari"] = median(values)
        row[f"{kind}_replicates"] = len(values)
    row["confirmation_guardrails_pass"] = (
        row["guardrails_pass"] and row["repeat_ari"] is not None and row["repeat_ari"] >= 0.65
        and all(row[f"{kind}_ari"] is not None and row[f"{kind}_ari"] >= 0.65
                for kind in ("edge_removal", "vocabulary_subsample")))
    return row


def write_csv(path, rows):
    if not rows:
        return
    fields = list(dict.fromkeys(key for row in rows for key in row))
    with path.open("w", encoding="utf-8", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=fields)
        writer.writeheader()
        writer.writerows(rows)


def assignments(path):
    with path.open(encoding="utf-8-sig", newline="") as stream:
        rows = list(csv.DictReader(stream))
    if not rows or not {"token", "community"}.issubset(rows[0]):
        raise ValueError(f"Expected nonempty token,community assignments in {path}")
    tokens = [row["token"] for row in rows]
    if any(not token for token in tokens) or len(tokens) != len(set(tokens)):
        raise ValueError(f"Blank or repeated tokens in unique-label assignments: {path}")
    if any(not row["community"] for row in rows):
        raise ValueError(f"Blank community in {path}")
    return rows


def assignment_path(candidate, study):
    if candidate.get("representative_assignments"):
        return resolve(candidate["representative_assignments"], study)
    return resolve(candidate["directory"], study) / "communities.csv"


def selected_exports(study, candidate):
    path = assignment_path(candidate, study)
    rows = assignments(path)
    by_token = {row["token"]: row["community"] for row in rows}
    groups = defaultdict(list)
    for row in rows:
        groups[row["community"]].append(row["token"])
    profile = size_profile(map(len, groups.values()))
    expected = candidate["representative_metrics"]
    if profile["tokens"] != expected["tokens"] or profile["communities"] != expected["communities"]:
        raise ValueError(f"Assignment count differs from representative metrics: {path}")
    ordered = sorted(groups.items(), key=lambda item: (-len(item[1]), item[0]))
    write_csv(study / "community-sizes.csv", [
        {"community": community, "unique_labels": len(words)} for community, words in ordered])
    write_csv(study / "cluster-review.csv", [
        {"community": community, "unique_labels": len(words), "words": " | ".join(sorted(words))}
        for community, words in ordered])
    return path, by_token, profile


def id_profile(study, candidate, by_token):
    locations = []
    for key in ("id_assignments", "assignments_with_ids"):
        if candidate.get(key):
            locations.append(resolve(candidate[key], study))
    locations.extend((study / "recommended.communities.csv", study / "recommended-communities.csv",
                      study / "communities-with-ids.csv",
                      resolve(candidate["directory"], study) / "communities-with-ids.csv"))
    for path in dict.fromkeys(locations):
        if not path.exists():
            continue
        with path.open(encoding="utf-8-sig", newline="") as stream:
            reader = csv.DictReader(stream)
            fields = set(reader.fieldnames or [])
            id_key = next((key for key in ("ID", "id", "identifier") if key in fields), None)
            token_key = next((key for key in ("token", "Tilelabels", "word") if key in fields), None)
            if not id_key or not token_key or "community" not in fields:
                continue
            rows = list(reader)
        ids = [row[id_key] for row in rows]
        if not rows or any(not identifier for identifier in ids) or len(ids) != len(set(ids)):
            raise ValueError(f"ID assignments contain missing or repeated IDs: {path}")
        assigned = []
        unassigned = []
        for row in rows:
            token = row[token_key]
            if not token:
                raise ValueError(f"ID assignments contain a blank token: {path}")
            if token not in by_token:
                if row["community"] == "" and row.get("status") == "missing_embedding":
                    unassigned.append(row)
                    continue
                raise ValueError(f"Unknown token must have blank community and missing_embedding status: {path}")
            if by_token[token] != row["community"] or row.get("status") == "missing_embedding":
                raise ValueError(f"ID assignment does not match selected token membership: {path}")
            assigned.append(row)
        if {row[token_key] for row in assigned} != set(by_token):
            raise ValueError(f"ID assignments do not cover the selected token set: {path}")
        coverage = {"source_ids": len(rows), "assigned_ids": len(assigned),
                    "unassigned_ids": len(unassigned),
                    "unassigned_unique_labels": len({row[token_key] for row in unassigned})}
        return path, size_profile(Counter(row["community"] for row in assigned).values()), coverage
    return None, None, None


def fmt(value, digits=3):
    if value is None:
        return "—"
    return f"{value:.{digits}f}"


def percent(value):
    return "—" if value is None else f"{100 * value:.2f}%"


def table(rows):
    lines = ["| Candidate | Profile distance² ↓ | Groups¹ | Median² | p95² | Max² | Singletons¹ | 3–30¹ | >100¹ | Cohesion¹ | Silhouette¹ | Repeat ARI | Edge ARI | Subsample ARI |",
             "| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |"]
    for row in rows:
        values = [row["candidate"], fmt(row["profile_distance"], 4), fmt(row["communities_median"], 1),
                  fmt(row["representative_sizes_median"], 1), fmt(row["representative_sizes_q95"], 1),
                  fmt(row["representative_sizes_max"], 0), percent(row["singleton_token_fraction_median"]),
                  percent(row["fraction_in_sizes_3_to_30_median"]), percent(row["fraction_in_sizes_over_100_median"]),
                  fmt(row["cohesion_median"]), fmt(row["silhouette_median"]), fmt(row["repeat_ari"]),
                  fmt(row["edge_removal_ari"]), fmt(row["vocabulary_subsample_ari"])]
        lines.append("| " + " | ".join(str(value).replace("|", "\\|") for value in values) + " |")
    return "\n".join(lines)


def range_table(rows):
    lines = ["| Candidate | Groups: min / median / max | Singleton token fraction: min / median / max | Largest group: min / median / max |",
             "| --- | ---: | ---: | ---: |"]
    for row in rows:
        values = [row["candidate"]]
        for key, formatter in (("communities", lambda value: fmt(value, 1)),
                               ("singleton_token_fraction", percent),
                               ("sizes_max", lambda value: fmt(value, 1))):
            values.append(" / ".join(formatter(row[f"{key}_{stat}"]) for stat in ("min", "median", "max")))
        lines.append("| " + " | ".join(values) + " |")
    return "\n".join(lines)


def profile_table(reference, selected=None, expanded=None):
    labels = [("tokens", "Items"), ("communities", "Communities"), ("mean", "Mean community size"),
              ("q25", "p25"), ("median", "Median"), ("q75", "p75"), ("q95", "p95"), ("max", "Largest")]
    columns = [("Reference unique labels", reference)]
    if selected:
        columns.append(("Selected unique labels", selected))
    if expanded:
        columns.append(("Selected assigned source IDs", expanded))
    lines = ["| Measure | " + " | ".join(name for name, _ in columns) + " |",
             "| --- | " + " | ".join("---:" for _ in columns) + " |"]
    for key, label in labels:
        lines.append("| " + label + " | " + " | ".join(fmt(profile[key], 2) for _, profile in columns) + " |")
    lines.extend(("", "| Size band | " + " | ".join(f"{name}: groups / items" for name, _ in columns) + " |",
                  "| --- | " + " | ".join("---:" for _ in columns) + " |"))
    for index, band in enumerate(BAND_NAMES):
        lines.append("| " + band + " | " + " | ".join(
            f"{percent(profile['group_fractions'][index])} / {percent(profile['token_fractions'][index])}"
            for _, profile in columns) + " |")
    return "\n".join(lines)


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("study", type=Path)
    parser.add_argument("--selected", help="Explicit candidate name to report as the recommendation")
    args = parser.parse_args(argv)
    study = args.study
    screen_path, screen = load_stage(study, ("screening", "screen"))
    confirm_path, confirm = load_stage(study, ("confirmation", "confirm"))
    if not screen:
        raise ValueError("Screening summary is absent or empty")
    protocol_path = study / "protocol.json"
    if not protocol_path.exists():
        protocol_path = screen_path.parent / "protocol.json"
    protocol = read_json(protocol_path)
    reference = protocol["reference_profile"]
    shortlist_path = screen_path.parent / "shortlist.json"
    finalists = read_json(shortlist_path).get("candidates", []) if shortlist_path.exists() else []
    screen_rows = sorted((summarize(candidate, "screening") for candidate in screen),
                         key=lambda row: (row["profile_distance"], row["candidate"]))
    confirm_rows = sorted((summarize(candidate, "confirmation") for candidate in confirm),
                          key=lambda row: (row["profile_distance"], row["candidate"]))
    write_csv(study / "screening-summary.csv", screen_rows)
    if confirm_rows:
        write_csv(study / "confirmation-summary.csv", confirm_rows)
    selected = None
    selected_path = None
    selected_profile = None
    id_path = None
    expanded_profile = None
    id_coverage = None
    if args.selected:
        selected = next((candidate for candidate in confirm if candidate["name"] == args.selected), None)
        if selected is None:
            selected = next((candidate for candidate in screen if candidate["name"] == args.selected), None)
        if selected is None:
            raise ValueError(f"Selected candidate does not exist: {args.selected}")
        selected_path, by_token, selected_profile = selected_exports(study, selected)
        id_path, expanded_profile, id_coverage = id_profile(study, selected, by_token)

    lines = ["# Updated-vocabulary Leiden study", ""]
    if selected:
        stage = "fresh confirmation" if any(c["name"] == args.selected for c in confirm) else "screening only"
        lines.extend((f"Selected recommendation: **{args.selected}**, reviewed using {stage}.", "",
                      f"Unique-label assignments: `{selected_path}`.", ""))
        if id_path:
            lines.extend((f"Source-ID assignments: `{id_path}`. All {id_coverage['source_ids']:,} source IDs are unique; {id_coverage['assigned_ids']:,} have memberships checked against the selected unique-label assignment. The remaining {id_coverage['unassigned_ids']:,} IDs ({id_coverage['unassigned_unique_labels']:,} unique labels) have no embedding and are preserved with a blank community and `missing_embedding` status.", ""))
        else:
            lines.extend(("No ID-bearing assignment file was available when this report ran. The unique-label result alone does not establish source-ID coverage.", ""))
    else:
        lines.extend(("No recommendation has been selected. The tables rank measured settings by size-profile distance for inspection.", ""))
    related = [("DECISION.md", "Recommendation rationale"),
               ("LEARNING_REVIEW.md", "18-anchor qualitative review"),
               ("assignment-audit.json", "Assignment integrity audit"),
               ("engineering-checks.json", "Implementation checks")]
    links = [f"[{label}]({filename})" for filename, label in related if (study / filename).exists()]
    if links:
        lines.extend((" · ".join(links), ""))
    lines.extend(("## Objective and scope", "",
                  "The target is a similar distribution of community sizes to the supplied reference, while allowing memberships to change. Distribution distance is a descriptive fit to that preference; it is not semantic accuracy. The Rust protocol records the graph grid, resolution search, seeds and stress checks. This report performs no fitting, parameter tuning or automatic selection.", "",
                  "Cluster repeated surface labels once and map the resulting membership to each source ID. This preserves IDs without giving repeated spellings extra graph weight. Label-only embeddings cannot distinguish the senses represented by separate IDs.", "",
                  f"Protocol: `{protocol_path}` (SHA-256 `{digest(protocol_path)}`).", ""))
    if protocol.get("selection_rule"):
        lines.extend(("Frozen selection rule: " + protocol["selection_rule"], ""))
    reference_path = protocol.get("reference", protocol.get("reference_path"))
    if reference_path:
        lines.extend((f"Reference assignments: `{reference_path}`.", ""))
    lines.extend(("## Community-size distribution", "", profile_table(reference, selected_profile, expanded_profile), "",
                  "Size bands show the fraction of groups and the fraction of items in those groups. Keeping both views exposes differences that a matched community count or median can hide. Source-ID sizes include every assigned repeated-label row. Missing-embedding rows remain unassigned and are excluded from community-size statistics; unique-label sizes describe the graph actually clustered.", "",
                  "## Screening", "",
                  f"{len(screen_rows)} measured settings. The first {min(15, len(screen_rows))} by profile distance are shown; `screening-summary.csv` contains every setting and the source directories.", "",
                  table(screen_rows[:15]), ""))
    if finalists:
        lines.extend(("### Frozen screening finalists", "",
                      "These candidates were retained before fresh confirmation; the complete screening table may include settings outside the guardrails.", "",
                      table([row for row in screen_rows if row["candidate"] in finalists]), ""))
    if confirm_rows:
        lines.extend(("## Fresh confirmation", "", table(confirm_rows), "",
                      "### Variation across confirmation seeds", "", range_table(confirm_rows), ""))
    lines.extend(("¹ Median across solver repeats; fractions are fractions of tokens. ² Representative assignment's profile distance and sizes, rather than a median across seeds. Edge/subsample ARI are medians of paired perturbations. Missing measurements are shown as —. Candidate CSVs retain representative values and per-seed minima, medians and maxima separately, including group count, singleton fraction and largest-group size.", "",
                  "The representative seed is chosen by repeat agreement in the Rust study. A repeated fit measures solver sensitivity; 5% edge removal measures graph sensitivity, and 90% vocabulary retention rebuilds the graph for the retained vocabulary. Similar size profiles can still have different members.", "",
                  "## Inspecting the recommendation", ""))
    if selected:
        lines.extend(("`community-sizes.csv` lists every selected group, largest first. `cluster-review.csv` lists every member of every group for qualitative inspection. Memberships and IDs should be joined by their explicit columns, since numerical community labels have no semantic meaning across runs.", ""))
    else:
        lines.extend(("Run this report with `--selected CANDIDATE` after reviewing the candidates to export the selected group sizes and complete member lists.", ""))
    lines.extend(("Coherence, silhouette and margins use the same source embeddings as the graph. No independent embedding-space semantic quality, external labels, or learner evaluation was measured. Stability and a close size distribution support practical inspection; neither establishes semantic correctness. The new vocabulary may legitimately change topical group sizes.", ""))
    (study / "README.md").write_text("\n".join(lines), encoding="utf-8")
    audited = [path for path in (protocol_path, screen_path, confirm_path, selected_path, id_path) if path]
    if shortlist_path.exists():
        audited.append(shortlist_path)
    audit = {"selected": args.selected, "report_script_sha256": digest(Path(__file__)),
             "inputs_sha256": {str(path): digest(path) for path in audited},
             "selected_profile": selected_profile, "id_expanded_profile": expanded_profile,
             "id_coverage": id_coverage}
    (study / "report-audit.json").write_text(json.dumps(audit, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"Wrote report for {len(screen_rows)} screening and {len(confirm_rows)} confirmation settings to {study}")


if __name__ == "__main__":
    main()
