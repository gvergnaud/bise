# RFC 0002 : Worktree optionnel par tâche

- Statut : Brouillon
- Date : 2026-09-27
- Auteur : Gabriel Vergnaud
- Étend : [`rfc-0001-switchboard.md`](rfc-0001-switchboard.md) (§7.1 brief,
  §9.4 drop, §10.3 conflits de fichiers)

Les mots **DOIT**, **NE DOIT PAS**, **DEVRAIT** et **PEUT** ont le sens de la
RFC 2119.

## 1. Résumé

Une tâche est une session d'agent. Elle n'a pas de lien avec git par défaut :
elle travaille dans le dossier du workspace, peut durer longtemps, et n'a pas
de branche.

L'utilisateur PEUT demander qu'une tâche tourne dans un **worktree git
isolé**. C'est la seule façon d'obtenir un worktree : ni main ni le hub ne le
choisissent seuls.

`/drop <nom>` (RFC 0001, §9.4) arrête une tâche et l'archive. Si la tâche a
un worktree, le même `/drop` supprime aussi le worktree et sa branche locale.
Si du travail risque d'être perdu, le hub le sauvegarde d'abord dans un ref
git caché, récupérable avec `/restore <nom>`, sans limite de durée.

Switchboard ne merge rien et n'a pas de commande pour pousser. Pour pousser
la branche, l'utilisateur le demande à l'agent dans la conversation ; il
merge lui-même, par exemple avec une pull request GitHub.

## 2. Décisions

- 2026-09-27 : pas de worktree par défaut. Seulement sur demande de
  l'utilisateur.
- 2026-09-27 : pas de commande de merge (`/land`). Le merge se fait hors de
  Switchboard.
- 2026-09-27 : pas de worktrees préparés à l'avance.
- 2026-09-27 : pas d'expiration des sauvegardes (`/restore` marche
  toujours).
- 2026-09-27 : pas de `/push`. L'utilisateur demande à l'agent de pousser.

## 3. Demander un worktree

Deux façons, toutes deux à l'initiative de l'utilisateur :

1. **Commande :** `/new -w fix-safari: le login casse sur Safari`.
2. **Dans main, en langage naturel :** « lance ça dans un worktree isolé ».
   Main appelle alors `task_spawn` avec `workspace: "worktree"`.

Règles pour main :

- Main NE DOIT PAS choisir `worktree` si l'utilisateur ne l'a pas demandé
  pour cette tâche.
- Main PEUT le suggérer (par exemple quand deux tâches modifient les mêmes
  fichiers, §10.3 de la RFC 0001), mais seulement sous forme de question.
- Si le workspace n'est pas un dépôt git, la demande est refusée avec un
  message clair. La tâche n'est pas créée.

Le fil de main affiche le worktree dans la ligne de routage :
`main → nouvelle tâche @fix-safari (worktree sb/fix-safari)`.

### 3.1 Ajouter un worktree à une tâche existante

`/isolate <nom>` passe une tâche `shared` en `worktree`. C'est permis
seulement si la tâche n'a encore modifié aucun fichier (§10.3 de la RFC
0001). Sinon, le hub refuse : il ne peut pas séparer les modifications de la
tâche de celles de l'utilisateur.

Le passage inverse (`worktree` → `shared`) n'existe pas : ce serait un merge.

## 4. Création d'un worktree

### 4.1 Étapes

Pendant l'état `starting`, le hub :

1. Résout la base : `worktree.base` (défaut : la pointe de main, la
   branche par défaut du dépôt, jamais le `HEAD` du workspace qui peut
   être sur la branche d'un autre agent ; en flow PR, sa copie
   `origin/<main>`, sans fetch ; issue #8, `trunk.rs`) en un commit. Il
   l'enregistre comme `base_commit`.
2. Choisit le nom de branche `sb/<nom>`. Si la branche existe déjà, il
   ajoute `-2`, `-3`, etc.
3. Lance `git worktree add -b <branche> <chemin> <base_commit>`.
4. Copie les fichiers listés dans `worktree.copy` (par exemple `.env`), s'ils
   existent dans le workspace.
5. Lance `worktree.setup` dans le worktree (par exemple `pnpm install`).
6. Démarre la session du sous-agent avec le worktree comme dossier de
   travail, puis envoie le brief.

Si une étape échoue, la tâche passe en `failed` avec le log de l'étape, et
le hub supprime le worktree à moitié créé.

### 4.2 Modifications non commitées du workspace

Le worktree part de `base_commit`. Il ne contient **pas** les modifications
non commitées du workspace.

- Si le workspace a des modifications non commitées, la ligne de routage
  affiche un avertissement.
- `/new -w --with-changes …` copie ces modifications dans le worktree
  (`git stash create`, puis `git stash apply` dans le worktree). Le workspace
  n'est pas modifié.

### 4.3 Emplacement

Défaut : `$XDG_STATE_HOME/switchboard/<workspace-id>/worktrees/<nom>`,
**hors du dépôt**. Dans le dépôt, le worktree serait vu par `rg`, les
watchers et les outils de test du workspace.

Le chemin est affiché dans la vue checkout et peut être copié.

### 4.4 Confinement du sous-agent

- Le dossier de travail de la session est le worktree.
- Le prompt système dit au sous-agent de modifier seulement des fichiers du
  worktree.
- Si le runtime supporte des racines de système de fichiers (déclarations
  filesystem du Unified Harness), le hub DEVRAIT limiter les écritures au
  worktree.

### 4.5 Pousser la branche

Le sous-agent PEUT commiter dans sa branche. Il NE DOIT PAS pousser sans que
l'utilisateur le demande. Pour pousser, l'utilisateur le demande à l'agent
dans la conversation (checkout, ou via main) ; l'agent lance `git push`.
Le merge se fait ensuite sur GitHub.

## 5. Drop d'une tâche avec worktree

Le drop général est défini dans la RFC 0001, §9.4. Pour une tâche avec
worktree, le hub ajoute ces étapes entre « arrêter » et « archiver ».

### 5.1 Mesurer ce qui serait perdu

- `dirty` : fichiers modifiés ou non suivis, hors `.gitignore`.
- `unpushed` : commits de la branche absents de toute branche distante
  (`git rev-list <branche> --not --remotes`).

Un commit poussé est considéré comme sauvé. Cas connu : après un squash
merge sur GitHub suivi de la suppression de la branche distante, les commits
locaux ne sont plus sur aucun remote. Ils sont alors comptés comme
`unpushed` : le hub fait une sauvegarde et demande une confirmation. C'est
inutile mais sans danger.

### 5.2 Sauvegarder si besoin

Si `dirty` ou `unpushed`, le hub crée un commit de sauvegarde **sans toucher
au worktree ni à son index** :

1. Copier l'index du worktree dans un fichier temporaire.
2. `GIT_INDEX_FILE=<copie> git add -A`, puis `git write-tree`.
3. `git commit-tree -p HEAD` avec cet arbre.
4. `git update-ref refs/switchboard/trash/<nom>/<horodatage> <commit>`.

Les fichiers ignorés (`node_modules`, builds, fichiers copiés comme `.env`)
ne sont pas sauvegardés. `worktree.setup` les recrée.

### 5.3 Supprimer

1. `git worktree remove --force <chemin>`.
2. `git branch -D <branche>`. La branche **distante** n'est jamais
   supprimée.
3. La tâche est archivée avec `worktree.state = "dropped"`.

### 5.4 Confirmation

| Situation | Confirmation |
|---|---|
| Rien à perdre (propre, et tout est poussé ou aucun commit) | Aucune. |
| Sauvegarde faite | Une ligne : `Drop @fix-safari ? 3 fichiers modifiés et 2 commits non poussés seront sauvegardés (/restore). [y/N]` |

Il y a au plus une confirmation pour tout le drop (tâche en cours comprise,
RFC 0001 §9.4). `--force` la saute.

### 5.5 Qui peut dropper

- L'utilisateur, toujours.
- Main, seulement si rien n'est à perdre. Sinon, main ouvre une carte
  d'attention qui propose le drop.

### 5.6 Restaurer

`/restore <nom>` :

1. Recrée la branche au parent du commit de sauvegarde, et le worktree.
2. Remet le contenu de la sauvegarde dans le worktree, sans commit
   (`git checkout <sauvegarde> -- .`, puis `git reset`).
3. Relance `worktree.copy` et `worktree.setup`.
4. La tâche passe en `idle`, avec son historique de session.

La restauration ne garde pas la différence entre fichiers indexés et non
indexés : tout revient comme modifications non indexées.

Les refs de sauvegarde n'expirent jamais : le hub ne les supprime pas. Un
`/restore` réussi supprime le ref qu'il a utilisé.

## 6. Affichage

- La liste des tâches marque les tâches avec worktree : `⎇`.
- Le tableau des tâches envoyé à main ajoute la branche et un diffstat :
  `fix-safari  idle  ⎇ sb/fix-safari  +120 −14 (3 commits, 1 non poussé)`.
- La vue checkout affiche en haut : branche, chemin, base, diffstat.
- `Ctrl+O` dans un checkout ouvre un shell dans le dossier de travail de la
  tâche (worktree ou workspace). La commande est configurable.

## 7. Configuration

`.switchboard/config.toml` dans le workspace :

```toml
[worktree]
root = ""                        # vide = $XDG_STATE_HOME/switchboard/<id>/worktrees
base = ""                        # vide = la pointe de main ; ou "origin/main", "HEAD"
branch_prefix = "sb/"
copy = [".env", ".env.local"]    # copiés du workspace s'ils existent
setup = ""                       # par exemple "pnpm install --frozen-lockfile"
```

## 8. Ajouts au modèle de données

```ts
interface TaskWorkspace {
  mode: "shared" | "worktree";
  // présents seulement si mode = "worktree"
  path?: string;
  branch?: string;
  base_commit?: string;
  state?: "creating" | "ready" | "dropped";
}

type HubEvent =
  // … événements de la RFC 0001
  | { type: "worktree_created"; task: string; path: string; branch: string; base_commit: string }
  | { type: "worktree_setup_failed"; task: string; step: "add" | "copy" | "setup"; log: string }
  | { type: "worktree_removed"; task: string; snapshot_ref?: string; dirty_files: number; unpushed_commits: number }
  | { type: "worktree_restored"; task: string; snapshot_ref: string };
```

Commandes : `/new -w`, `/new -w --with-changes`, `/isolate`, `/restore`. Le
drop reste `/drop` (RFC 0001). Chacune peut aussi être demandée à main en
langage naturel.

## 9. Cas limites

| Cas | Comportement |
|---|---|
| Le dossier du worktree a été supprimé à la main | Le hub le détecte (`git worktree prune`), passe la tâche en `failed` et propose `/restore` si une sauvegarde existe, sinon `/drop`. |
| L'utilisateur change de branche dans le workspace | Aucun effet sur les worktrees : ils partent de `base_commit`. |
| Deux tâches ont besoin du même port (serveur de dev) | Le hub met `SB_TASK` et `SB_PORT_OFFSET` (0, 1, 2…) dans l'environnement de chaque tâche. Le reste dépend du projet. |
| La branche `sb/<nom>` est déjà checkoutée ailleurs | `git worktree add` échoue ; le hub essaie le nom suivant (`-2`). |
| Message envoyé à une tâche dropée qui avait un worktree | Refusé : la tâche ne repart pas sans son dossier de travail. Le hub propose `/restore` si une sauvegarde existe. |
| Drop pendant `setup` | Le hub tue le setup et supprime le worktree. Aucune sauvegarde n'est nécessaire. |
| `/new -w` dans un dossier qui n'est pas un dépôt git | Refusé. La tâche n'est pas créée. |

## 10. Questions ouvertes

Aucune pour l'instant.
