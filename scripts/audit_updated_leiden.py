#!/usr/bin/env python3
"""Independently audit Leiden assignment artifacts using only the standard library.

Usage: python3 scripts/audit_updated_leiden.py STUDY
       python3 scripts/audit_updated_leiden.py --self-test
"""

from __future__ import annotations

import argparse
import csv
import hashlib
import json
import math
import os
import tempfile
from collections import Counter
from datetime import datetime, timezone
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def require(condition, message):
    if not condition:
        raise ValueError(message)


def read_assignments(path, tokens):
    with path.open(encoding="utf-8-sig", newline="") as stream:
        reader = csv.DictReader(stream)
        require({"token", "community"} <= set(reader.fieldnames or []), "Missing assignment columns")
        rows = {}
        for row in reader:
            token, label = row["token"], row["community"]
            require(token and token not in rows, f"Blank or repeated token {token!r}")
            require(label and label.isascii() and label.isdecimal(), f"Invalid community for {token!r}")
            rows[token] = int(label)
    require(set(rows) == set(tokens),
            f"Vocabulary differs: {len(set(tokens) - set(rows))} missing, {len(set(rows) - set(tokens))} unknown")
    return rows


def check_metrics(rows, metrics):
    sizes = list(Counter(rows.values()).values())
    n, k = len(rows), len(sizes)
    singletons, pairs = sizes.count(1), sizes.count(2)
    expected = {
        "tokens": n, "communities": k,
        "singleton_token_fraction": singletons / n,
        "singleton_community_fraction": singletons / k,
        "pair_token_fraction": 2 * pairs / n,
        "pair_community_fraction": pairs / k,
        "largest_community_share": max(sizes) / n,
        "nonsingleton_coverage": 1 - singletons / n,
        "fraction_in_sizes_3_to_30": sum(s for s in sizes if 3 <= s <= 30) / n,
        "fraction_in_sizes_over_100": sum(s for s in sizes if s > 100) / n,
    }
    for key, value in expected.items():
        require(key in metrics and math.isclose(metrics[key], value, rel_tol=1e-12, abs_tol=1e-12),
                f"Stored {key} differs from assignments")
    for key, value in {"count": k, "min": min(sizes), "max": max(sizes), "mean": n / k}.items():
        require(math.isclose(metrics["sizes"][key], value, rel_tol=1e-12, abs_tol=1e-12),
                f"Stored sizes.{key} differs from assignments")


class Audit:
    def __init__(self, study):
        self.study = study.resolve()
        self.hashes = {}
        self.counts = Counter()
        self.errors = []
        self.notes = []
        self.graphs = {}

    def resolve(self, value):
        path = Path(value)
        if path.is_absolute():
            return path
        return next((p for p in (ROOT / path, self.study / path, path) if p.exists()), ROOT / path)

    def digest(self, path):
        path = path.resolve()
        key = str(path)
        if key not in self.hashes:
            digest = hashlib.sha256()
            with path.open("rb") as stream:
                for block in iter(lambda: stream.read(1024 * 1024), b""):
                    digest.update(block)
            self.hashes[key] = digest.hexdigest()
        return self.hashes[key]

    def json(self, path):
        self.digest(path)
        with path.open(encoding="utf-8") as stream:
            return json.load(stream)

    def attempt(self, label, function):
        try:
            return function()
        except (OSError, ValueError, KeyError, TypeError, ZeroDivisionError, csv.Error) as error:
            self.errors.append(f"{label}: {error}")
            return None

    def assignments(self, path, tokens, metrics=None):
        self.digest(path)
        rows = read_assignments(path, tokens)
        if metrics is not None:
            check_metrics(rows, metrics)
        self.counts["assignment_files"] += 1
        self.counts["assignment_rows"] += len(rows)
        return rows

    def fit(self, directory, tokens, metrics=None):
        stored = self.json(directory / "metrics.json")
        rows = self.assignments(directory / "communities.csv", tokens, stored)
        if metrics is not None:
            check_metrics(rows, metrics)
        return rows

    def candidate(self, candidate, stage, tokens, protocol):
        name = candidate["name"]
        directory = self.resolve(candidate["directory"])
        self.graph(candidate, tokens)
        expected_seeds = protocol[f"{stage}_seeds"]
        seeds = [run["seed"] for run in candidate["runs"]]
        require(seeds == expected_seeds, "Run seeds differ from protocol")
        for run in candidate["runs"]:
            self.attempt(f"{stage}/{name}/seed-{run['seed']}",
                         lambda run=run: self.fit(directory / f"seed-{run['seed']}", tokens, run["metrics"]))
        representative = self.assignments(directory / "communities.csv", tokens,
                                          candidate["representative_metrics"])
        seed = candidate["representative_seed"]
        require(seed in seeds, "Representative seed is not a fitted seed")
        require((directory / "communities.csv").read_bytes() ==
                (directory / f"seed-{seed}" / "communities.csv").read_bytes(),
                "Representative differs bytewise from its saved seed assignment")
        self.counts[f"{stage}_candidates"] += 1
        if stage == "confirmation":
            expected = {(kind, seed) for kind in ("edge_removal", "vocabulary_subsample")
                        for seed in protocol["stress_seeds"]}
            recorded = [(item["kind"], item["seed"]) for item in candidate["stress"]]
            require(len(recorded) == len(expected) and set(recorded) == expected, "Incomplete or duplicate stress records")
            for seed in protocol["stress_seeds"]:
                self.attempt(f"stress/{name}/seed-{seed}",
                             lambda seed=seed: self.stress(candidate, seed, tokens))
        return representative

    def graph(self, candidate, tokens):
        key = candidate["graph_key"]
        if key in self.graphs:
            require(self.graphs[key] == candidate["spec"]["source"], "Candidates disagree on graph source")
            return
        directory = self.study / "screening" / "graphs" / key
        source = self.json(directory / "source.json")
        require(source == candidate["spec"]["source"], "Graph source differs from candidate")
        graph = self.json(directory / "graph.json")
        metrics = self.json(directory / "metrics.json")
        require(graph["tokens"] == tokens, "Graph vocabulary differs from embeddings")
        require(metrics["nodes"] == len(tokens) and metrics["edges"] == len(graph["edges"]),
                "Graph counts differ from saved graph metrics")
        previous = (-1, -1)
        for edge in graph["edges"]:
            pair = (edge["u"], edge["v"])
            require(previous < pair and 0 <= pair[0] < pair[1] < len(tokens)
                    and math.isfinite(edge["weight"]) and edge["weight"] > 0,
                    "Invalid, duplicate or unordered graph edge")
            previous = pair
        self.graphs[key] = source
        self.counts["graphs"] += 1
        self.counts["graph_edges"] += len(graph["edges"])

    def stress(self, candidate, seed, tokens):
        directory = self.study / "confirmation" / "stress" / candidate["name"] / f"seed-{seed}"
        sampling = self.json(directory / "sampling.json")
        indices = sampling["retained_indices"]
        require(sampling["seed"] == seed, "Sampling seed differs")
        require(indices and all(type(i) is int and 0 <= i < len(tokens) for i in indices)
                and indices == sorted(set(indices)), "Invalid retained-token indices")
        for kind in ("comparator", "edge_removal"):
            self.fit(directory / kind, tokens)
        self.fit(directory / "vocabulary_subsample", [tokens[i] for i in indices])
        for item in candidate["stress"]:
            if item["seed"] == seed:
                expected_n = len(indices) if item["kind"] == "vocabulary_subsample" else len(tokens)
                require(item["retained_tokens"] == expected_n, "Stress retained-token count differs")
        self.counts["stress_pairs"] += 1

    def stage(self, stage, tokens, protocol):
        directory = self.study / stage
        require(self.json(directory / "status.json")["status"] == "complete", "Stage is incomplete")
        summary = self.json(directory / "summary.json")
        names = [candidate["name"] for candidate in summary]
        require(names and len(names) == len(set(names)), "Empty or duplicate candidate names")
        for candidate in summary:
            self.attempt(f"{stage}/{candidate['name']}",
                         lambda candidate=candidate: self.candidate(candidate, stage, tokens, protocol))
        return summary

    def ids(self, memberships, tokens):
        dataset = self.study / "dataset.csv"
        exported = self.study / "recommended.communities.csv"
        self.digest(dataset)
        self.digest(exported)
        with dataset.open(encoding="utf-8-sig", newline="") as stream:
            source = list(csv.DictReader(stream))
        with exported.open(encoding="utf-8-sig", newline="") as stream:
            output = list(csv.DictReader(stream))
        require(len(source) == len(output) and source, "Exported ID row count differs")
        ids = [row["ID"] for row in source]
        require(all(ids) and len(ids) == len(set(ids)), "Source IDs are blank or repeated")
        require([(row["ID"], row["Tilelabels"]) for row in source] ==
                [(row["ID"], row["token"]) for row in output], "Exported ID/token values or original row order differ")
        require(set(memberships) == set(tokens), "Recommended vocabulary differs from input")
        seen = {}
        assigned = missing = 0
        for row in output:
            token = row["token"]
            if token in memberships:
                require(row["community"] == str(memberships[token]) and row["status"] == "assigned",
                        f"Exported membership/status differs for ID {row['ID']}")
                assigned += 1
            else:
                require(row["community"] == "" and row["status"] == "missing_embedding",
                        f"Missing-vector ID {row['ID']} needs blank community and missing_embedding status")
                missing += 1
            value = (row["community"], row["status"])
            require(token not in seen or seen[token] == value, f"Duplicate spelling {token!r} has inconsistent assignments")
            seen[token] = value
        require(set(tokens) <= set(seen), "Export omits a clustered spelling")
        self.counts.update(source_ids=len(ids), assigned_ids=assigned, missing_ids=missing,
                           repeated_spelling_rows=len(ids) - len(seen))
        preparation = self.study / "preparation.json"
        if preparation.exists():
            report = self.json(preparation)
            require(self.digest(dataset) == report["dataset_sha256"], "Archived dataset checksum differs from preparation")
            require((len(ids), assigned, missing) == (report["rows"], report["matched_rows"], report["missing_rows"]),
                    "ID coverage differs from preparation")

    def run(self):
        screening = self.study / "screening"
        protocol = self.json(screening / "protocol.json")
        self.digest(Path(__file__))
        seed_lists = [protocol[key] for key in ("screening_seeds", "confirmation_seeds", "stress_seeds")]
        require([len(seeds) for seeds in seed_lists] == [3, 10, 10], "Protocol seed cohort sizes differ from study design")
        seeds = [seed for cohort in seed_lists for seed in cohort]
        require(len(seeds) == len(set(seeds)), "Protocol reuses seeds across cohorts")
        for name, expected in protocol["code_hashes"].items():
            path = ROOT / name if name in ("Cargo.toml", "Cargo.lock") else ROOT / "src" / name
            require(self.digest(path) == expected, f"Compiled source reference changed: {name}")
        embedding_path = self.resolve(protocol["input"])
        require(self.digest(embedding_path) == protocol["input_sha256"], "Embedding checksum differs")
        with embedding_path.open(encoding="utf-8") as stream:
            header = stream.readline().split()
            require(len(header) == 2 and int(header[0]) > 0 and int(header[1]) > 0, "Invalid embedding header")
            tokens = [line.split(" ", 1)[0] for line in stream]
        require(len(tokens) == int(header[0]) and len(tokens) == len(set(tokens))
                and tokens == sorted(tokens) and all(tokens), "Embedding token count/order/uniqueness differs")
        self.counts["input_tokens"] = len(tokens)
        self.counts["embedding_dimension"] = int(header[1])
        reference = self.resolve(protocol["reference"])
        require(self.digest(reference) == protocol["reference_sha256"], "Reference checksum differs")
        screened = self.attempt("screening", lambda: self.stage("screening", tokens, protocol))
        shortlist = self.json(screening / "shortlist.json")
        require(shortlist["protocol_sha256"] == self.digest(screening / "protocol.json")
                and shortlist["summary_sha256"] == self.digest(screening / "summary.json"), "Shortlist evidence hashes differ")
        require(shortlist["rule"] == protocol["selection_rule"], "Shortlist rule differs from protocol")
        if screened is not None:
            require(set(shortlist["candidates"]) <= {c["name"] for c in screened}, "Shortlist contains an unknown candidate")
        confirmed = self.attempt("confirmation", lambda: self.stage("confirmation", tokens, protocol))
        require(self.json(self.study / "confirmation" / "protocol.json") == protocol, "Confirmation protocol differs")
        require(self.json(self.study / "confirmation" / "shortlist.json") == shortlist, "Confirmation shortlist differs")
        if confirmed is not None:
            require([c["name"] for c in confirmed] == shortlist["candidates"], "Confirmed candidates differ from shortlist")
        recommendation = self.json(self.study / "confirmation" / "recommendation.json")
        if recommendation["name"] is None:
            self.notes.append("No qualified recommendation; ID export was not audited")
            return
        require(confirmed is not None, "Cannot validate recommendation without confirmation summary")
        chosen = next((c for c in confirmed if c["name"] == recommendation["name"]), None)
        require(chosen is not None, "Recommended candidate was not confirmed")
        final = self.study / "confirmation" / "communities.csv"
        require(self.digest(final) == recommendation["assignment_sha256"], "Recommendation assignment checksum differs")
        require(recommendation["shortlist_sha256"] == self.digest(screening / "shortlist.json"), "Recommendation shortlist checksum differs")
        require(recommendation["representative_seed"] == chosen["representative_seed"], "Recommendation seed differs")
        require(recommendation["rule"] == protocol["selection_rule"], "Recommendation rule differs from protocol")
        require(final.read_bytes() == (self.resolve(chosen["directory"]) / "communities.csv").read_bytes(),
                "Final recommendation differs bytewise from candidate representative")
        memberships = self.assignments(final, tokens, chosen["representative_metrics"])
        self.attempt("ID export", lambda: self.ids(memberships, tokens))

    def save(self):
        result = {"schema_version": 1, "status": "failed" if self.errors else "passed",
                  "audited_at": datetime.now(timezone.utc).isoformat(), "study": str(self.study),
                  "audit_script_sha256": self.digest(Path(__file__)),
                  "counts": dict(self.counts), "sha256": self.hashes, "notes": self.notes, "errors": self.errors}
        destination = self.study / "assignment-audit.json"
        with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", dir=self.study, delete=False) as stream:
            json.dump(result, stream, ensure_ascii=False, indent=2)
            stream.write("\n")
            temporary = stream.name
        os.replace(temporary, destination)
        return result


def self_test():
    import unittest

    class AuditTests(unittest.TestCase):
        def test_assignment_and_metric_corruption(self):
            with tempfile.TemporaryDirectory() as temporary:
                path = Path(temporary) / "communities.csv"
                path.write_text("token,community\na,7\nb,7\nc,2\n", encoding="utf-8")
                rows = read_assignments(path, ["a", "b", "c"])
                metrics = {"tokens": 3, "communities": 2, "singleton_token_fraction": 1 / 3,
                           "singleton_community_fraction": 1 / 2, "pair_token_fraction": 2 / 3,
                           "pair_community_fraction": 1 / 2, "largest_community_share": 2 / 3,
                           "nonsingleton_coverage": 2 / 3, "fraction_in_sizes_3_to_30": 0,
                           "fraction_in_sizes_over_100": 0, "sizes": {"count": 2, "min": 1, "max": 2, "mean": 1.5}}
                check_metrics(rows, metrics)
                metrics["sizes"]["max"] = 3
                with self.assertRaises(ValueError):
                    check_metrics(rows, metrics)
                for text in ("token,community\na,0\na,1\nc,2\n", "token,community\na,-1\nb,0\nc,2\n"):
                    path.write_text(text, encoding="utf-8")
                    with self.assertRaises(ValueError):
                        read_assignments(path, ["a", "b", "c"])

        def test_id_preservation_missing_rows_and_reordering(self):
            with tempfile.TemporaryDirectory() as temporary:
                study = Path(temporary)
                (study / "dataset.csv").write_text("ID,Tilelabels\n001,a\n002,b\n003,a\n004,c\n", encoding="utf-8")
                exported = study / "recommended.communities.csv"
                text = "ID,token,community,status\n001,a,7,assigned\n002,b,9,assigned\n003,a,7,assigned\n004,c,,missing_embedding\n"
                exported.write_text(text, encoding="utf-8")
                audit = Audit(study)
                audit.ids({"a": 7, "b": 9}, ["a", "b"])
                self.assertEqual(audit.counts["missing_ids"], 1)
                exported.write_text(text.replace("003,a,7", "003,a,8"), encoding="utf-8")
                with self.assertRaises(ValueError):
                    Audit(study).ids({"a": 7, "b": 9}, ["a", "b"])
                lines = text.splitlines()
                exported.write_text("\n".join([lines[0], lines[2], lines[1], *lines[3:]]) + "\n", encoding="utf-8")
                with self.assertRaises(ValueError):
                    Audit(study).ids({"a": 7, "b": 9}, ["a", "b"])

    result = unittest.TextTestRunner().run(unittest.defaultTestLoader.loadTestsFromTestCase(AuditTests))
    return 0 if result.wasSuccessful() else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("study", nargs="?", type=Path)
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()
    if args.self_test:
        return self_test()
    if args.study is None:
        parser.error("STUDY is required")
    audit = Audit(args.study)
    audit.attempt("study", audit.run)
    result = audit.save()
    print(f"Assignment audit {result['status']}: {len(audit.errors)} errors; {audit.counts['assignment_files']} assignment files")
    for error in audit.errors:
        print(error)
    return 1 if audit.errors else 0


if __name__ == "__main__":
    raise SystemExit(main())
