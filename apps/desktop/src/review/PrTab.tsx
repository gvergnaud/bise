// A PR's tab (pr-integration A, rules 7 to 12): one pull request the hub follows, by its link. Its
// head from the hub's row only (ArtifactPr.status: title, #412, state, head → base, agents, last
// push, the status line, pink only when it waits on him); Open on GitHub ↗ always; Merge as a ghost
// button only when the forge says mergeable, at the head he sees (pr/act); Close only in its ⋯,
// after a confirm. Summary (pr/detail: description as untrusted markdown, checks with 'Ask <agent>
// to fix' that fills the composer and never sends, reviews read-only, its agents) and Changes (its
// diff, the hub's pick of place or forge). Every wait says what it waits for, with the link out
// beside a miss. Owner: pr-artifacts.
import { useEffect, useRef, useState, type MouseEvent, type ReactNode } from 'react';
import { Button, Empty, Head, MoreButton, Seg, Skel, tipAttrs, type EntityMenu } from '@/ds';
import { prDiffKey, useArtifacts, useDiffError, usePrActed, usePrCheckLog, usePrDetail, usePrDiff, useStore } from '@/store';
import type { Pr, PrCheck, PrDetail } from '@/proto';
import { panel } from '@/proto/words_panel.gen';
import { menus } from '@/proto/words_menus.gen';
import { Markdown } from '@/thread/Markdown';
import { useRouter } from '@/app/router';
import { useNow } from '@/app/clock';
import { keptStorage } from '@/app/keep';
import { askCaret } from '@/composer/caret';
import { putTextInDraft } from '@/artifacts/put';
import { prLine } from '@/artifacts/pr';
import { DiffFiles } from './DiffFiles';
import { actLine, askFixText, canClose, canMerge, checkMark, errWords, forgeWord, headFacts, verdictWord } from './prwords';
import './prtab.css';

const bise = () => (window as Partial<Window>).bise;
const openOut = (url: string) => {
  void bise()?.openExternal(url);
};

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
  const [tab, setTab] = useState(0);
  const [confirming, setConfirming] = useState(false);
  const busy = acted !== null && !acted.done;
  const now = useNow();

  const menu: EntityMenu = {
    onContextMenu: (e: MouseEvent<HTMLElement>) => {
      e.preventDefault();
      void showMenu();
    },
    open: () => {
      void showMenu();
    },
  };
  async function showMenu(): Promise<void> {
    const popup = bise()?.popupMenu;
    if (popup === undefined) return;
    const items = [
      { id: 'copy-link', label: menus.copy_link, icon: 'link' },
      ...(s === null ? [] : [{ id: 'copy-branch', label: menus.copy_branch }]),
      ...(canClose(s) && !busy ? [{ type: 'separator' as const }, { id: 'close', label: panel.prtab_close, destructive: true }] : []),
    ];
    const id = await popup(items);
    if (id === 'copy-link') void bise()?.copy(url);
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
        title={<span data-testid="prtab.title">{art?.title ?? tag}</span>}
        sub={<span data-testid="prtab.facts">{headFacts(tag, s, now).join(' · ')}</span>}
        end={
          <>
            {out}
            {canMerge(s) && !busy && (
              <Button kind="ghost" size="sm" onClick={merge} title={panel.prtab_merge_tip({ forge })} testId="prtab.merge">
                {panel.prtab_merge}
              </Button>
            )}
            <MoreButton menu={menu} testId="prtab.more" />
          </>
        }
      />
      {line !== null && line.words !== '' && (
        <p className="prt-status" data-testid="prtab.status">
          <span className={line.yours ? 'prt-yours' : undefined}>{line.words}</span>
          {line.late !== '' && <span className="prt-late" {...tipAttrs(panel.pr_as_of_tip)}>{` · ${line.late}`}</span>}
        </p>
      )}
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
        <>
          <Seg items={[panel.prtab_summary, panel.prtab_changes]} on={tab} onPick={setTab} testId="prtab.tabs" />
          {tab === 0 ? <Summary project={project} url={url} s={s} tag={tag} forge={forge} out={out} /> : <Changes project={project} url={url} />}
        </>
      )}
    </section>
  );
}

function Summary({ project, url, s, tag, forge, out }: { project: string; url: string; s: Pr | null; tag: string; forge: string; out: ReactNode }) {
  const store = useStore();
  const router = useRouter();
  const ans = usePrDetail(project, url);
  useEffect(() => {
    store.askPrDetail(project, url);
  }, [store, project, url]);
  const agents = s?.agents ?? [];
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
      <h3>{panel.prtab_description}</h3>
      {d.body.trim() === '' ? <p className="prt-none">{panel.prtab_no_description}</p> : (
        <div className="prt-md" data-testid="prtab.body"><Markdown text={d.body} onLink={openOut} /></div>
      )}
      {d.body_cut === true && <p className="prt-none">{panel.prtab_body_cut({ forge })}</p>}
      <h3>{panel.prtab_checks}</h3>
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
        <button type="button" className="prt-link" onClick={() => { openOut(url); }} data-testid="prtab.more_notes">
          {panel.prtab_more_notes({ n: d.more_notes ?? 0, forge })}
        </button>
      )}
      {agents.length > 0 && (
        <>
          <h3>{panel.prtab_agents}</h3>
          <p className="prt-agents" data-testid="prtab.agents">
            {agents.map((a) => (
              <button key={a} type="button" className="prt-link" onClick={() => { router.go({ screen: 'thread', project, agent: a }); }}>{a}</button>
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
        <button type="button" className="prt-link" onClick={() => { if (c.url != null) openOut(c.url); }}>{c.name}</button>
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

function Changes({ project, url }: { project: string; url: string }) {
  const store = useStore();
  const router = useRouter();
  const diff = usePrDiff(project, url);
  const failed = useDiffError(prDiffKey(project, url));
  useEffect(() => {
    store.askPrDiff(project, url);
  }, [store, project, url]);
  return (
    <DiffFiles project={project} diff={diff} loading={panel.prtab_loading_changes} error={failed}
      onRetry={() => { store.askPrDiff(project, url); }} none={panel.prtab_no_changes}
      to={diff?.agent ?? ''} from={url} onEscape={() => { router.back(); }} testId="prtab.changes" />
  );
}
