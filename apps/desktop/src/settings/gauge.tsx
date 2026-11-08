// settings · Setup's gauge words and parts (page models-setup, then models-settings pick A): the
// features' names, a fix's button, why a feature failed, the report Copy report copies, dictation's
// Try it and a failed row's troubleshooting. Everything comes from the core's setup_gauge
// (bise_catalog::features): on, failed or off, the fix, Next and its order are the core's; this
// file only words and draws them (one owner, architect m_53985). The list that shows them is
// roles.tsx. A test is a real call: only on his click (Test everything, Test again) or when a role
// changes (the core's), never at open. Owner: setup-page.
import { useState, type ReactNode } from 'react';
import type { FeatureId, FeaturePart, FeatureRow, Perm, RoleId, SetupFix, SetupGauge } from '@/proto';
import { Act, Button } from '@/ds';
import { settings } from '@/proto/words.gen';
import { article } from './keycheck';

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

/** A key of `provider` is there and failed (refused, expired): its fix replaces it. Pure. */
export const keyFailed = (g: SetupGauge, provider: string): boolean =>
  g.rows.some((r) => r.parts.some((p) => p.model.split('/')[0] === provider && p.state === 'failed' && p.why === 'wrong_key'));

/** A permission's name for people. */
export const PERM_NAME: Record<Perm, string> = { mic: settings.onb_perm_mic, accessibility: settings.onb_perm_accessibility, screen: settings.onb_perm_screen, unknown: '' };

/** A fix's button words. Pure. */
export function fixButton(f: SetupFix, g: SetupGauge): string {
  switch (f.t) {
    case 'add_key': {
      const provider = providerName(g, f.provider);
      return keyFailed(g, f.provider) ? settings.fix_replace_key({ provider }) : settings.fix_add_key({ a: article(provider), provider });
    }
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
  copy?: (text: string) => void;
  /** dictation's Try it: its own path (the store's dictateStart / dictateStop and useDictation);
   * none: no Try it (no core mic in this run) */
  tryIt?: TryPort;
}

/** What Try it needs: start the mic (an id), stop it, and read that dictation's words. */
export interface TryPort {
  start: () => string;
  stop: (id: string) => void;
  useHeard: (id: string | null) => { readonly text: string; readonly final: boolean; readonly error: string | null } | null;
}

/** Try it lasts this long: then the mic stops and the words come (page models-setup: 3 s). */
export const TRY_MS = 3000;

/** Dictation's Try it (his click only): 3 s of mic through dictation's own path, then what it
 * heard. It owns the row's mark (designer m_54627): while it listens no ✓ (nothing is done yet) and
 * the instruction in the text colour; heard words bring the ✓ back; nothing heard is ✗ in red. */
export function TryIt({ port, mark }: { port: TryPort; mark: ReactNode }) {
  const [id, setId] = useState<string | null>(null);
  const heard = port.useHeard(id);
  const listening = id !== null && heard !== null && !heard.final;
  const nothing = id !== null && heard?.final === true && (heard.error !== null || heard.text.trim() === '');
  const words = id !== null && heard?.final === true && !nothing ? heard.text.trim() : null;
  // Try it before the mark: the mark keeps the status column's right edge, like every row's
  return (
    <>
      <span className="st-try" data-busy={id !== null || undefined}>
        {words !== null && (
          <span className="st-heard" data-testid="settings.feature.dictation.heard">
            {settings.try_heard({ text: words })}
          </span>
        )}
        <Act
          disabled={listening}
          onClick={(e) => {
            e.stopPropagation();
            const d = port.start();
            setId(d);
            setTimeout(() => {
              port.stop(d);
            }, TRY_MS);
          }}
          testId="settings.feature.dictation.try"
        >
          {settings.try_it}
        </Act>
      </span>
      {listening ? (
        <span className="st-listen" data-testid="settings.feature.dictation.heard">
          {settings.try_listening}
        </span>
      ) : nothing ? (
        <span className="st-mark err" data-testid="settings.feature.dictation.heard">
          {heard.error ?? settings.try_nothing}
        </span>
      ) : (
        mark
      )}
    </>
  );
}

/** A failed feature's troubleshooting: why, the provider's own words, when, the fix, Copy report,
 * Test again (another model: the row's own button). A neutral surface with a 2 px red rule
 * (designer). */
export function Trouble({ r, g, ports }: { r: FeatureRow; g: SetupGauge; ports: GaugePorts }) {
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
