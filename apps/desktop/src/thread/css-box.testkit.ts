// Boxes computed from the stylesheets themselves, for the tests of where the column's things sit
// (happy-dom lays nothing out; vitest gives a css ?raw import empty, so a test reads them from
// disk): a selector's declarations, and a length of calc/min/max/var/px/% for a box of a width.
// Shared by thread/to-end.test.ts and composer/home/home-room.test.ts. Test-only.

/** The declarations of every rule of `css` whose selector list holds `sel`, in order (a later one wins). */
export function rule(css: string, sel: string): Map<string, string> {
  const blocks = css
    .replace(/\/\*[\s\S]*?\*\//gu, '')
    .split(/(?<=\})/u)
    .filter((block) => block.includes('{') && block.slice(0, block.indexOf('{')).split(',').map((s) => s.trim()).includes(sel));
  if (blocks.length === 0) throw new Error(`no rule for ${sel}`);
  const out = new Map<string, string>();
  for (const b of blocks)
    for (const d of b.slice(b.indexOf('{') + 1, b.lastIndexOf('}')).split(';')) {
      const i = d.indexOf(':');
      if (i > 0) out.set(d.slice(0, i).trim(), d.slice(i + 1).trim());
    }
  return out;
}

/**
 * A length (px) of calc/min/var/px/% for a box `w` wide, the vars given (a var not given takes its
 * fallback): enough of CSS for the column's expressions.
 */
export function length(expr: string, w: number, vars: Readonly<Record<string, string>>): number {
  const toks = expr.match(/var\(|calc\(|min\(|max\(|--[\w-]+|-?\d*\.?\d+(?:px|%)?|[-+*/(),]/gu) ?? [];
  let i = 0;
  const peek = (): string | undefined => toks[i];
  const take = (t?: string): string => {
    const v = toks[i++];
    if (v === undefined || (t !== undefined && v !== t)) throw new Error(`bad length ${expr}`);
    return v;
  };
  const atom = (): number => {
    const t = take();
    if (t === '(' || t === 'calc(') {
      const v = sum();
      take(')');
      return v;
    }
    if (t === 'min(' || t === 'max(') {
      const vs = [sum()];
      while (peek() === ',') {
        take(',');
        vs.push(sum());
      }
      take(')');
      return t === 'min(' ? Math.min(...vs) : Math.max(...vs);
    }
    if (t === 'var(') {
      const name = take();
      let fallback: number | undefined;
      if (peek() === ',') {
        take(',');
        fallback = sum();
      }
      take(')');
      const v = vars[name];
      if (v !== undefined) return length(v, w, vars);
      if (fallback === undefined) throw new Error(`no ${name}`);
      return fallback;
    }
    if (t === '-') return -atom();
    if (t.endsWith('%')) return (Number.parseFloat(t) / 100) * w;
    return Number.parseFloat(t);
  };
  const product = (): number => {
    let v = atom();
    while (peek() === '*' || peek() === '/') v = take() === '*' ? v * atom() : v / atom();
    return v;
  };
  const sum = (): number => {
    let v = product();
    while (peek() === '+' || peek() === '-') v = take() === '+' ? v + product() : v - product();
    return v;
  };
  return sum();
}

/**
 * An absolutely placed box's left and right edges (px from its containing block's left, `w` wide)
 * from its declarations: `left` and `width`, or `left`/`right`, or `inset` (one value) for what is
 * not said.
 */
export function absSpan(d: ReadonlyMap<string, string>, w: number, vars: Readonly<Record<string, string>>): { l: number; r: number } {
  const inset = d.get('inset');
  const side = (name: 'left' | 'right'): number | null => {
    const v = d.get(name) ?? inset;
    return v === undefined || v === 'auto' ? null : length(v, w, vars);
  };
  const l = side('left');
  const width = d.get('width');
  if (l !== null && width !== undefined) return { l, r: l + length(width, w, vars) };
  const r = side('right');
  if (l !== null && r !== null) return { l, r: w - r };
  throw new Error('no box: neither left and width nor left and right');
}
