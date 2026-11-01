// bucket_chart: one column per bucket (stacked or side by side series); the
// axis labels thin out to fit; hover a column for its numbers.
(function (DV) {
  // A bucket's axis label: the grain's unit, the bigger one where it changes.
  const label = (c, i, o) => {
    if (!c.time) return c.buckets[i];
    const g = c.time.grain, t = c.buckets[i], prev = i ? c.buckets[i - 1] : null;
    if (g === "quarter") {
      const d = new Date(t), y = o.utc ? d.getUTCFullYear() : d.getFullYear(), m = o.utc ? d.getUTCMonth() : d.getMonth();
      const py = prev == null ? null : (o.utc ? new Date(prev).getUTCFullYear() : new Date(prev).getFullYear());
      return `Q${Math.floor(m / 3) + 1}` + (py !== y ? ` ${y}` : "");
    }
    return DV.timeLabel(t, g, o, prev);
  };

  DV.kinds.bucket_chart = {
    label,
    draw(c, o) {
      const W = o.width, H = o.fullscreen ? 340 : 240, n = c.buckets.length;
      const all = c.series.map((s, i) => ({ ...s, i, color: DV.color(i) }));
      const shown = all.filter((s) => !o.hidden.has(s.name));
      let v0 = 0, v1 = 0;
      for (let j = 0; j < n; j++) {
        if (c.stacked) v1 = Math.max(v1, shown.reduce((a, s) => a + s.values[j], 0));
        else for (const s of shown) { v0 = Math.min(v0, s.values[j]); v1 = Math.max(v1, s.values[j]); }
      }
      const yd = DV.nice(v0, v1, H > 300 ? 6 : 4);
      const left = DV.yLabelW(yd.ticks, o), right = 4, top = 8, bottom = 24;
      const y = DV.scale(yd.lo, yd.hi, H - bottom, top);
      const bw = (W - left - right) / n;
      const barW = Math.max(1, bw * (n > 60 ? 0.86 : 0.72));
      let svg = DV.yAxis(yd.ticks, y, left, W - right, o);
      // the labels: every k-th bucket so they never touch
      const labels = c.buckets.map((_, j) => label(c, j, o));
      const widest = Math.max(...labels.map((l) => DV.textW(l))) + 10;
      const every = Math.max(1, Math.ceil(widest / bw));
      for (let j = 0; j < n; j += every) {
        const xx = DV.px(left + bw * (j + 0.5));
        svg += `<text x="${xx}" y="${H - 6}" text-anchor="middle" style="fill:var(--k-text-3)">${DV.esc(DV.clip(labels[j], Math.max(bw * every - 6, 24)))}</text>`;
      }
      const zero = y(Math.max(yd.lo, Math.min(0, yd.hi)));
      const r = Math.min(2, barW / 4);
      for (let j = 0; j < n; j++) {
        const x0 = left + bw * j + (bw - barW) / 2;
        if (c.stacked) {
          let acc = 0;
          shown.forEach((s, k) => {
            const v = s.values[j];
            if (!v) return;
            const ya = y(acc), yb = y(acc + v);
            acc += v;
            const gap = k < shown.length - 1 && ya - yb > 3 ? 1 : 0;
            svg += `<rect x="${DV.px(x0)}" y="${DV.px(yb + gap)}" width="${DV.px(barW)}" height="${DV.px(Math.max(0.5, ya - yb - gap))}" style="fill:${s.color}"/>`;
          });
        } else {
          const sw = barW / shown.length;
          shown.forEach((s, k) => {
            const v = s.values[j], ya = y(v);
            svg += `<rect x="${DV.px(x0 + k * sw)}" y="${DV.px(Math.min(ya, zero))}" width="${DV.px(Math.max(0.5, sw - (shown.length > 1 ? 1 : 0)))}" height="${DV.px(Math.max(v ? 0.5 : 0, Math.abs(zero - ya)))}" rx="${shown.length > 1 ? 0 : r}" style="fill:${s.color}"/>`;
          });
        }
      }
      const hover = (px, py) => {
        if (!shown.length || px < left || px > W - right || py < 0 || py > H) return null;
        const j = Math.min(n - 1, Math.floor((px - left) / bw));
        const head = c.time ? DV.timeTip(c.buckets[j], o, c.time.grain) : String(c.buckets[j]);
        const rows = shown.map((s) => ({ name: s.name, value: DV.num(s.values[j], c.unit, { locale: o.locale }), color: s.color }));
        if (shown.length > 1) rows.push({ name: "total", value: DV.num(shown.reduce((a, s) => a + s.values[j], 0), c.unit, { locale: o.locale }) });
        return { tip: DV.tipRows(head, rows), band: { x: DV.px(left + bw * j), y: top, w: DV.px(bw), h: H - bottom - top } };
      };
      const legend = all.length > 1 ? all.map((s) => ({ name: s.name, color: s.color, off: o.hidden.has(s.name) })) : [];
      return { height: H, svg, tips: [], hover, legend };
    },
  };
})(DV);
