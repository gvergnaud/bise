import { describe, expect, it } from 'vitest';
import { quoteOn } from './quote';

describe('his context.quote (the ❝ chip), one rule for main and the capsule', () => {
  it('off only when false; unset or junk: on', () => {
    expect(quoteOn({ context: { quote: false } })).toBe(false);
    expect(quoteOn({ context: { quote: true } })).toBe(true);
    expect(quoteOn({ context: { shot: false } })).toBe(true);
    expect(quoteOn({ context: 'x' })).toBe(true);
    expect(quoteOn(null)).toBe(true);
  });
});
