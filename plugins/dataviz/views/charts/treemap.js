// treemap: squarified rectangles; top-level items take the colors, groups get a
// header with their name, leaves a tint of their group's color.
(function (DV) {
  // Squarified layout (Bruls, Huizing, van Wijk) of values sorted biggest first
  // into the box x, y, w, h: one {x, y, w, h} per value, in the same order.
  DV.squarify = (values, x, y, w, h) => {
    const out = new Array(values.length);
    const total = values.reduce((a, v) => a + v, 0);
    if (!total || w <= 0 || h <= 0) { values.forEach((_, i) => { out[i] = { x, y, w: 0, h: 0 }; }); return out; }
    const k = (w * h) / total;
    let i = 0;
    while (i < values.length) {
      const side = Math.min(w, h);
      let row = [], sum = 0, worst = Infinity;
      for (let j = i; j < values.length; j++) {
        const a = values[j] * k, s = sum + a;
        const mx = Math.max(...row, a), mn = Math.min(...row, a);
        const wst = Math.max((side * side * mx) / (s * s), (s * s) / (side * side * mn));
        if (row.length && wst > worst) break;
        row.push(a); sum = s; worst = wst;
      }
      const thick = sum / side;
      let off = 0;
      row.forEach((a, r) => {
        const len = thick ? a / thick : 0;
        out[i + r] = w >= h ? { x, y: y + off, w: thick, h: len } : { x: x + off, y, w: len, h: thick };
        off += len;
      });
      if (w >= h) { x += thick; w -= thick; } else { y += thick; h -= thick; }
      i += row.length;
    }
    return out;
  };

  DV.kinds.treemap = {
    draw(c, o) {
      const W = o.width, H = o.fullscreen ? 440 : Math.round(Math.min(340, Math.max(220, W * 0.56)));
      const total = c.items.reduce((a, n) => a + n.value, 0);
      const tips = [];
      let svg = "";
      const tile = (n, path, color, box, depth) => {
        const { x, y, w, h } = box;
        if (w < 1 || h < 1) return;
        const i = tips.length;
        const rows = [{ name: "value", value: DV.num(n.value, c.unit, { locale: o.locale }) }, { name: "share of all", value: DV.pct(n.value, total) }];
        if (path.length) rows.push({ name: `share of ${path[path.length - 1]}`, value: DV.pct(n.value, n.parentValue) });
        tips.push(DV.tipRows([...path, n.name].join(" › "), rows));
        if (n.children) {
          const val = DV.num(n.value, c.unit, { compact: true, locale: o.locale });
          const name = h > 40 ? DV.fit([n.name], w - 12 - DV.textW(` · ${val}`)) ? n.name : DV.fit([n.name], w - 12) : "";
          const head = name ? 20 : 0;
          svg += `<g data-tip="${i}"><rect x="${DV.px(x)}" y="${DV.px(y)}" width="${DV.px(w)}" height="${DV.px(h)}" rx="3" style="fill:${color};fill-opacity:${depth === 1 ? 0.1 : 0.06};stroke:var(--k-bg);stroke-width:2"/>`;
          if (name) {
            const withVal = DV.textW(`${name} · ${val}`) <= w - 12;
            svg += `<text x="${DV.px(x + 6)}" y="${DV.px(y + 13)}" style="fill:var(--k-text);font-weight:600">${DV.esc(name)}${withVal ? `<tspan style="fill:var(--k-text-2);font-weight:400"> · ${DV.esc(val)}</tspan>` : ""}</text>`;
          }
          svg += "</g>";
          const pad = 2;
          const kids = n.children.map((k) => ({ ...k, parentValue: n.value }));
          const boxes = DV.squarify(kids.map((k) => k.value), x + pad, y + head + pad, w - 2 * pad, h - head - 2 * pad);
          kids.forEach((k, j) => tile(k, [...path, n.name], color, boxes[j], depth + 1));
          return;
        }
        svg += `<g data-tip="${i}"><rect x="${DV.px(x)}" y="${DV.px(y)}" width="${DV.px(w)}" height="${DV.px(h)}" rx="${w > 8 && h > 8 ? 3 : 0}" style="fill:${color};fill-opacity:${depth === 1 ? 0.2 : 0.16};stroke:var(--k-bg);stroke-width:1"/>`;
        // a tint light enough for --k-text and --k-text-2 on it in both themes; labels whole or none
        const name = h > 22 ? DV.fit([n.name], w - 12) : "";
        if (name) {
          svg += `<text x="${DV.px(x + 6)}" y="${DV.px(y + 14)}" style="fill:var(--k-text)">${DV.esc(name)}</text>`;
          const val = h > 38 ? DV.fit([DV.num(n.value, c.unit, { compact: true, locale: o.locale })], w - 12) : "";
          if (val) svg += `<text x="${DV.px(x + 6)}" y="${DV.px(y + 30)}" style="fill:var(--k-text-2)">${DV.esc(val)}</text>`;
        }
        svg += "</g>";
      };
      const boxes = DV.squarify(c.items.map((n) => n.value), 0, 0, W, H);
      c.items.forEach((n, i) => tile({ ...n, parentValue: total }, [], DV.color(i), boxes[i], 1));
      return { height: H, svg, tips };
    },
  };
})(DV);
