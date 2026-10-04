# Bilan : les interactions de Switchboard en Bend, et leurs lois

La logique d'interaction du hub (messages, fils, livraison, attentes, fins de
tour, plantages, tâches, cartes) tourne en Bend : `hub/*.bend`, compilé en
`sb-core`. `core.rs` ne fait plus que le lien : entrées et sorties, git, rendu.
Il n'y a qu'une seule source de vérité : sb-core renvoie l'état après chaque
étape, et le Rust n'a plus d'`apply`.

Portes au moment du bilan (HEAD) : `bend PROOF.bend` = ALL PROOFS CHECK,
`cargo test -p switchboard` = 85/85. La dernière passe e2e à 9/9 date
d'avant les lois L2 à L4 : elle n'a pas été relancée depuis.
`--verdict` n'a pas pu tourner, car Lean n'est pas installé.

## Lois prouvées (LAWS.bend, section « Switchboard hub »)

Toutes sont quantifiées sur tous les états, toutes les entrées, ou toutes les
suites d'entrées.

| Loi | Ce qu'elle garantit |
|---|---|
| hub_step_replays, hub_run_replays | L'état est exactement le rejeu du journal : un redémarrage le reconstruit. |
| msg_ids_fresh | Chaque message prend l'id libre suivant : aucun message n'en écrase un autre. |
| auto_never_asks | Une réponse automatique ne pose jamais de question : pas de chaîne. |
| tick_clears_late_waiters | Après un tick, toute attente échue a reçu son timeout : pas d'interblocage au-delà de 25 s. |
| stop_leaves_no_waiter | Un stop ou un drop ne laisse aucune attente de la tâche. |
| init_unique, msgs_unique (L2) | Chaque message est un seul enregistrement, avec un seul état. |
| msgs_kept (L2) | Un message envoyé n'est jamais retiré. |
| delivered_once (L3) | Un message est livré seulement depuis la file. Il n'y retourne que par un steer_leftover explicite. |
| stop_leaves_no_queue, stop_leaves_no_card (L12) | Un stop ou un drop ne laisse ni message en file ni carte ouverte de la tâche. |
| init_cards_unique, cards_unique | Deux cartes ouvertes n'ont jamais le même id (hypothèse de L12). |
| fire_is_a_message | Un timer (sb every) ne compte un tir que si son message de réveil est en file, dans le même pas. |
| wake_never_stacked | Un timer ne tire jamais vers un agent qui a déjà un message de bise en file. |
| times_bound | Un timer ne tire jamais plus que ses `--times` : le dernier tir l'arrête dans le même pas. |
| stop_leaves_no_timer | Un archivage ou un drop ne laisse aucun timer de l'agent (sb stop les garde en attente). |
| init_names_apart, names_apart | Les noms et anciens noms de deux agents ne se croisent jamais (préalable de L4). |

## Lois ouvertes

- **L4 : aucun agent inactif avec des messages en file.** Arrêtée à la demande de l'utilisateur.
  - Fait : `pump_wakes` (une livraison ne laisse jamais son agent bloqué),
    `pump_frame` (elle ne bloque aucun autre agent), l'invariant « aucun
    bloqué sauf {A} » que le pump de A rétablit, et les lemmes support
    (égalité de chaînes réflexive et correcte, noms disjoints, files qui ne
    font que se vider).
  - Reste : le cadre de chaque type d'événement (lifecycle, created, renamed,
    declared…, re-file, message envoyé via names_apart), puis l'enchaînement
    à travers les ~25 handlers, puis la loi sur toute suite d'entrées.
    J'estime 5 à 8 h.
- **L13 (plantages bornés)** : laissée ouverte. La version bon marché serait vraie par
  construction ; la vraie preuve coûte 3 à 4 h pour une loi de faible valeur.
- **L5** (toute question reçoit une réponse), **L9** (une réponse par requête `sb`),
  **L11** (qui peut réveiller une tâche), **L8** (relations parent/enfant) :
  non commencées. Elles demandent le même genre d'invariant global que L4.

## Temps réel et estimations

| Travail | Temps réel | Estimation |
|---|---|---|
| Portage en Bend + branchement Rust + option A | ~3 h | — |
| Les 6 premières lois (rejeu, ids, autoreply, tick, stop) | ~40 min | — |
| L2 | 7 min | 2-3 h |
| L3 | 6 min + rapport de bugs | 4-6 h |
| L12 | 16 min | 1 h |
| L4 (partielle) | ~1 h 15 | 4-6 h, révisée ensuite à 8-10 h |

L2, L3 et L12 ont été rapides parce que l'invariant voyage avec le contexte
d'étape. Ce type `Core<o>` porte ses propres preuves : chaque `emit` prouve
que l'événement garde les invariants du journal. Les lois sur toute suite
d'entrées en découlent par induction. L4 est lente pour la raison inverse :
elle mêle l'état durable et l'état runtime, et doit donc être prouvée à
travers chaque handler.

## Bugs trouvés par les preuves (5)

Chacun est corrigé avec un test de régression. Les 4 premiers existaient déjà
dans le Rust d'origine.

1. `sb wait` re-livrait une réponse déjà livrée : deux livraisons du même
   message (preuve de L3, 0fcb21d).
2. `sb wait` prenait une réponse `--mode queued` encore en file, contre la
   règle de ce mode (preuve de L3, 0fcb21d).
3. Un message en file pour une tâche renommée restait bloqué pour toujours
   (en énonçant L4, 07839fe : les files suivent les alias).
4. `/restore` d'une tâche en échec mais inactive ne lui livrait pas son
   courrier en file (preuve de L4, e32abac).
5. L'entrée de test `force_run` changeait le run sans livrer (preuve de L4,
   3461173 ; spécifique au portage, jamais envoyée par le daemon).

Aucun des tests existants ne voyait ces bugs.

## Mon avis sur Bend pour le hub

- **Ce qui marche** : des lois vraiment universelles, qui ont trouvé des bugs
  réels. Porter les preuves dans le type du contexte d'étape rend les lois
  « sur tout le journal » presque gratuites. Le code prouvé est le code qui
  tourne.
- **Ce qui coûte** :
  - pas de tactiques, donc chaque réécriture s'écrit à la main ;
  - pas de `match` sur une valeur calculée, ni de récursion mutuelle : des
    dizaines de petites fonctions d'aide ;
  - `Bool.pick` évalue ses deux branches ;
  - il a fallu prouver soi-même que l'égalité de chaînes est correcte ;
  - un invariant global sur une grosse machine à états (L4) se prouve handler
    par handler : c'est là que les heures partent.
- **Friction d'outillage** : la REPL plantait sur les commandes bash de plus
  de 20 000 caractères (BR-006). Le worktree est partagé : un de mes commits
  a embarqué des changements indexés par une autre tâche. Je commite depuis
  par un index privé.
- **Ma recommandation** : garder Bend pour le cœur, et viser d'abord les lois
  qu'un invariant porté par le contexte peut établir (sûreté du journal, des
  ids, des livraisons). Réserver les invariants globaux runtime × durable
  (L4, L5, L9) aux cas où un vrai bug l'a justifié.
