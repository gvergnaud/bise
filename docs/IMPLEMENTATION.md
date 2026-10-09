# Switchboard — implémentation (journal de travail)

Sur `main` (branche `switchboard` fusionnée), checkout `~/lab/bend-lab/harness`.
Ce fichier sert aussi de mémoire de travail : il dit où en est le code.

## Architecture retenue (option B de la RFC 0001, §13)

- **Crate Rust `rust/switchboard`** (lib), branchée dans `bend-harness` :
  - `bend-harness switchboard` : client (TUI ou mode ligne), lance le hub
    si besoin ;
  - `bend-harness sbd --workspace DIR` : le hub (daemon, un par workspace) ;
  - `bise sb …` : la CLI des agents ; `<state>/bin/sb`, dans le PATH de
    chaque REPL, est un lien vers le binaire, qui fait `sb` quand il est
    appelé sous ce nom (comme busybox).
- **Une REPL Bend (`repl-live`) par agent** (main + chaque tâche), lancée
  par le hub. Le hub est le seul client TCP de chaque REPL.
  - message à un agent inactif : `say <texte>` sur le socket ;
  - message à un agent en plein tour : fichier de steering (ADR 0005) ;
  - interruption : fichier d'interruption.
- **Outils des agents = la CLI `sb` via le tool bash** (pas de nouveaux
  outils natifs) : `sb list/send/wait/ask/report/status/spawn/inspect/
  interrupt/stop/drop/card/history`.
- **État du hub** : journal `journal.jsonl` (ajout seul) + projection en
  mémoire. (La RFC dit SQLite ; JSONL suffit et reste lisible.)
- **Répertoire d'état** : `SB_STATE_DIR` ou
  `$XDG_STATE_HOME/switchboard/<nom>-<hash>` (défaut
  `~/.local/state/switchboard/…`).

### Changements Bend (petits, avec lois)

1. `BEND_WORKDIR` : le tool bash fait `cd` dans ce dossier (workspace ou
   worktree). La REPL reste lancée depuis la racine de l'app.
2. `BEND_EXTRA_PROMPT` : fichier ajouté au prompt système live (rôle de
   main ou de la tâche).
3. `BEND_CONTEXT_FILE` : fichier relu à chaque appel au modèle (appels
   agent seulement, pas la compaction) et ajouté en dernier message
   `user`, jamais stocké dans l'historique (tableau des tâches, §8.2).

## Écarts assumés par rapport aux RFC

- Journal JSONL au lieu de SQLite.
- `wait` borné à `BEND_BG_AFTER - 5` s (le tool bash passe en arrière-plan
  après `BEND_BG_AFTER`). Une réponse plus tardive arrive comme un nouveau
  message et réveille l'agent.
- `needs_approval` n'existe pas : ce harness n'a pas de porte
  d'approbation sur bash.
- Le rapport automatique de fin de tour (RFC 0001 §7.2) met à jour le
  tableau sans réveiller main. Main est réveillé par les réponses
  automatiques (RFC 0003 §7) aux messages qu'il a envoyés.
- Le résumé d'échange direct est livré avec le prochain message à main
  (il ne démarre pas de tour à lui seul).
- `/new` sans nom : le hub dérive le nom du brief (pas de tour de main).
- Budget de tokens : non appliqué si le runtime ne publie pas l'usage.

## Lancer

```sh
cd ~/mon-projet
bise                      # TUI ; lance le hub si besoin, ou rejoint celui qui tourne
bise switchboard --stop   # arrête le hub et les agents
bise doctor               # vérifie le Mac, l'install, les clés, les hubs
```

`bise` est dans `~/.local/bin` (BISE-129). Sur la machine de dev, le canal
dev l'installe une fois :

```sh
sh ~/lab/bend-lab/harness/packaging/install.sh --dev
```

Ce `bise` lance la version que fait tourner le hub du dépôt de dev
(`versions.json` 'current' : ce que `/restart` et `sb restart` choisissent),
donc chaque restart là-bas met à jour `bise` partout. `bise --version` dit
laquelle ; `BISE_DEV_VERSION=<id> bise` en lance une autre ;
`install.sh --uninstall --dev` l'enlève. `./run.sh` reste pour compiler et
lancer depuis les sources (un worktree, un test).

Fermer le TUI (`/quit`, Ctrl+C) laisse le hub et les agents tourner. Le
relancer dans le même dossier retrouve tout (fils, tâches, cartes).

## Tester

```sh
tests/run_all.sh          # lois Bend, tests Rust, E2E, TUI
tests/run_all.sh --live   # + un test avec le vrai modèle
```

- `rust/switchboard` : 61 tests (22 scénarios du core, routeur, CLI,
  tableau, worktrees sur de vrais dépôts git).
- `tests/e2e.py` (10 scénarios, dont les contrôles de main, dont plantage d'une tâche + `sb tasks`, et
  `sb inspect main --origin` + recherche par une tâche) : le vrai hub, de vraies REPL, le vrai `sb`, de vrais
  worktrees ; le modèle est `tests/fake_provider.py`, piloté par des
  marqueurs `[[bash: …]]` dans les messages.
- `tests/tui_tmux.py` : le TUI dans tmux (panneau, checkout, Esc, Alt+N,
  aperçu, /tasks, D).
- `tests/live_smoke.py` : le modèle par défaut du harness crée une tâche,
  la tâche écrit un fichier, main répond.

## Lire le fil d'un agent (`sb inspect`)

Module `rust/switchboard/src/transcript.rs` (pur, testé) ; le daemon lit
le fichier et appelle ce module. Détail dans la RFC 0001 §7.5 bis.

- Position `#<n>` = numéro de ligne dans `agents/<dir>/transcript.log`
  (fichier en ajout seul : la position est stable).
- `--before/--after/--around/--at #<n>`, `--query`, `--limit` (`--last`
  reste accepté). Page bornée : 20 entrées, 6 000 caractères.
- `--origin` : la ligne `sb spawn : … → nouvelle tâche @<dir>` du fil de
  main (la plus proche de `created_ms` si le nom a servi deux fois), et le
  dernier `sb you :` avant elle. Rien de nouveau dans le journal : tout se
  relit depuis le transcript.
- Pas de restriction sur qui lit qui (comme avant).

## Touches (TUI)

Compositeur vide : Alt+↑↓ choisir dans le panneau (Ctrl+J/K retirés, BISE-302), Ctrl+1…9 ouvrir l'élément N de l'inbox, ⏎ entrer (checkout),
Espace aperçu, D drop, Esc revenir à main, Alt+1…9 tâche N, Alt+0 main,
Ctrl+A répondre à la carte suivante, Ctrl+Z annuler le dernier routage,
Ctrl+O shell dans le dossier de l'agent affiché. `/help` liste les
commandes.

## Pas implémenté (v1)

- `needs_approval` (pas de porte d'approbation dans ce harness).
- Détection d'un worktree supprimé à la main (RFC 0002 §9) : la REPL
  redémarre en boucle puis la tâche passe en `failed`.
- Skills du workspace : la REPL tourne depuis la racine de l'app, donc
  `$PWD/.agents/skills` est celui du harness.
- `/reload` en mode switchboard.

## Plan

- [x] P0 Bend : BEND_WORKDIR, BEND_EXTRA_PROMPT, BEND_CONTEXT_FILE + lois,
      PROOF vert, repl-live recompilé.
- [x] P1 Rust pur : modèle, journal/projection, routeur, règles de
      livraison, tableau ; tests unitaires.
- [x] P2 Hub : superviseur des REPL, livraison, socket client, socket CLI.
- [x] P3 CLI `sb` + prompts de main et des tâches.
- [x] P4 Worktrees : création, drop avec sauvegarde, restore, isolate.
- [x] P5 TUI : vues par agent, liste des tâches, checkout/Esc, aperçu,
      cartes, compteurs.
- [x] P6 Tests E2E : faux provider (Python), client headless ; test live ;
      test TUI sous pty.
- [x] P7 Docs et commit.

## État

- P0 fait (commit 179e72d) : 8 lois ajoutées, PROOF vert, demo déterministe.
- P1-P4 faits : `rust/switchboard` (core + 22 tests de scénario, worktree
  + tests git, daemon, cli, client), sous-commandes `bend-harness sb|sbd|
  switchboard`. 55 tests unitaires.
- P5 : mode switchboard du TUI (`rust/tui/src/sb.rs`), testé sous tmux.
- P6 : `tests/e2e.py` + `tests/fake_provider.py` : 7 scénarios verts
  (spawn + réponse auto, message direct + note, ask/wait, carte, worktree
  drop/restore, CLI refusée, redémarrage du hub) ; `tui_tmux.py` et
  `live_smoke.py` verts.
- Corrigé grâce au test live : un `sb report` répond à la demande du
  parent (plus de réponse automatique en double) ; un steering arrivé
  pendant la réponse finale (jamais lu par le modèle, ADR 0005
  finish_turn) relance un tour.
- `@tâche message` depuis une autre vue (RFC 0003 §5.1) : `Msg.via` = la
  vue ; balise `<user_message via=…>` (prompts::tagged) ; à la fin du tour,
  `answer_user_via` affiche `sb msg-in : @tâche : …` dans la vue, règle le
  message et note main. Testé dans un hub de dev (`--dev`).

### Notes de conception du core (pour reprendre après compaction)

- `Hub::handle(input, env) -> Vec<Effect>` ; `env` = trait (now, git).
- Entrées : ReplReady, ReplLine, ReplIdle{leftover_steer}, ReplExited,
  ClientHello/Input/Focus/Gone/Confirm/Cancel/Interrupt, Agent{token,
  from, req}, Tick.
- Effets : Journal(Event), Spawn{resume, crash_note}, Kill, Say, Steer,
  Passthrough(/compact seulement), Interrupt, Context{text}, Line
  (ligne synthétique `sb …` dans le flux d'un agent), Reply{token},
  ToClient, Renamed, State.
- Livraison (`pump`) : Down/Starting → file ; Busy → steer (ids notés)
  des seuls messages `steer` (un message `Msg.queued`, `sb send --mode
  queued`, reste en file : jamais steeré ni remis à un `sb wait`) ;
  Idle → `say` (notes de main en préfixe), sauf limite de 4 tâches
  occupées ou limite de réveils par des pairs (20/h).
- À `--- idle` : le shell lit+vide le fichier steer ; s'il restait du
  contenu, les messages steerés sont remis en file. Puis réponses
  automatiques (RFC 0003 §7) et rapport auto (tableau seulement).
- `sb wait` : waiter {token, agent, msg, deadline} ; un message
  expect_reply entrant termine le wait (`incoming_request`).
- Lignes synthétiques : `sb you : <texte>` (message de l'utilisateur),
  `sb msg-in : …`, `sb route : …`, `sb spawn : …`, `sb card : #n …`,
  `sb direct : …`, `sb info : …`, `sb warn : …`.
- Contrôles de main (`sb close N ["note"]`, `sb rename`, `sb restore`,
  `sb isolate`) : `AgentReq` → `{cmd: close|rename|restore|isolate}` →
  `req.main.go` dans hub/core.bend (réservés à main, `main_only` sinon) ;
  ils réutilisent `close_card`, `rename.apply`, `restore_task`,
  `isolate_task`. `sb version switch|rollback` : réservé à main dans le
  daemon (`version_allowed`), `list` pour tous. `sb restart` / `/restart`
  (réservé à main), BISE-131 :
  - **hors des sources de bise** (le workspace n'a pas `versions.sh` +
    `rust/switchboard/Cargo.toml`, `switch::dev_workspace`) : un
    **reload**, comme « Reload Window » de VS Code, sans rien construire.
    Un autre argument que `current` est refusé (`/version` change de
    version).
  - **dans les sources de bise** (mode dev) : inchangé (`restart_plan`,
    test `restart_is_unchanged_in_dev_and_a_reload_elsewhere`) : sans
    argument (ou `latest`) il construit HEAD puis switche dessus
    (probation) ; `<commit>` switche sur ce commit ; `current`, ou un HEAD
    déjà en cours, relance le hub seul sur la version en cours (les
    agents continuent).
  - **mécanique du reload** : le même switcher (`sbswitch --restart
    --reload`, même probation de 2 min et même rollback) relance le hub
    sur la version en cours (le binaire de sa racine, sinon celui du hub
    en cours : un arbre de dev construit dans un CARGO_TARGET_DIR). Il laisse `reload` (un id en ms) dans le state dir ;
    le nouveau hub le prend au boot (`switch::take_reload`), relance
    chaque REPL adopté à son prochain idle (`reload_repls` →
    `switch_idle_repls` : `reload`, checkpoint, même session, même port)
    et met l'id dans le résultat d'`initialize` (`reload`) ; une TUI qui a connu un autre id
    s'exec à nouveau (`follow_reload`, même binaire, même app root).
  - **rien de perdu** : le journal (agents, cartes, messages en attente
    côté hub, rôles), les transcripts (les fils), la session de chaque
    agent (son historique : le REPL repart de son checkpoint), les ports
    (commandes en arrière-plan, steer), les brouillons de chaque agent et
    leurs images, les prompts envoyés (`↑`), les messages en file de la
    TUI (sauvés avec les brouillons, rendus au `ready`), le focus. Une
    sauvegarde du state dir part dans `/tmp/sb-backup-…` comme pour un
    switch.
  - **agent en plein tour** : son REPL n'est pas tué ; le nouveau hub
    l'adopte, le tour finit sur l'ancien processus, puis il est relancé
    à son idle (jamais de tour coupé). Un REPL déjà mort en plein tour
    repart sur sa session avec « continue where you left off ».
- Dossiers par agent : `agents/<dir>/` où `dir` = nom à la création
  (un rename ne déplace rien).
