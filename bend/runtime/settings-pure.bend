# runtime/settings-pure.bend - the pure half of runtime/settings.bend: everything the laws pin, with
# no IO, no @unsafe and no foreign code in its closure (the
# PROOF gate refuses any in the import closure since Bend 2.0.32).
import Base

def CONFIG_TEMPLATE() -> String:
  "# bend-harness configuration.\n# Precedence on every key: environment variable > this file > built-in default.\n\n# The model to use, \"provider/model\" (bise models lists them). No model:\n# bise asks for a provider and checks its key at the next launch.\n# model = \"anthropic/claude-opus-5-5\"\n\n# When the conversation compacts: a number of estimated tokens, or a\n# share of the model\'s context window as \"45%\". Never above 80% of the\n# window (a number set for a 1M model stays safe on a 200k one).\n# Default: 80% of the window.\n# compaction_threshold = \"45%\"\n\n# After a compaction the agent keeps the summary and, verbatim, your most\n# recent messages up to this many estimated tokens, counted from the\n# newest (the latest is always kept). Default: 6000.\n# compaction_keep_tokens = 6000\n\n# A bash command still running after this many seconds moves to the\n# background and the agent keeps going (default 30)\nbg_after = 30\n"
