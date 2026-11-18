"""Standard-library checks for traceable IDs in the updated Leiden report."""

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path


SCRIPT = Path(__file__).resolve().parents[1] / "scripts" / "report_updated_leiden.py"
SPEC = importlib.util.spec_from_file_location("report_updated_leiden", SCRIPT)
report = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(report)


class ReportTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="leiden-report-")
        self.addCleanup(self.temporary.cleanup)
        self.study = Path(self.temporary.name)
        self.screening = self.study / "screening"
        self.confirmation = self.study / "confirmation"
        self.screening.mkdir()
        self.confirmation.mkdir()
        self.candidate_directory = self.confirmation / "candidate"
        self.candidate_directory.mkdir()
        (self.candidate_directory / "communities.csv").write_text(
            "token,community\n爱,0\n喜欢,0\n猫,1\n狗,1\n兔子,1\n", encoding="utf-8")
        profile = report.size_profile([2, 3])
        metrics = {"tokens": 5, "communities": 2,
                   "sizes": {key: profile[key] for key in ("mean", "q25", "median", "q75", "q95", "max")},
                   "singleton_token_fraction": 0, "pair_token_fraction": 0.4,
                   "fraction_in_sizes_3_to_30": 0.6, "fraction_in_sizes_over_100": 0,
                   "largest_community_share": 0.6, "nonsingleton_coverage": 1,
                   "cohesion": {"value": 0.5}, "silhouette": {"value": 0.1},
                   "margin": {"value": 0.05}, "all_communities_connected": True}
        other_metrics = dict(metrics, communities=3, singleton_token_fraction=0.2,
                             sizes=dict(metrics["sizes"], max=2))
        self.candidate = {"name": "test-q0.3", "spec": {"source": {"kind": "threshold", "config": 0.4}},
                          "q": 0.3, "resolution": 0.1, "theta": 0.1, "graph_key": "test",
                          "runs": [{"seed": seed, "metrics": data, "solver_seconds": 0.01}
                                   for seed, data in ((42, metrics), (43, other_metrics))],
                          "representative_seed": 42, "representative_metrics": metrics,
                          "repeat_ari": 0.8, "profile_distance": 0,
                          "directory": str(self.candidate_directory),
                          "stress": [{"kind": kind, "seed": 1, "ari": 0.75}
                                     for kind in ("edge_removal", "vocabulary_subsample")]}
        (self.screening / "protocol.json").write_text(json.dumps({"reference_profile": profile}))
        for stage in (self.screening, self.confirmation):
            (stage / "summary.json").write_text(json.dumps([self.candidate]))
        self.ids = ("ID,token,community,status\n"
                    "1,爱,0,assigned\n2,喜欢,0,assigned\n3,猫,1,assigned\n"
                    "4,狗,1,assigned\n5,兔子,1,assigned\n6,爱,0,assigned\n"
                    "7,缺词,,missing_embedding\n")
        self.id_path = self.study / "recommended.communities.csv"
        self.id_path.write_text(self.ids, encoding="utf-8")
        self.by_token = {"爱": "0", "喜欢": "0", "猫": "1", "狗": "1", "兔子": "1"}

    def test_reports_assigned_duplicate_and_missing_id(self):
        report.main([str(self.study), "--selected", self.candidate["name"]])
        audit = json.loads((self.study / "report-audit.json").read_text())
        self.assertEqual(audit["selected_profile"]["tokens"], 5)
        self.assertEqual(audit["id_expanded_profile"]["tokens"], 6)
        self.assertEqual(audit["id_expanded_profile"]["mean"], 3)
        self.assertEqual(audit["id_expanded_profile"]["communities"], 2)
        self.assertEqual(audit["id_coverage"], {"source_ids": 7, "assigned_ids": 6,
                                             "unassigned_ids": 1, "unassigned_unique_labels": 1})
        text = (self.study / "README.md").read_text()
        self.assertIn("remaining 1 IDs (1 unique labels)", text)
        self.assertIn("Variation across confirmation seeds", text)
        self.assertIn("Representative assignment's profile distance", text)

    def test_unknown_token_requires_explicit_missing_status_and_blank_community(self):
        for replacement in ("7,缺词,,assigned", "7,缺词,1,missing_embedding", "7,缺词,,"):
            with self.subTest(replacement=replacement):
                self.id_path.write_text(self.ids.replace("7,缺词,,missing_embedding", replacement))
                with self.assertRaisesRegex(ValueError, "Unknown token"):
                    report.id_profile(self.study, self.candidate, self.by_token)

    def test_known_token_cannot_be_missing_or_have_wrong_membership(self):
        for replacement in ("6,爱,1,assigned", "6,爱,,missing_embedding"):
            with self.subTest(replacement=replacement):
                self.id_path.write_text(self.ids.replace("6,爱,0,assigned", replacement))
                with self.assertRaisesRegex(ValueError, "does not match"):
                    report.id_profile(self.study, self.candidate, self.by_token)

    def test_seed_ranges_keep_representative_separate_from_median(self):
        row = report.summarize(self.candidate, "confirmation")
        self.assertEqual((row["communities_min"], row["communities_median"], row["communities_max"]), (2, 2.5, 3))
        self.assertEqual((row["sizes_max_min"], row["sizes_max_median"], row["sizes_max_max"]), (2, 2.5, 3))
        self.assertEqual(row["representative_sizes_max"], 3)
        self.assertEqual(row["singleton_token_fraction_median"], 0.1)
        self.assertEqual(row["singleton_token_fraction_min"], 0)
        self.assertEqual(row["singleton_token_fraction_max"], 0.2)


if __name__ == "__main__":
    unittest.main()
