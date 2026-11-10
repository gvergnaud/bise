"""scripts/release_halt.py's decisions, on fixtures: no gh call, no network."""
import os
import sys
import unittest

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "scripts"))
import release_halt as H  # noqa: E402

RELS = [
    {"tagName": "v2026.11.9", "isDraft": False, "isPrerelease": False, "publishedAt": "2026-11-09T08:38:37Z"},
    {"tagName": "desktop-v2026.11.9", "isDraft": False, "isPrerelease": False, "publishedAt": "2026-11-09T08:44:50Z"},
    {"tagName": "v2026.11.8", "isDraft": False, "isPrerelease": False, "publishedAt": "2026-11-08T15:20:19Z"},
    {"tagName": "v2026.11.7-2", "isDraft": False, "isPrerelease": True, "publishedAt": "2026-11-08T10:00:00Z"},
    {"tagName": "v2026.11.7", "isDraft": False, "isPrerelease": False, "publishedAt": "2026-11-07T18:16:57Z"},
    {"tagName": "v2026.11.10", "isDraft": True, "isPrerelease": False, "publishedAt": None},
]


class Halt(unittest.TestCase):
    def test_a_halt_falls_back_to_the_terminal_release_published_before(self):
        self.assertEqual(H.previous(RELS, "v2026.11.9"), "v2026.11.8", "the app's release and a prerelease never count")
        self.assertEqual(H.previous(RELS, "v2026.11.8"), "v2026.11.7")
        self.assertIsNone(H.previous(RELS, "v2026.11.7"), "nothing before the first")
        self.assertIsNone(H.previous(RELS, "v2026.11.10"), "a draft was never out: nothing to halt")

    def test_the_commands_move_latest_and_pause_or_resume_both_app_channels(self):
        cmds = H.plan("halt", "v2026.11.9", RELS)
        self.assertEqual(cmds[0], ["gh", "release", "edit", "v2026.11.8", "-R", H.REPO, "--latest"])
        self.assertEqual([c[-2:] for c in cmds[1:]], [["pause", "beta"], ["pause", "stable"]])
        back = H.plan("resume", "v2026.11.9", RELS)
        self.assertEqual(back[0][3], "v2026.11.9")
        self.assertEqual([c[-2:] for c in back[1:]], [["resume", "beta"], ["resume", "stable"]])

    def test_refused_with_why(self):
        for action, tag in (("halt", "desktop-v2026.11.9"), ("halt", "v2026.11.7"), ("resume", "v2026.11.10"), ("stop", "v2026.11.9")):
            with self.assertRaises(ValueError):
                H.plan(action, tag, RELS)


if __name__ == "__main__":
    unittest.main()
