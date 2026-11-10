"""scripts/upgrade_paths.py's decisions, on fixtures: no gh call, no process, no network."""
import os
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "scripts"))
import upgrade_paths as U  # noqa: E402

RELS = [
    {"tagName": "v2026.11.9", "isDraft": False, "publishedAt": "2026-11-09T08:38:37Z"},
    {"tagName": "desktop-v2026.11.9", "isDraft": False, "publishedAt": "2026-11-09T08:44:50Z"},
    {"tagName": "desktop-v2026.11.8", "isDraft": False, "publishedAt": "2026-11-08T15:25:31Z"},
    {"tagName": "desktop-beta", "isDraft": False, "publishedAt": "2026-11-09T08:45:00Z"},
    {"tagName": "desktop-v2026.11.7", "isDraft": False, "publishedAt": "2026-11-07T18:28:07Z"},
    {"tagName": "desktop-v2026.11.6", "isDraft": False, "publishedAt": "2026-11-05T19:38:44Z"},
    {"tagName": "desktop-v2026.11.5", "isDraft": False, "publishedAt": "2026-11-05T09:48:52Z"},
    {"tagName": "desktop-v2026.11.10", "isDraft": True, "publishedAt": None},
]

GOOD = {"up": True, "rc": 0, "phase": "passed", "root": "/v/cand", "cand": "/v/cand", "hubs": 1, "answers": True}


class Paths(unittest.TestCase):
    def test_the_last_app_releases_newest_first_never_a_draft_a_terminal_or_a_channel(self):
        self.assertEqual(U.last_app_releases(RELS, 4), ["desktop-v2026.11.9", "desktop-v2026.11.8", "desktop-v2026.11.7", "desktop-v2026.11.6"])
        self.assertEqual(U.to_prune(["desktop-v2026.11.5", "desktop-v2026.11.9"], ["desktop-v2026.11.9"]), ["desktop-v2026.11.5"])

    def test_green_only_when_the_candidate_runs_alone_and_answers(self):
        self.assertEqual(U.judge(GOOD)[0], "green")
        # Gauthier's three outcomes, each red with its own words
        self.assertIn("runs /v/old", U.judge(dict(GOOD, root="/v/old"))[1], "a pass on the old version")
        self.assertIn("the hub stopped (crash)", U.judge(dict(GOOD, rc=2, phase="rolled_back", reason="the hub stopped (crash)"))[1])
        self.assertIn("2 hubs", U.judge(dict(GOOD, hubs=2))[1])
        self.assertEqual(U.judge(dict(GOOD, answers=False))[0], "red")
        self.assertIn("never came up", U.judge(dict(GOOD, up=False))[1])
        self.assertEqual(U.judge(dict(GOOD, why="TimeoutExpired: sbswitch"))[1], "TimeoutExpired: sbswitch")

    def test_a_red_blocks_a_network_failure_says_so(self):
        self.assertEqual(U.exit_code([("a", "green", ""), ("b", "green", "")]), 0)
        self.assertEqual(U.exit_code([("a", "green", ""), ("b", "network", "")]), 3)
        self.assertEqual(U.exit_code([("a", "red", ""), ("b", "network", "")]), 1)
        self.assertEqual(U.exit_code([]), 3, "no path walked is never a pass")


if __name__ == "__main__":
    unittest.main()
