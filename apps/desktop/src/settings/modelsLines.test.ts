// Settings and the terminal's /models say the same lines for the same gauge: tests/words/models_lines.json
// is read here and by rust/tui/src/onboarding/gauge_view.rs (architect m_57594).
import { describe, expect, it } from 'vitest';
import type { FeatureId, SetupGauge } from '@/proto';
import FX from '../../../../tests/words/models_lines.json';
import { fixLine, newsLine, partStatus } from './roleList';

interface Case {
  name: string;
  gauge: unknown;
  fix: string | null;
  news: [string, string] | null;
  stops: [FeatureId, number, string][];
}

describe('models_lines.json: the lines both screens share', () => {
  for (const c of FX.cases as unknown as Case[]) {
    it(c.name, () => {
      const g = c.gauge as SetupGauge;
      expect(fixLine(g)?.text ?? null).toBe(c.fix);
      const n = newsLine(g, []);
      expect(n === null ? null : [n.text, n.act]).toEqual(c.news);
      for (const [id, i, text] of c.stops) {
        const part = g.rows.find((r) => r.id === id)?.parts[i];
        expect(part, `${id} ${i}`).toBeDefined();
        if (part !== undefined) expect(partStatus(part, g).text).toBe(text);
      }
    });
  }
});
