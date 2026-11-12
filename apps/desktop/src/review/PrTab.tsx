// A PR's tab (pr-integration A, rules 7 to 12; page pr-guided V2: the PR IS its review): its title,
// whose ▾ (and right click) is its menu (Open on GitHub ↗, copy its link and branch, Close after a
// one-line confirm); then, scrolling away over its guided review, one state line from the hub's
// row (ArtifactPr.status: '#412 · draft · conflicts · checks ✗ 1', pink only what waits on him; a
// failing piece opens the checks) with Open on GitHub ↗ and Merge (a ghost button, only when the
// forge says mergeable, at the head he sees: pr/act) and, at its end, the PR's size as GitHub's
// (+438 −151 and five squares, diffstat.ts, once its diff comes); its description folded to its first line,
// which opens what the Summary was (its branch, the description as untrusted markdown, checks with
// 'Ask <agent> to fix' that fills the composer and never sends, reviews read-only, its agents);
// then its diff (the hub's pick of place or forge) as the guided review (DiffFiles: its bar sticky,
// a part's 'Ask <agent>'). Every wait says what it waits for, with the link out beside a miss.
// Owner: pr-artifacts.
import { useEffect, useMemo, useRef, useState, type MouseEvent, type ReactNode } from 'react';
import { Button, Empty, Head, Press, Skel, tipAttrs, type EntityMenu } from '@/ds';
import { prDiffKey, useArtifacts, useDiffError, usePlaces, usePrActed, usePrCheckLog, usePrDetail, usePrDiff, useStore } from '@/store';
import type { Artifact, GuideTarget, Pr, PrCheck, PrDetail } from '@/proto';
import { panel } from '@/proto/words_panel.gen';
import { menus } from '@/proto/words_menus.gen';
import { pages } from '@/proto/words_pages.gen';
import { Markdown } from '@/thread/Markdown';
import { useRouter } from '@/app/router';
import { useNow } from '@/app/clock';
import { keptStorage } from '@/app/keep';
import { askCaret } from '@/composer/caret';
import { putTextInDraft } from '@/artifacts/put';
import { prLine } from '@/artifacts/pr';
import { PrWords } from '@/artifacts/PrWords';
import { DiffFiles } from './DiffFiles';
import { diffStat, SQUARES } from './diffstat';
import { useOpenChange } from './openChange';
import { actLine, askFixText, branchFacts, canClose, canMerge, checkMark, errWords, firstLine, forgeWord, verdictWord } from './prwords';
import './prtab.css';

const bise = () => (window as Partial<Window>).bise;
const openOut = (url: string) => {
  void bise()?.openExternal(url);
};

/** The description's fold: shut, open, or open on its checks (a failing piece's click). */
type Fold = 'shut' | 'open' | 'checks';

export function PrTab({ project, url }: { project: string; url: string }) {
  const store = useStore();
  const arts = useArtifacts(project);
  const art = arts.find((a) => a.pr != null && a.url === url);
  const s: Pr | null = art?.pr?.status ?? null;
  const line = art === undefined ? null : prLine(art);
  const tag = line?.sigil ?? '';
  const forge = forgeWord(s?.provider);
  const acted = usePrActed(project, url);
  const said = actLine(acted, forge);
  const [confirming, setConfirming] = useState(false);
  const busy = acted !== null && !acted.done;

  const menu: EntityMenu = {
    onContextMenu: (e: MouseEvent<HTMLElement>) => {
      e.preventDefault();
      void showMenu();
    },
    open: () => {
      void showMenu();
    },
  };
  // its title's ▾: the PR's menu (review-guided m_58300); Open on GitHub stays on the line too
  async function showMenu(): Promise<void> {
    const popup = bise()?.popupMenu;
    if (popup === undefined) return;
    const items = [
      { id: 'open', label: panel.prtab_open_on({ forge }) },
      { type: 'separator' as const },
      { id: 'copy-link', label: menus.copy_link, icon: 'link' },
      ...(s === null ? [] : [{ id: 'copy-branch', label: menus.copy_branch }]),
      ...(canClose(s) && !busy ? [{ type: 'separator' as const }, { id: 'close', label: panel.prtab_close, destructive: true }] : []),
    ];
    const id = await popup(items);
    if (id === 'open') openOut(url);
    else if (id === 'copy-link') void bise()?.copy(url);
    else if (id === 'copy-branch' && s !== null) void bise()?.copy(s.branch);
    else if (id === 'close') setConfirming(true);
  }

  const merge = () => {
    if (s?.head_oid != null) store.prAct(project, url, s.head_oid, 'merge');
  };
  const close = () => {
    setConfirming(false);
    store.prAct(project, url, s?.head_oid ?? '', 'close');
  };

  const out = (
    <Button kind="ghost" size="sm" onClick={() => { openOut(url); }} testId="prtab.open">
      {panel.prtab_open_on({ forge })}
    </Button>
  );

  return (
    <section className="prt" data-testid="prtab" data-url={url}>
      <Head
        title={
          <Press look="prt-title" aria-haspopup="menu" aria-label={panel.prtab_menu} {...tipAttrs(panel.prtab_menu)}
            onClick={menu.open} onContextMenu={menu.onContextMenu} data-testid="prtab.menu">
            <span className="prt-title-t" data-testid="prtab.title">{art?.title ?? tag}</span>
            <span className="prt-caret" aria-hidden>▾</span>
          </Press>
        }
      />
      {said !== null && (
        <p className={said.err ? 'prt-said prt-err' : 'prt-said'} data-testid="prtab.acted">{said.words}</p>
      )}
      {confirming && (
        <div className="prt-confirm" role="alertdialog" data-testid="prtab.confirm">
          <span>{panel.prtab_close_confirm({ tag, forge })}</span>
          <Button kind="danger" size="sm" onClick={close} testId="prtab.confirm.yes">{panel.prtab_close_yes}</Button>
          <Button size="sm" onClick={() => { setConfirming(false); }} testId="prtab.confirm.no">{panel.prtab_close_no}</Button>
        </div>
      )}
      {art === undefined ? (
        <Empty testId="prtab.waiting" title={panel.prtab_waiting_row} action={out} />
      ) : (
        <Changes project={project} url={url}
          lead={<Lead project={project} url={url} art={art} s={s} tag={tag} forge={forge} out={out} merge={canMerge(s) && !busy ? merge : null} />} />
      )}
    </section>
  );
}

/**
 * What scrolls away over the guide (designer m_58284): the one state line, then the description
 * folded to its first line; the guide's bar stays sticky under them.
 */
function Lead({ project, url, art, s, tag, forge, out, merge }: {
  project: string; url: string; art: Artifact; s: Pr | null; tag: string; forge: string; out: ReactNode; merge: (() => void) | null;
}) {
  const store = useStore();
  const ans = usePrDetail(project, url);
  const [fold, setFold] = useState<Fold>('shut');
  useEffect(() => {
    store.askPrDetail(project, url);
  }, [store, project, url]);
  const first = ans?.detail == null ? '' : firstLine(ans.detail.body);
  return (
    <div className="prt-lead" data-testid="prtab.lead">
      {/* the hub's words in one line, the acts after them: narrow, it wraps, an act is never cut */}
      <p className="prt-line" data-testid="prtab.line">
        <PrWords a={art} onFail={() => { setFold('checks'); }} testId="prtab.status" />
        <span className="prt-sep" aria-hidden> · </span>
        <Press look="prt-out" onClick={() => { openOut(url); }} data-testid="prtab.open">{panel.prtab_open_on({ forge })}</Press>
        {merge !== null && (
          <Button kind="ghost" size="sm" onClick={merge} title={panel.prtab_merge_tip({ forge })} testId="prtab.merge">
            {panel.prtab_merge}
          </Button>
        )}
        <Stat project={project} url={url} />
      </p>
      <PrAhead project={project} url={url} tag={tag} />
      <Press look="prt-desc" aria-expanded={fold !== 'shut'} onClick={() => { setFold((f) => (f === 'shut' ? 'open' : 'shut')); }} data-testid="prtab.desc.fold">
        <span className="prt-desc-k">{fold === 'shut' ? '▸' : '▾'} {panel.prtab_desc_fold}</span>
        {fold === 'shut' && first !== '' && <span className="prt-desc-first">{first}</span>}
      </Press>
      {fold !== 'shut' && <Summary project={project} url={url} s={s} tag={tag} forge={forge} out={out} onChecks={fold === 'checks'} />}
    </div>
  );
}

/** a square's side and the gap between two, in the svg's own units (px) */
const SQ = 8;
const SQ_GAP = 2;

/**
 * The PR's size at the line's end, as GitHub shows it (his m_62199): '+438 −151' and five squares
 * for the added and removed shares (diffstat.ts), over the whole diff the guide reads. Nothing
 * until that diff comes.
 */
function Stat({ project, url }: { project: string; url: string }) {
  const diff = usePrDiff(project, url);
  const st = useMemo(() => (diff === null ? null : diffStat(diff.files)), [diff]);
  if (st === null) return null;
  const w = SQUARES * SQ + (SQUARES - 1) * SQ_GAP;
  return (
    <span className="prt-stat" data-testid="prtab.stat">
      <span className="prt-add">+{st.add}</span>
      <span className="prt-del">−{st.del}</span>
      <svg className="prt-sq" width={w} height={SQ} viewBox={`0 0 ${String(w)} ${String(SQ)}`} aria-hidden data-testid="prtab.stat.squares">
        {st.squares.map((k, i) => (
          <rect key={i} className={`prt-sq-${k}`} x={i * (SQ + SQ_GAP)} y={0} width={SQ} height={SQ} rx={2} />
        ))}
      </svg>
    </span>
  );
}

/**
 * The agent's branch has commits its PR has not yet (the hub counts them, Place.ahead_of_pr;
 * architect m_58451): 'sb/perf has 2 commits not on #412 · show them', which opens exactly those
 * commits, the PR's head to the branch's tip (review-guided m_58314, designer m_58488). A push makes
 * the count 0: the line goes by itself.
 */
export function PrAhead({ project, url, tag }: { project: string; url: string; tag: string }) {
  const place = usePlaces(project).find((p) => p.pr?.url === url);
  const open = useOpenChange(project);
  const n = place?.ahead_of_pr ?? 0;
  const agent = place?.agents?.[0];
  const since = place?.pr?.head_oid ?? '';
  if (place === undefined || n <= 0 || agent === undefined || since === '') return null;
  return (
    <p className="prt-ahead" data-testid="prtab.ahead">
      <span>{pages.review_ahead_of_pr({ branch: place.branch ?? place.pr?.branch ?? '', n, tag })}</span>
      <span className="prt-sep" aria-hidden> · </span>
      <Press look="prt-out" onClick={() => { open({ agent, since }); }} data-testid="prtab.ahead.show">{pages.review_ahead_show}</Press>
    </p>
  );
}

function Summary({ project, url, s, tag, forge, out, onChecks }: { project: string; url: string; s: Pr | null; tag: string; forge: string; out: ReactNode; onChecks: boolean }) {
  const store = useStore();
  const router = useRouter();
  const ans = usePrDetail(project, url);
  const now = useNow();
  const checks = useRef<HTMLHeadingElement>(null);
  const agents = s?.agents ?? [];
  // opened from 'checks ✗ 1': its checks come into view
  const shown = ans?.detail != null;
  useEffect(() => {
    if (onChecks && shown) checks.current?.scrollIntoView({ block: 'start' });
  }, [onChecks, shown]);
  if (ans === null) return <p className="prt-wait" data-testid="prtab.loading"><Skel /> {panel.prtab_loading_detail({ forge })}</p>;
  if (ans.detail == null) {
    return (
      <div className="prt-miss" data-testid="prtab.miss">
        <span>{errWords(ans.err, forge, ans.why)}</span>
        <Button size="sm" onClick={() => { store.askPrDetail(project, url); }} testId="prtab.retry">{panel.prtab_retry}</Button>
        {out}
      </div>
    );
  }
  const d: PrDetail = ans.detail;
  return (
    <div className="prt-body" data-testid="prtab.summary">
      {s !== null && <p className="prt-facts" data-testid="prtab.facts">{branchFacts(s, now).join(' · ')}</p>}
      {d.body.trim() === '' ? <p className="prt-none">{panel.prtab_no_description}</p> : (
        <div className="prt-md" data-testid="prtab.body"><Markdown text={d.body} onLink={openOut} /></div>
      )}
      {d.body_cut === true && <p className="prt-none">{panel.prtab_body_cut({ forge })}</p>}
      <h3 ref={checks}>{panel.prtab_checks}</h3>
      {d.checks.length === 0 ? <p className="prt-none">{panel.prtab_no_checks}</p> : (
        <ul className="prt-checks" data-testid="prtab.checks">
          {d.checks.map((c) => <CheckRow key={c.name} project={project} url={url} c={c} tag={tag} forge={forge} agent={agents[0]} />)}
        </ul>
      )}
      <h3>{panel.prtab_reviews}</h3>
      {d.notes.length === 0 ? <p className="prt-none">{panel.prtab_no_reviews}</p> : (
        <ul className="prt-notes" data-testid="prtab.notes">
          {d.notes.map((n, i) => (
            <li key={`${n.author}.${String(n.at_ms)}.${String(i)}`}>
              <span className="prt-who">{[n.author, verdictWord(n.verdict)].filter((x) => x !== '').join(' · ')}</span>
              {n.body.trim() !== '' && <div className="prt-md"><Markdown text={n.body} onLink={openOut} /></div>}
            </li>
          ))}
        </ul>
      )}
      {(d.more_notes ?? 0) > 0 && (
        <Press look="prt-link" onClick={() => { openOut(url); }} data-testid="prtab.more_notes">
          {panel.prtab_more_notes({ n: d.more_notes ?? 0, forge })}
        </Press>
      )}
      {agents.length > 0 && (
        <>
          <h3>{panel.prtab_agents}</h3>
          <p className="prt-agents" data-testid="prtab.agents">
            {agents.map((a) => (
              <Press key={a} look="prt-link" onClick={() => { router.go({ screen: 'thread', project, agent: a }); }}>{a}</Press>
            ))}
          </p>
        </>
      )}
    </div>
  );
}

function CheckRow({ project, url, c, tag, forge, agent }: { project: string; url: string; c: PrCheck; tag: string; forge: string; agent: string | undefined }) {
  const store = useStore();
  const router = useRouter();
  const log = usePrCheckLog(project, url, c.name);
  const [asked, setAsked] = useState(false);
  // his one click fills the composer once, whatever re-renders after
  const filled = useRef(false);
  const reading = asked && log === null;
  const mark = checkMark(c.state);
  // his click asked the log: once it comes (or says why not), the composer gets the ask, never sent
  useEffect(() => {
    if (!asked || filled.current || log === null || agent === undefined) return;
    filled.current = true;
    const target = `${project}/${agent}`;
    putTextInDraft(keptStorage, target, askFixText(c.name, tag, log.lines ?? []));
    askCaret(target);
    router.go({ screen: 'thread', project, agent });
  }, [asked, log, agent, project, c.name, tag, router]);
  return (
    <li data-testid={`prtab.check.${c.name}`} data-state={c.state}>
      <span className={`prt-mark prt-${c.state}`} {...tipAttrs(mark.tip)}>{mark.glyph}</span>
      {c.url == null ? <span>{c.name}</span> : (
        <Press look="prt-link" onClick={() => { if (c.url != null) openOut(c.url); }}>{c.name}</Press>
      )}
      {c.state === 'fail' && agent !== undefined && (
        reading ? <span className="prt-wait">{panel.prtab_ask_fix_reading}</span> : (
          <Button size="sm" kind="ghost" onClick={() => { filled.current = false; setAsked(true); store.askPrCheckLog(project, url, c.name); }} testId={`prtab.fix.${c.name}`}>
            {panel.prtab_ask_fix({ agent })}
          </Button>
        )
      )}
      {!reading && log?.err != null && <span className="prt-none">{errWords(log.err, forge, log.why)}</span>}
    </li>
  );
}

function Changes({ project, url, lead }: { project: string; url: string; lead: ReactNode }) {
  const store = useStore();
  const router = useRouter();
  const diff = usePrDiff(project, url);
  const failed = useDiffError(prDiffKey(project, url));
  // its guide (page review-guided): the hub keys a PR by its canonical link
  const target = useMemo((): GuideTarget => ({ kind: 'pr', url }), [url]);
  useEffect(() => {
    store.askPrDiff(project, url);
  }, [store, project, url]);
  return (
    <DiffFiles project={project} diff={diff} loading={panel.prtab_loading_changes} error={failed}
      onRetry={() => { store.askPrDiff(project, url); }} none={panel.prtab_no_changes}
      to={diff?.agent ?? ''} from={url} onEscape={() => { router.back(); }} testId="prtab.changes" guide={target} lead={lead} />
  );
}
