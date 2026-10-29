---
name: quiz-cards
description: Quiz the user with flashcards and follow their score. Use when the user wants to learn, revise, memorize, practice vocabulary, or asks to be quizzed on something.
---

# Quiz cards

1. Write 5 to 12 cards on the topic and call `tools.quiz_cards.make_quiz`
   from run_typescript with `topic` and `cards: [{q, a, hint}]`: short
   questions, short answers, a hint only when the question is hard.
   Without cards it uses a built-in deck (capitals, spanish, http).
2. Never give the answers in your reply: the user flips the cards.
3. The score comes back as model context while they play ("Quiz … 3/4
   right so far. Missed: …") and as a message at the end ("I got 3/5 on
   …. Quiz me on the ones I missed: …"). For that message, make a new
   quiz on the misses, asked another way (reverse the question, add a
   hint), plus one or two new related cards.
