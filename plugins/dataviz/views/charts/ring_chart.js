// ring_chart: a donut with the total in its middle and a legend that carries
// each slice's value and share (beside the ring, or under it when narrow).
(function (DV) {
  const TAU = Math.PI * 2;
  const pt = (cx, cy, r, a) => `${DV.px(cx + r * Math.sin(a))} ${DV.px(cy - r * Math.cos(a))}`;
  // An annular sector from angle a0 to a1 (clockwise from 12 o'clock).
  DV.arc = (cx, cy, r0, r1, a0, a1) => {
    if (a1 - a0 >= TAU - 1e-6) a1 = a0 + TAU - 1e-4; // a full ring: almost closed
    const big = a1 - a0 > Math.PI ? 1 : 0;
    return `M${pt(cx, cy, r1, a0)}A${r1} ${r1} 0 ${big} 1 ${pt(cx, cy, r1, a1)}L${pt(cx, cy, r0, a1)}A${r0} ${r0} 0 ${big} 0 ${pt(cx, cy, r0, a0)}Z`;
  };

  DV.kinds.ring_chart = {
    draw(c, o) {
      const W = o.width;
      const total = c.slices.reduce((a, s) => a + s.value, 0);
      const color = (s, i) => (s.other ? "var(--k-text-3)" : DV.color(i));
      const narrow = W < 440;
      const R = Math.min(o.fullscreen ? 130 : 96, narrow ? W / 2 - 8 : W * 0.22), r0 = R * 0.64;
      const rowH = 24;
      const legendH = c.slices.length * rowH;
      const ringBox = 2 * R + 8;
      const H = narrow ? ringBox + 12 + legendH : Math.max(ringBox, legendH);
      const cx = narrow ? W / 2 : R + 4, cy = narrow ? R + 4 : H / 2;
      let svg = "", a = 0;
      const gap = c.slices.length > 1 ? Math.min(0.012, 0.25 / c.slices.length) : 0;
      const tips = [];
      c.slices.forEach((s, i) => {
        const span = total ? (s.value / total) * TAU : 0;
        tips.push(DV.tipRows(s.label, [
          { name: "value", value: DV.num(s.value, c.unit, { locale: o.locale }) },
          { name: "share", value: DV.pct(s.value, total) },
        ]));
        if (span > 0) {
          const g = Math.min(gap, span / 3);
          svg += `<path data-tip="${i}" d="${DV.arc(cx, cy, r0, R, a + g, a + span - g)}" style="fill:${color(s, i)}"/>`;
        }
        a += span;
      });
      svg += `<text x="${DV.px(cx)}" y="${DV.px(cy - (c.center ? 4 : 0))}" text-anchor="middle" dy="0.32em" style="fill:var(--k-text);font-size:var(--k-h-md);font-weight:600">${DV.esc(DV.num(total, c.unit, { compact: true, locale: o.locale }))}</text>`;
      if (c.center) svg += `<text x="${DV.px(cx)}" y="${DV.px(cy + 16)}" text-anchor="middle" style="fill:var(--k-text-3)">${DV.esc(DV.clip(c.center, r0 * 1.6))}</text>`;
      // the legend: swatch, label, value, share
      const lx = narrow ? 8 : 2 * R + 32, lw = W - lx - 4;
      const ly = narrow ? ringBox + 12 : (H - legendH) / 2;
      const pctW = 48, valW = Math.min(110, Math.max(...c.slices.map((s) => DV.textW(DV.num(s.value, c.unit, { locale: o.locale })))) + 12);
      c.slices.forEach((s, i) => {
        const yy = ly + i * rowH + rowH / 2;
        svg += `<g data-tip="${i}"><rect x="${lx - 4}" y="${DV.px(yy - rowH / 2)}" width="${DV.px(lw + 4)}" height="${rowH}" style="fill:transparent"/>`
          + `<rect x="${lx}" y="${DV.px(yy - 4)}" width="8" height="8" rx="2" style="fill:${color(s, i)}"/>`
          + `<text x="${lx + 16}" y="${DV.px(yy)}" dy="0.32em" style="fill:var(--k-text)">${DV.esc(DV.clip(s.label, lw - 16 - valW - pctW))}</text>`
          + `<text x="${DV.px(lx + lw - pctW)}" y="${DV.px(yy)}" dy="0.32em" text-anchor="end" style="fill:var(--k-text)">${DV.esc(DV.num(s.value, c.unit, { locale: o.locale }))}</text>`
          + `<text x="${DV.px(lx + lw)}" y="${DV.px(yy)}" dy="0.32em" text-anchor="end" style="fill:var(--k-text-3)">${DV.esc(DV.pct(s.value, total))}</text></g>`;
        if (i < c.slices.length - 1) svg += `<line x1="${lx}" x2="${DV.px(lx + lw)}" y1="${DV.px(yy + rowH / 2)}" y2="${DV.px(yy + rowH / 2)}" style="stroke:var(--k-line)"/>`;
      });
      return { height: Math.ceil(H), svg, tips };
    },
  };
})(DV);
