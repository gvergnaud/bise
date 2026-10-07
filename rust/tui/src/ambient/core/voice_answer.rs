//! Answering the card in view by voice (bar N14/L8, architect m_10930).
//! The focused window says which card is in front of him (`card_in_view`,
//! the ⌘I inbox's current item; none when it blurs, closes or leaves the
//! inbox); when fn's talk ends, its words are tried against that card
//! with the TUI's own rule (voicemode/answers.rs `decide`: `pick` for a
//! question, only "allow" for an approval). A match sends `voice_answer`
//! (the window shows the heard line for `ms`, esc undoes, then answers
//! through the hub's one answer path) and the words don't go to main; a
//! card that closed meanwhile, or no match: the words go to main as
//! before. Voice mode tries its turns against the open card of the agent
//! he talks to, through the same rule. Owner: amb-home.

use super::*;
use crate::voicemode::answers;

impl Core {
    /// True: `words` answered the card in view (`voice_answer` sent).
    pub(super) fn voice_answer(&mut self, words: &str) -> bool {
        let Some((project, id)) = self.in_view.clone() else { return false };
        self.voice_answer_on(&project, |c| c.id == id, words).is_some()
    }

    /// Voice mode (core/voice_mode.rs, architect m_11164): his turn tried
    /// against the open card of the agent he talks to, with the same rule
    /// (`decide`) and the same `voice_answer`. Some(line): answered.
    pub(super) fn voice_answer_agent(&mut self, project: &str, agent: &str, words: &str) -> Option<String> {
        self.voice_answer_on(project, |c| c.agent == agent, words)
    }

    /// The first open card of `project` that `which` picks, answered by
    /// `words` when the TUI's rule says they name one of its options.
    fn voice_answer_on(&mut self, project: &str, which: impl Fn(&bise_proto::rows::Card) -> bool, words: &str) -> Option<String> {
        let card = self.project_rows().into_iter().filter(|(p, _, _)| p == project).flat_map(|(_, _, cards)| cards).find(|c| which(c))?;
        let labels: Vec<String> = card.options.iter().map(|o| o.label.clone()).collect();
        let (i, line) = answers::decide(words, card.approval, &labels)?;
        let opt = card.options.get(i)?;
        self.emit(json!({"ev": "voice_answer", "project": project, "card": card.id, "n": opt.n, "line": line, "ms": answers::HEARD_FOR.as_millis() as u64}));
        Some(line)
    }
}
