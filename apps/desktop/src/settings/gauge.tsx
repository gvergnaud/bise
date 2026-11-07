// settings · Setup (page models-setup, Gabriel's pick B): the gauge (N/7), Next, Test everything,
// the features with their marks, and a failed feature's troubleshooting. Everything it shows comes
// from the core's setup_gauge (bise_catalog::features): on, failed or off, the fix, Next and its
// order are the core's; this file only words and draws them (one owner, architect m_53985). A test
// is a real call: only on his click (Test everything, Test again), never at open. Owner: setup-page.
import { useState } from 'react';
import type { FeatureId, FeaturePart, FeatureRow, Perm, RoleId, SetupFix, SetupGauge } from '@/proto';
import { Act, Button, Press, tipAttrs } from '@/ds';
import { settings } from '@/proto/words.gen';
import { cx } from '@/ds/cx';

/** A feature's name. */
export const FEATURE_WORDS: Record<FeatureId, string> = {
  talk: settings.feat_talk,
  agents: settings.feat_agents,
  small: settings.feat_small,
  vision: settings.feat_vision,
  dictation: settings.feat_dictation,
  voice_mode: settings.feat_voice_mode,
  approvals: settings.feat_approvals,
  unknown: '',
};

/** A duration in seconds, one decimal ("0.6"). Pure. */
export const secsOf = (ms: number): string => (Math.round(ms / 100) / 10).toFixed(1);

/** A provider's name for people: a part's (its model's provider), else the id. Pure. */
export function providerName(g: SetupGauge, id: string): string {
  for (const r of g.rows) for (const p of r.parts) if (p.model.split('/')[0] === id && p.provider !== undefined && p.provider !== '') return p.provider;
  return id.length > 0 ? id.charAt(0).toUpperCase() + id.slice(1) : id;
}

/** The parts that failed, then the rest (the troubleshooting speaks of the first). */
const firstFailed = (r: FeatureRow): FeaturePart | undefined => r.parts.find((p) => p.state === 'failed');

/** The slowest test of a feature's parts (its check's time), none untested. Pure. */
export function tookMs(r: FeatureRow): number | null {
  const ms = r.parts.map((p) => p.ms).filter((m): m is number => typeof m === 'number');
  return ms.length === 0 ? null : Math.max(...ms);
}

export type Tone = 'ink' | 'err' | 'faint';

/** A feature's mark: ✓ (and how long its check took) in ink, ✗ failed in red, ○ off faint; voice
 * mode's tip says its voice has no test of its own yet (architect m_54103). Pure. */
export function markOf(r: FeatureRow, g: SetupGauge): { text: string; tone: Tone; tip: string } {
  if (r.state === 'failed') return { text: settings.mark_failed, tone: 'err', tip: whyWords(r, g) };
  if (r.state !== 'on') {
    const main = r.parts[0];
    const tip = r.id === 'vision' && main !== undefined && r.fix?.t === 'pick_model' ? settings.tip_no_images({ model: main.model }) : settings.tip_off;
    return { text: settings.mark_off, tone: 'faint', tip };
  }
  const ms = tookMs(r);
  // voice mode: the same mark as every row; its tip says only the key is checked (architect m_54103)
  const tts = r.id === 'voice_mode' ? r.parts.find((p) => p.role === 'tts') : undefined;
  if (tts !== undefined) return { text: ms === null ? settings.mark_on : settings.mark_took({ secs: secsOf(ms) }), tone: 'ink', tip: settings.tip_key_only({ provider: tts.provider ?? '' }) };
  const better = r.better != null && r.better.t === 'add_key' ? settings.tip_better({ provider: providerName(g, r.better.provider) }) : null;
  if (ms === null) return { text: settings.mark_on, tone: 'ink', tip: better ?? settings.tip_untested };
  return { text: settings.mark_took({ secs: secsOf(ms) }), tone: 'ink', tip: better ?? settings.tip_took({ secs: secsOf(ms) }) };
}

const PERM_NAME: Record<Perm, string> = { mic: settings.onb_perm_mic, accessibility: settings.onb_perm_accessibility, screen: settings.onb_perm_screen, unknown: '' };

/** A fix's button words. Pure. */
export function fixButton(f: SetupFix, g: SetupGauge): string {
  switch (f.t) {
    case 'add_key':
      return settings.fix_add_key({ provider: providerName(g, f.provider) });
    case 'sign_in':
      return settings.fix_sign_in({ provider: providerName(g, f.provider) });
    case 'credit':
      return settings.fix_credit({ provider: providerName(g, f.provider) });
    case 'pick_model':
      return settings.fix_pick_model;
    case 'remove_env':
      return settings.fix_remove_env({ var: f.var });
    case 'allow_perm':
      return f.open ? settings.open_system_settings : f.perm === 'mic' ? settings.fix_allow_mic : settings.fix_allow({ perm: PERM_NAME[f.perm] });
    case 'restart':
      return settings.fix_restart;
    case 'turn_on':
      return settings.fix_turn_on;
    case 'test_again':
    default:
      return settings.fix_test_again;
  }
}

/** Next's words: the action, then what it brings back or turns on. Pure. */
export function nextWords(g: SetupGauge): string {
  const n = g.next;
  if (n == null) return settings.gauge_all_set;
  const f = n.fix;
  const act =
    f.t === 'add_key'
      ? settings.next_add_key({ provider: providerName(g, f.provider) })
      : f.t === 'sign_in'
        ? settings.next_sign_in({ provider: providerName(g, f.provider) })
        : f.t === 'credit'
          ? settings.next_credit({ provider: providerName(g, f.provider) })
          : f.t === 'pick_model'
            ? settings.next_pick_model
            : f.t === 'remove_env'
              ? settings.next_remove_env({ var: f.var })
              : f.t === 'allow_perm'
                ? f.perm === 'mic'
                  ? settings.next_allow_mic
                  : settings.next_allow({ perm: PERM_NAME[f.perm] })
                : f.t === 'restart'
                  ? settings.next_restart
                  : f.t === 'turn_on'
                    ? settings.next_turn_on({ what: FEATURE_WORDS.approvals.toLowerCase() })
                    : settings.next_test_again;
  const what = n.unlocks.map((id) => FEATURE_WORDS[id].toLowerCase()).join(', ');
  const failed = n.unlocks.some((id) => g.rows.find((r) => r.id === id)?.state === 'failed');
  return `${act} ${failed ? settings.gauge_brings_back({ what }) : settings.gauge_turns_on({ what })}`;
}

/** Why a feature failed, from its verdict (keycheck::Why), in plain words. Pure. */
export function whyWords(r: FeatureRow, g: SetupGauge): string {
  if (r.home_gone != null) return settings.check_home_gone({ path: r.home_gone });
  const p = firstFailed(r);
  if (p === undefined) return r.perms?.some((x) => x.grant === 'denied') === true ? settings.why_mic : settings.why_off;
  const provider = p.provider ?? providerName(g, p.model.split('/')[0] ?? '');
  switch (p.why) {
    case 'no_key':
      return settings.why_no_key({ provider });
    case 'wrong_key':
      return settings.why_wrong_key({ provider });
    case 'no_credit':
      return settings.why_no_credit({ provider });
    case 'model':
      return settings.why_model({ model: p.model });
    case 'no_access':
      return settings.why_no_access({ model: p.model });
    case 'config':
      return settings.why_config({ provider });
    case 'unreachable':
    default:
      return settings.why_unreachable({ provider });
  }
}

/** The features of other providers that still work beside a failed one ('listening still works:
 * it goes through Groq'): the other parts of this feature that are on. Pure. */
export function stillWorks(r: FeatureRow): string | null {
  const bad = firstFailed(r);
  if (bad === undefined) return null;
  const ok = r.parts.find((p) => p.state === 'on' && p.provider !== undefined && p.provider !== bad.provider && p.role !== 'main');
  if (ok === undefined) return null;
  return settings.trouble_still({ what: ROLE_DOES[ok.role] ?? ok.role, provider: ok.provider ?? '' });
}

const ROLE_DOES: Partial<Record<RoleId, string>> = { voice: settings.feat_dictation, tts: settings.feat_voice_mode };

/** The report Copy report copies: each feature, its state, its models and the providers' own words;
 * never a key nor a token (none is in the gauge). Pure. */
export function reportOf(g: SetupGauge): string {
  const lines = [`bise setup ${settings.gauge_count({ on: String(g.on), of: String(g.of) })}`];
  for (const r of g.rows) {
    lines.push(`${FEATURE_WORDS[r.id]}: ${r.state}`);
    for (const p of r.parts) {
      const bits = [p.role, p.model || '-', p.source, p.var ?? '', p.state, p.why ?? '', p.ms != null ? `${String(p.ms)} ms` : '', p.said ?? ''].filter((b) => b !== '');
      lines.push(`  ${bits.join(' · ')}`);
    }
  }
  return lines.join('\n');
}

/** What a fix does when he clicks it (the page's ports). */
export interface GaugePorts {
  fix: (f: SetupFix, row: FeatureId | null) => void;
  /** setup_test: every model (none), or these */
  test: (models?: readonly string[]) => void;
  /** open the roles table (another way) */
  roles: () => void;
  copy?: (text: string) => void;
}

/** The gauge's ring, 40 px, in ink: on/of. */
function GaugeRing({ on, of }: { on: number; of: number }) {
  const r = 17;
  const around = 2 * Math.PI * r;
  const len = of === 0 ? 0 : (on / of) * around;
  return (
    <svg className="st-ring" viewBox="0 0 40 40" width="40" height="40" aria-hidden="true">
      <circle className="track" cx="20" cy="20" r={r} />
      {len > 0 && <circle className="used" cx="20" cy="20" r={r} strokeDasharray={`${len.toFixed(2)} ${(around - len).toFixed(2)}`} transform="rotate(-90 20 20)" />}
    </svg>
  );
}

export function GaugeHead({ g, ports, onNext }: { g: SetupGauge; ports: GaugePorts; onNext: (id: FeatureId) => void }) {
  const testing = (g.testing?.length ?? 0) > 0;
  const n = g.next;
  const first = n?.unlocks[0];
  return (
    <div className="st-head" data-testid="settings.gauge">
      <span className="st-count" {...tipAttrs(settings.gauge_tip({ on: String(g.on), of: String(g.of) }), undefined, 'below')} data-testid="settings.gauge.count">
        <GaugeRing on={g.on} of={g.of} />
        <span className="n">{settings.gauge_count({ on: String(g.on), of: String(g.of) })}</span>
      </span>
      {/* one primary per screen (designer m_54523): Next is a link to its row, whose panel holds the fix */}
      {first !== undefined ? (
        <Press look="st-next" onClick={() => { onNext(first); }} data-testid="settings.gauge.next">
          <b>{settings.gauge_next}</b> {nextWords(g)} ›
        </Press>
      ) : (
        <span className="st-next" data-testid="settings.gauge.next">
          {nextWords(g)}
        </span>
      )}
      <Button onClick={() => { ports.test(); }} disabled={testing} testId="settings.gauge.test">
        {testing ? settings.gauge_testing : settings.gauge_test_all}
      </Button>
    </div>
  );
}

/** The gauge and its features: Next opens its row's troubleshooting and scrolls to it. */
export function GaugeView({ g, ports, at }: { g: SetupGauge; ports: GaugePorts; at?: FeatureId | null }) {
  const [shut, setShut] = useState<ReadonlySet<FeatureId>>(new Set());
  return (
    <>
      <GaugeHead
        g={g}
        ports={ports}
        onNext={(id) => {
          const next = new Set(shut);
          next.delete(id);
          setShut(next);
          document.querySelector(`[data-testid="settings.feature.${id}"]`)?.scrollIntoView({ block: 'nearest' });
        }}
      />
      <FeatureRows g={g} ports={ports} at={at ?? null} shut={shut} setShut={setShut} />
    </>
  );
}

/** A feature's sub line: its models and where they come from, its permissions. */
function subOf(r: FeatureRow): string {
  const seen = new Set<string>();
  const models = r.parts
    .filter((p) => p.model !== '' && !seen.has(p.model) && seen.add(p.model))
    .map((p) => (p.var != null ? `${p.model} · ${settings.part_env({ var: p.var })}` : p.model));
  const perms = (r.perms ?? []).map((p) => `${PERM_NAME[p.perm]} ${p.grant === 'allowed' ? settings.perm_allowed : p.grant === 'denied' ? settings.perm_refused : settings.perm_not_asked}`);
  return [...models, ...perms].join(' · ');
}

/** The features; a failed one opens its troubleshooting unless he closed it (`shut`). */
export function FeatureRows({ g, ports, at, shut, setShut }: { g: SetupGauge; ports: GaugePorts; at?: FeatureId | null; shut: ReadonlySet<FeatureId>; setShut: (s: ReadonlySet<FeatureId>) => void }) {
  return (
    <div className="st-rows" data-testid="settings.features">
      {g.rows.map((r) => {
        const m = markOf(r, g);
        const open = r.state === 'failed' && !shut.has(r.id);
        const testing = r.parts.some((p) => g.testing?.includes(p.model) === true);
        return (
          <div key={r.id} className={cx('st-feature', at === r.id && 'is-at')} data-state={r.state} data-testid={`settings.feature.${r.id}`}>
            <div
              className="st-row"
              role={r.state === 'failed' ? 'button' : undefined}
              onClick={
                r.state === 'failed'
                  ? () => {
                      const next = new Set(shut);
                      if (open) next.add(r.id);
                      else next.delete(r.id);
                      setShut(next);
                    }
                  : undefined
              }
            >
              <span className="st-name">
                {FEATURE_WORDS[r.id]}
                <span className="st-sub">{subOf(r)}</span>
              </span>
              <span className={cx('st-mark', testing ? 'faint' : m.tone)} {...tipAttrs(m.tip)} data-testid={`settings.feature.${r.id}.mark`}>
                {testing ? settings.gauge_testing : m.text}
              </span>
              {r.fix != null && !open && (
                <Button
                  kind={r.state === 'failed' ? 'pri' : undefined}
                  onClick={() => { if (r.fix != null) ports.fix(r.fix, r.id); }}
                  testId={`settings.feature.${r.id}.fix`}
                >
                  {fixButton(r.fix, g)}
                </Button>
              )}
              {r.fix === undefined && r.better != null && r.better.t === 'allow_perm' && (
                <Act onClick={() => { if (r.better != null) ports.fix(r.better, r.id); }} testId={`settings.feature.${r.id}.better`}>
                  {fixButton(r.better, g)}
                </Act>
              )}
            </div>
            {open && <Trouble r={r} g={g} ports={ports} />}
          </div>
        );
      })}
    </div>
  );
}

/** A failed feature's troubleshooting: why, the provider's own words, when, the fix, another way,
 * Copy report, Test again. A neutral surface with a 2 px red rule (designer). */
function Trouble({ r, g, ports }: { r: FeatureRow; g: SetupGauge; ports: GaugePorts }) {
  const [copied, setCopied] = useState(false);
  const p = firstFailed(r);
  const still = stillWorks(r);
  const env = r.parts.find((x) => x.var != null && x.state === 'failed')?.var;
  const at = p?.tested_at;
  const models = r.parts.filter((x) => x.model !== '').map((x) => x.model);
  return (
    <div className="st-trouble" data-testid={`settings.feature.${r.id}.trouble`}>
      <p className="why">
        {whyWords(r, g)}
        {still !== null && ` ${still}`}
      </p>
      {p?.said != null && <p className="said mono">{settings.trouble_said({ said: p.said })}</p>}
      {env != null && <p className="said">{settings.trouble_env({ var: env })}</p>}
      {at != null && <p className="when">{settings.trouble_checked({ time: new Date(at).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' }) })}</p>}
      <div className="acts">
        {r.fix != null && (
          <Button kind="pri" onClick={() => { if (r.fix != null) ports.fix(r.fix, r.id); }} testId={`settings.feature.${r.id}.trouble.fix`}>
            {fixButton(r.fix, g)}
          </Button>
        )}
        {r.parts.length > 0 && r.fix?.t !== 'pick_model' && (
          <Button kind="ghost" onClick={ports.roles} testId={`settings.feature.${r.id}.trouble.other`}>
            {settings.trouble_other}
          </Button>
        )}
        {ports.copy !== undefined && (
          <Button
            kind="ghost"
            onClick={() => {
              ports.copy?.(reportOf(g));
              setCopied(true);
            }}
            testId={`settings.feature.${r.id}.trouble.copy`}
          >
            {copied ? settings.trouble_copied : settings.trouble_copy}
          </Button>
        )}
        <Button kind="ghost" onClick={() => { ports.test(models); }} testId={`settings.feature.${r.id}.trouble.again`}>
          {settings.fix_test_again}
        </Button>
      </div>
    </div>
  );
}
