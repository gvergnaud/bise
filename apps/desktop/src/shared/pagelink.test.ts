import { describe, expect, it } from 'vitest';
import { isPagesFrame, pageLinkOf, pagesArtifact } from './pagelink';

const S = 'http://127.0.0.1:47438';

describe('isPagesFrame (an artifact in the window, amb-kit m_11871)', () => {
  it('the local pages server, any port and path', () => {
    expect(isPagesFrame(`${S}/p/desktop-app`)).toBe(true);
    expect(isPagesFrame('http://127.0.0.1:9/p/x#item')).toBe(true);
  });
  it('anything else stays out: https, another host, localhost, credentials, file, junk', () => {
    expect(isPagesFrame('https://127.0.0.1:47438/p/x')).toBe(false);
    expect(isPagesFrame('http://example.com/p/x')).toBe(false);
    expect(isPagesFrame('http://localhost:47438/p/x')).toBe(false);
    expect(isPagesFrame('http://127.0.0.1.evil.com/p/x')).toBe(false);
    expect(isPagesFrame('http://u:p@127.0.0.1:47438/p/x')).toBe(false);
    expect(isPagesFrame('file:///etc/passwd')).toBe(false);
    expect(isPagesFrame('not a url')).toBe(false);
  });
});

describe('pagesArtifact', () => {
  it("the pages server's artifact and page routes", () => {
    expect(pagesArtifact(`${S}/a/sign/2/index.html?theme=light`)).toEqual({ id: 'sign', v: 2 });
    expect(pagesArtifact(`${S}/a/deck/view`)).toEqual({ id: 'deck' });
    expect(pagesArtifact(`${S}/p/plan`)).toEqual({ id: 'plan' });
    expect(pagesArtifact(`${S}/p/plan/v/4`)).toEqual({ id: 'plan', v: 4 });
  });
  it('the kit, junk, another host: none', () => {
    expect(pagesArtifact(`${S}/kit/kit.css`)).toBeNull();
    expect(pagesArtifact(`${S}/`)).toBeNull();
    expect(pagesArtifact(`${S}/a/Bad_Id/1/x`)).toBeNull();
    expect(pagesArtifact('https://example.com/p/plan')).toBeNull();
  });
});

describe('pageLinkOf (panel-page-blank: a link to his hub\'s page opens it in the panel, architect m_65009)', () => {
  const projects = [
    { project: 'harness', pages_url: 'http://127.0.0.1:7841/p' },
    { project: 'dashboard', pages_url: 'http://127.0.0.1:47520/p' },
    { project: 'quiet' },
    { project: 'down', pages_url: null },
  ];
  const cases: [string, string, unknown][] = [
    ["his bug: dashboard's page, from dashboard's thread", 'http://127.0.0.1:47520/p/vibebot-sprint-critique', { project: 'dashboard', id: 'vibebot-sprint-critique' }],
    ["another project's port", 'http://127.0.0.1:7841/p/weekly-update', { project: 'harness', id: 'weekly-update' }],
    ['a version', 'http://127.0.0.1:47520/p/plan/v/3', { project: 'dashboard', id: 'plan', v: 3 }],
    ['/a/ with a sub-path', 'http://127.0.0.1:7841/a/deck/2/slides/index.html?theme=dark', { project: 'harness', id: 'deck', v: 2 }],
    ['/a/<id>/view', 'http://127.0.0.1:7841/a/deck/view', { project: 'harness', id: 'deck' }],
    ['an unknown path on a bise server: the web tab\'s rule', 'http://127.0.0.1:47520/kit/kit.css', null],
    ['a port no project serves', 'http://127.0.0.1:5173/p/plan', null],
    ['localhost is not the pages server', 'http://localhost:47520/p/plan', null],
    ['the web', 'https://example.com/p/plan', null],
    ['credentials', 'http://u:p@127.0.0.1:47520/p/plan', null],
  ];
  it.each(cases)('%s', (_name, url, want) => {
    expect(pageLinkOf(url, projects)).toEqual(want);
  });
  it('no project holds a pages server: none', () => {
    expect(pageLinkOf('http://127.0.0.1:47520/p/plan', [{ project: 'dashboard' }])).toBeNull();
  });
});
