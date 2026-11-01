// bar_chart: a top list, one horizontal bar per row, the value after the bar.
(function (DV) {
  DV.kinds.bar_chart = {
    draw(c, o) {
      const W = o.width, rowH = 26, barH = 14;
      const max = Math.max(...c.bars.map((b) => b.value)) || 1;
      const total = c.bars.reduce((a, b) => a + b.value, 0) + (c.rest ? c.rest.value : 0);
      const rankW = DV.textW(String(c.bars.length)) + 8;
      const labelW = Math.min(W * 0.38, Math.max(...c.bars.map((b) => DV.textW(b.label))) + 12);
      const vals = c.bars.map((b) => DV.num(b.value, c.unit, { locale: o.locale }) + (b.note ? ` · ${b.note}` : ""));
      const valW = Math.min(W * 0.3, Math.max(...vals.map((v) => DV.textW(v))) + 10);
      const x0 = rankW + labelW, span = Math.max(40, W - x0 - valW);
      let svg = "";
      const tips = [];
      c.bars.forEach((b, i) => {
        const y = i * rowH, w = Math.max(2, (b.value / max) * span);
        const rows = [{ name: "value", value: DV.num(b.value, c.unit, { locale: o.locale }) }, { name: "share of all", value: DV.pct(b.value, total) }];
        if (b.note) rows.push({ name: "note", value: b.note });
        tips.push(DV.tipRows(`${i + 1}. ${b.label}`, rows));
        svg += `<g data-tip="${i}"><rect x="0" y="${y}" width="${W}" height="${rowH}" style="fill:transparent"/>`
          + `<text x="0" y="${y + rowH / 2}" dy="0.32em" style="fill:var(--k-text-3)">${i + 1}</text>`
          + `<text x="${rankW}" y="${y + rowH / 2}" dy="0.32em" style="fill:var(--k-text)">${DV.esc(DV.clip(b.label, labelW - 10))}</text>`
          + `<rect x="${x0}" y="${y + (rowH - barH) / 2}" width="${DV.px(w)}" height="${barH}" rx="3" style="fill:${DV.color(0)}"/>`
          + `<text x="${DV.px(x0 + w + 6)}" y="${y + rowH / 2}" dy="0.32em" style="fill:var(--k-text-2)">${DV.esc(DV.clip(vals[i], valW))}</text></g>`;
      });
      let H = c.bars.length * rowH;
      if (c.rest) {
        H += 22;
        svg += `<text x="${rankW}" y="${H - 6}" style="fill:var(--k-text-3)">${DV.esc(`+ ${c.rest.count} more · ${DV.num(c.rest.value, c.unit, { locale: o.locale })} together`)}</text>`;
      }
      return { height: H, svg, tips };
    },
  };
})(DV);
