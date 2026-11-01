// time_series: lines over a time axis; hover shows every line at the nearest time.
(function (DV) {
  // The index of the point nearest to t in a sorted [[t, v]] list.
  const nearest = (pts, t) => {
    let lo = 0, hi = pts.length - 1;
    while (hi - lo > 1) { const m = (lo + hi) >> 1; if (pts[m][0] < t) lo = m; else hi = m; }
    return Math.abs(pts[lo][0] - t) <= Math.abs(pts[hi][0] - t) ? lo : hi;
  };

  DV.kinds.time_series = {
    nearest,
    draw(c, o) {
      const W = o.width, H = o.fullscreen ? 360 : 240;
      const all = c.series.map((s, i) => ({ ...s, i, color: DV.color(i) }));
      const shown = all.filter((s) => !o.hidden.has(s.name));
      let t0 = Infinity, t1 = -Infinity, v0 = Infinity, v1 = -Infinity;
      for (const s of all) for (const p of s.points) { if (p[0] < t0) t0 = p[0]; if (p[0] > t1) t1 = p[0]; }
      for (const s of shown) for (const p of s.points) { if (p[1] < v0) v0 = p[1]; if (p[1] > v1) v1 = p[1]; }
      if (c.from_zero) { if (v0 > 0) v0 = 0; if (v1 < 0) v1 = 0; }
      if (t0 === t1) { t0 -= 43200000; t1 += 43200000; }
      const yd = DV.nice(v0, v1, H > 300 ? 6 : 4);
      const left = DV.yLabelW(yd.ticks, o), right = 12, top = 8, bottom = 24;
      const x = DV.scale(t0, t1, left, W - right), y = DV.scale(yd.lo, yd.hi, H - bottom, top);
      let svg = DV.yAxis(yd.ticks, y, left, W - right, o);
      const xt = DV.timeTicks(t0, t1, Math.max(2, Math.floor((W - left - right) / 90)), o);
      for (const k of xt) {
        const xx = DV.px(x(k.t));
        svg += `<line x1="${xx}" x2="${xx}" y1="${H - bottom}" y2="${H - bottom + 4}" style="stroke:var(--k-line-strong)"/>`;
        svg += `<text x="${xx}" y="${H - 6}" text-anchor="middle" style="fill:var(--k-text-3)">${DV.esc(k.label)}</text>`;
      }
      const base = y(Math.max(yd.lo, Math.min(0, yd.hi)));
      for (const s of shown) {
        const d = s.points.map((p, j) => `${j ? "L" : "M"}${DV.px(x(p[0]))} ${DV.px(y(p[1]))}`).join("");
        if (c.area && s.points.length > 1) {
          const p0 = s.points[0], pn = s.points[s.points.length - 1];
          svg += `<path d="${d}L${DV.px(x(pn[0]))} ${DV.px(base)}L${DV.px(x(p0[0]))} ${DV.px(base)}Z" style="fill:${s.color};fill-opacity:${shown.length > 1 ? 0.06 : 0.12};stroke:none"/>`;
        }
        svg += `<path d="${d}" style="fill:none;stroke:${s.color};stroke-width:1.75;stroke-linejoin:round;stroke-linecap:round"/>`;
        // the line alone, a dot on its last point (the hover adds one under the pointer)
        const pn = s.points[s.points.length - 1];
        svg += `<circle cx="${DV.px(x(pn[0]))}" cy="${DV.px(y(pn[1]))}" r="3" style="fill:${s.color}"/>`;
      }
      const hover = (px, py) => {
        if (!shown.length || px < left - 4 || px > W - right + 4 || py < 0 || py > H) return null;
        const t = t0 + ((px - left) / (W - left - right)) * (t1 - t0);
        // the nearest real time among the shown lines, then each line's point there
        let best = null;
        for (const s of shown) { const p = s.points[nearest(s.points, t)]; if (!best || Math.abs(p[0] - t) < Math.abs(best - t)) best = p[0]; }
        const hits = shown.map((s) => ({ s, p: s.points[nearest(s.points, best)] }));
        const rows = hits.slice().sort((a, b) => b.p[1] - a.p[1])
          .map((h) => ({ name: h.s.name, value: DV.num(h.p[1], c.unit, { locale: o.locale }), color: h.s.color }));
        return {
          tip: DV.tipRows(DV.timeTip(best, o), rows),
          guide: { x: DV.px(x(best)), y1: top, y2: H - bottom },
          dots: hits.map((h) => ({ x: DV.px(x(h.p[0])), y: DV.px(y(h.p[1])), color: h.s.color })),
        };
      };
      const legend = all.length > 1 ? all.map((s) => ({ name: s.name, color: s.color, off: o.hidden.has(s.name) })) : [];
      return { height: H, svg, tips: [], hover, legend };
    },
  };
})(DV);
