// A plugin's settings (its manifest's dev.bise.settings: an API key), the 'Settings' group of its
// page (designer m_51514): one row per setting, the plugin's label and its help as the dim line;
// at the end 'not set' + Set…, 'set' + Change… and Forget, or '? needed to work' in the accent for
// a required one with no value (it waits on him, like a login). Set… opens the row's field (a
// password field for a secret), ⏎ saves, esc cancels; a save sends `plugin_setting` and reads
// 'applies from the next call' for 4 s. The value is never shown back, not even a non-secret one
// (architect m_51451: never on the wire), and the variable's name never shows here. Owner:
// gallery-lead.
import { useEffect, useRef, useState, type KeyboardEvent } from 'react';
import { BannerWait, Button, GroupLabel, Input, sentenceCase } from '@/ds';
import type { AppCmd, PluginSetting } from '@/proto';
import { settings } from '@/proto/words.gen';

/** How long a saved row says 'applies from the next call' before 'set' again. */
export const APPLIES_MS = 4000;

/** What a row's end says. Pure. */
export function stateOf(s: PluginSetting): 'set' | 'needed' | 'not_set' {
  if (s.set) return 'set';
  return s.required === true ? 'needed' : 'not_set';
}

type Mode = { kind: 'idle' } | { kind: 'edit' } | { kind: 'forget' } | { kind: 'saved' };

function SettingRow({ s, testId, save }: { s: PluginSetting; testId: string; save: (value: string | null) => void }) {
  const [mode, setMode] = useState<Mode>({ kind: 'idle' });
  const [value, setValue] = useState('');
  const field = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (mode.kind === 'edit') field.current?.focus();
    if (mode.kind !== 'saved') return;
    const t = window.setTimeout(() => { setMode({ kind: 'idle' }); }, APPLIES_MS);
    return () => { window.clearTimeout(t); };
  }, [mode.kind]);
  const close = () => { setValue(''); setMode({ kind: 'idle' }); };
  const submit = () => {
    if (value.trim() === '') return;
    save(value);
    setValue('');
    setMode({ kind: 'saved' });
  };
  const onKey = (e: KeyboardEvent) => {
    if (e.key === 'Enter') { e.preventDefault(); submit(); }
    if (e.key === 'Escape') { e.preventDefault(); close(); }
  };
  const st = stateOf(s);
  return (
    <>
      <div className="plg-row part" data-testid={testId} data-state={st}>
        <span className="plg-lead" aria-hidden="true" />
        <span className="plg-row-words">
          <span className="plg-row-name"><span className="plg-title">{s.label}</span></span>
          {s.help != null && <span className="plg-row-what">{s.help}</span>}
        </span>
        <span className="plg-row-end">
          {mode.kind === 'saved' ? (
            <span className="plg-dim" data-testid={`${testId}.applies`}>{settings.plugins_setting_applies}</span>
          ) : mode.kind === 'idle' && (
            <>
              {st === 'needed' && <span data-testid={`${testId}.needed`}><BannerWait>?</BannerWait>{` ${settings.plugins_setting_needed}`}</span>}
              {st === 'not_set' && <span className="plg-dim" data-testid={`${testId}.not_set`}>{settings.plugins_setting_not_set}</span>}
              {st === 'set' && <span className="plg-dim" data-testid={`${testId}.set`}>{settings.plugins_setting_set}</span>}
              <Button size="sm" onClick={() => { setMode({ kind: 'edit' }); }} testId={`${testId}.edit`}>
                {s.set ? settings.plugins_setting_change : settings.plugins_setting_set_button}
              </Button>
              {s.set && <Button size="sm" kind="ghost" onClick={() => { setMode({ kind: 'forget' }); }} testId={`${testId}.forget`}>{settings.plugins_setting_forget}</Button>}
            </>
          )}
        </span>
      </div>
      {mode.kind === 'edit' && (
        <div className="plg-row part plg-setting-field" data-testid={`${testId}.field`}>
          <span className="plg-lead" aria-hidden="true" />
          <Input ref={field} look="plg-setting-input" type={s.secret === true ? 'password' : 'text'} value={value}
            aria-label={s.label} placeholder={s.placeholder ?? ''} autoComplete="off" spellCheck={false}
            onChange={(e) => { setValue(e.target.value); }} onKeyDown={onKey} data-testid={`${testId}.input`} />
          <span className="plg-acts">
            <Button kind="pri" size="sm" disabled={value.trim() === ''} onClick={submit} testId={`${testId}.save`}>{settings.plugins_setting_save}</Button>
            <Button kind="ghost" size="sm" onClick={close} testId={`${testId}.cancel`}>{settings.plugins_setting_cancel}</Button>
          </span>
        </div>
      )}
      {mode.kind === 'forget' && (
        <div className="plg-ask" role="group" data-testid={`${testId}.ask`}>
          <span>{settings.plugins_setting_forget_ask({ label: s.label.toLowerCase() })}</span>
          <span className="plg-acts">
            <Button kind="danger" size="sm" onClick={() => { save(null); close(); }} testId={`${testId}.ask.yes`}>{settings.plugins_setting_forget}</Button>
            <Button kind="ghost" size="sm" onClick={close} testId={`${testId}.ask.no`}>{settings.plugins_setting_keep}</Button>
          </span>
        </div>
      )}
    </>
  );
}

/** The group: nothing when the plugin declares no setting. */
export function SettingsGroup({ plugin, list, project, send }: { plugin: string; list: readonly PluginSetting[]; project?: string; send: (cmd: AppCmd) => void }) {
  if (list.length === 0) return null;
  const at = project === undefined ? {} : { project };
  return (
    <div className="plg-group" data-testid={`plugins.settings.${plugin}`}>
      <GroupLabel count={list.length}>{sentenceCase(settings.plugins_settings_head)}</GroupLabel>
      <div className="plg-list">
        {list.map((s) => (
          <SettingRow key={s.var} s={s} testId={`plugins.setting.${plugin}.${s.var}`}
            save={(value) => { send({ cmd: 'plugin_setting', ...at, name: plugin, var: s.var, ...(value === null ? {} : { value }) }); }} />
        ))}
      </div>
    </div>
  );
}
