// The ui/* protocol's pure parts: what is a message, what a link or a download is, what the host
// announces (C7: never sampling), the context's style variables and its changes.
import { describe, expect, it } from 'vitest';
import { contextChange, hostContext, styleVariables } from './context';
import { downloadsOf, hostCapabilities, linkOf, readMsg, textBlocks } from './protocol';

describe('readMsg', () => {
  it('reads requests, notifications and responses; anything else is null', () => {
    expect(readMsg({ jsonrpc: '2.0', id: 1, method: 'tools/call', params: { name: 'bump' } })).toEqual({ kind: 'request', id: 1, method: 'tools/call', params: { name: 'bump' } });
    expect(readMsg({ jsonrpc: '2.0', method: 'ui/notifications/initialized' })).toEqual({ kind: 'notification', method: 'ui/notifications/initialized', params: {} });
    expect(readMsg({ jsonrpc: '2.0', id: 't1', result: {} })).toEqual({ kind: 'response', id: 't1', result: {} });
    expect(readMsg({ jsonrpc: '2.0', id: 2, error: { code: -1, message: 'no' } })).toEqual({ kind: 'response', id: 2, error: { code: -1, message: 'no' } });
    expect(readMsg({ jsonrpc: '1.0', id: 1, method: 'x' })).toBeNull();
    expect(readMsg({ jsonrpc: '2.0', method: 'x', params: [1] })).toBeNull();
    expect(readMsg({ jsonrpc: '2.0' })).toBeNull();
    expect(readMsg('{"jsonrpc":"2.0","method":"x"}')).toBeNull();
    expect(readMsg(null)).toBeNull();
  });
});

describe('what a view may ask', () => {
  it('opens http, https and mailto links only', () => {
    expect(linkOf({ url: 'https://example.com/a' })).toBe('https://example.com/a');
    expect(linkOf({ url: 'mailto:a@b.c' })).toBe('mailto:a@b.c');
    expect(linkOf({ url: 'javascript:alert(1)' })).toBeNull();
    expect(linkOf({ url: 'file:///etc/passwd' })).toBeNull();
    expect(linkOf({ url: '/relative' })).toBeNull();
    expect(linkOf({})).toBeNull();
  });

  it("reads a download's embedded resources: name, type, bytes or text", () => {
    expect(
      downloadsOf({
        contents: [
          { type: 'resource', resource: { uri: 'file:///score.mid', mimeType: 'audio/midi', blob: 'TVRoZA==' } },
          { type: 'resource', resource: { uri: 'ui://x/notes.txt', text: 'hi' } },
          { type: 'text', text: 'not a file' },
        ],
      }),
    ).toEqual([
      { name: 'score.mid', mime: 'audio/midi', base64: 'TVRoZA==' },
      { name: 'notes.txt', mime: 'application/octet-stream', text: 'hi' },
    ]);
    expect(downloadsOf({})).toEqual([]);
  });

  it('keeps the text blocks of a message only', () => {
    expect(textBlocks([{ type: 'text', text: 'make it major' }, { type: 'image', data: 'x' }, { type: 'text' }])).toEqual([{ type: 'text', text: 'make it major' }]);
    expect(textBlocks('text')).toEqual([]);
  });
});

describe('what the host announces', () => {
  it('links, downloads, its server, logging, text messages and context, its sandbox; never sampling', () => {
    const c = hostCapabilities({ perms: ['microphone'], csp: { connectDomains: ['https://api.open-meteo.com'] } });
    expect(Object.keys(c).sort()).toEqual(['downloadFile', 'logging', 'message', 'openLinks', 'sandbox', 'serverResources', 'serverTools', 'updateModelContext']);
    expect(c['sandbox']).toEqual({ permissions: { microphone: {} }, csp: { connectDomains: ['https://api.open-meteo.com'] } });
    expect(c['sampling']).toBeUndefined();
  });

  it('maps our tokens on the spec variables, with their computed values, empty ones left out', () => {
    const vars = styleVariables((t) => ({ '--c-text': ' #eee ', '--font-ui': '-apple-system' })[t] ?? '');
    expect(vars).toEqual({ '--color-text-primary': '#eee', '--font-sans': '-apple-system' });
  });

  it('sends the line heights of our type styles, read out of their font shorthand', () => {
    const fonts: Record<string, string> = { '--f-meta': ' .75rem/16px -apple-system, sans-serif', '--f-ui': '.8125rem / 18px -apple-system' };
    const vars = styleVariables((t) => fonts[t] ?? '');
    expect(vars).toEqual({ '--font-text-sm-line-height': '16px', '--font-text-md-line-height': '18px', '--font-heading-xs-line-height': '18px' });
  });

  it('a context: desktop, both modes, the tool; a change says only the keys that moved', () => {
    const base = { theme: 'dark' as const, variables: {}, mode: 'inline' as const, width: 600, height: 800, locale: 'fr-FR', timeZone: 'Europe/Paris', tool: { name: 'show_counter' } };
    const a = hostContext(base);
    expect(a).toMatchObject({ platform: 'desktop', availableDisplayModes: ['inline', 'fullscreen'], displayMode: 'inline', containerDimensions: { width: 600, maxHeight: 800 }, toolInfo: { tool: { name: 'show_counter' } } });
    const b = hostContext({ ...base, theme: 'light' });
    expect(contextChange(a, b)).toEqual({ theme: 'light' });
    expect(contextChange(a, a)).toBeNull();
  });
});
