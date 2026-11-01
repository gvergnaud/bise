// sankey: nodes in columns by their longest path from a source (the sinks on the
// last column), heights by value, bands from source to target. Labels sit on the
// open side of their node: right in the left half, left in the right half.
(function (DV) {
  // The layout, pure: nodes {i, name, value, depth, x0, x1, y0, y1},
  // links {source, target, value, width, sy, ty} (sy/ty: the band's center at each end).
  // opt: {x0, x1} the nodes' span (the label gutters outside it), top (room for a
  // label above the first node), pad (the gap between two nodes), nodeW.
  DV.sankeyLayout = (c, W, H, opt = {}) => {
    const nodeW = opt.nodeW ?? 10, pad = opt.pad ?? 14, gx0 = opt.x0 ?? 0, gx1 = opt.x1 ?? W, top = opt.top ?? 0;
    const n = c.nodes.length;
    const nodes = c.nodes.map((d, i) => ({ i, name: d.name, inV: 0, outV: 0, ins: [], outs: [], depth: 0 }));
    const links = c.links.map((l) => ({ ...l }));
    for (const l of links) { nodes[l.source].outV += l.value; nodes[l.target].inV += l.value; nodes[l.source].outs.push(l); nodes[l.target].ins.push(l); }
    for (const d of nodes) d.value = Math.max(d.inV, d.outV);
    // depth: the longest path from a source (the graph has no loop: the server checked)
    for (let changed = true, guard = 0; changed && guard <= n; guard++) {
      changed = false;
      for (const l of links) if (nodes[l.target].depth < nodes[l.source].depth + 1) { nodes[l.target].depth = nodes[l.source].depth + 1; changed = true; }
    }
    const last = Math.max(0, ...nodes.map((d) => d.depth));
    for (const d of nodes) if (!d.outs.length) d.depth = last; // sinks to the right edge
    const cols = Array.from({ length: last + 1 }, () => []);
    for (const d of nodes) cols[d.depth].push(d);
    const ky = Math.min(...cols.map((col) => (H - top - (col.length - 1) * pad) / Math.max(1e-9, col.reduce((a, d) => a + d.value, 0))));
    const colX = (k) => gx0 + (last ? (k * (gx1 - gx0 - nodeW)) / last : (gx1 - gx0 - nodeW) / 2);
    for (const col of cols) {
      let y = top;
      for (const d of col) { d.x0 = colX(d.depth); d.x1 = d.x0 + nodeW; d.y0 = y; d.y1 = y + d.value * ky; y = d.y1 + pad; }
      // center the column
      const off = (H - y + pad) / 2;
      for (const d of col) { d.y0 += off; d.y1 += off; }
    }
    const center = (d) => (d.y0 + d.y1) / 2;
    const settle = (col) => {
      col.sort((a, b) => a.y0 - b.y0);
      let y = top;
      for (const d of col) { const dy = y - d.y0; if (dy > 0) { d.y0 += dy; d.y1 += dy; } y = d.y1 + pad; }
      let over = y - pad - H;
      for (let j = col.length - 1; j >= 0 && over > 0; j--) {
        const d = col[j]; d.y0 -= over; d.y1 -= over;
        over = j > 0 ? col[j - 1].y1 + pad - d.y0 : 0;
      }
    };
    // relax: move each node toward the weighted center of its neighbours, both ways
    for (let it = 0, a = 1; it < 8; it++, a *= 0.9) {
      for (const col of cols.slice(1)) {
        for (const d of col) if (d.ins.length) {
          const w = d.ins.reduce((s, l) => s + l.value, 0);
          const yc = d.ins.reduce((s, l) => s + center(nodes[l.source]) * l.value, 0) / w;
          const dy = (yc - center(d)) * a; d.y0 += dy; d.y1 += dy;
        }
        settle(col);
      }
      for (const col of cols.slice(0, -1).reverse()) {
        for (const d of col) if (d.outs.length) {
          const w = d.outs.reduce((s, l) => s + l.value, 0);
          const yc = d.outs.reduce((s, l) => s + center(nodes[l.target]) * l.value, 0) / w;
          const dy = (yc - center(d)) * a; d.y0 += dy; d.y1 += dy;
        }
        settle(col);
      }
    }
    // band ends, ordered by the other end's height so bands don't cross at a node
    for (const d of nodes) {
      d.outs.sort((a, b) => nodes[a.target].y0 - nodes[b.target].y0);
      d.ins.sort((a, b) => nodes[a.source].y0 - nodes[b.source].y0);
      let y = d.y0;
      for (const l of d.outs) { l.width = l.value * ky; l.sy = y + l.width / 2; y += l.width; }
      y = d.y0;
      for (const l of d.ins) { l.ty = y + l.value * ky / 2; y += l.value * ky; }
    }
    return { nodes, links, last };
  };

  DV.kinds.sankey = {
    draw(c, o) {
      const W = o.width;
      // Labels never sit over a band or a node: the first column's in a gutter on
      // the left, the last column's in one on the right, a middle column's above
      // its node (so middle columns get a bigger gap and a top margin). A label
      // shows whole ("name value", else "name") or not at all: the tip has it.
      const L0 = DV.sankeyLayout(c, W, 100);
      const val = (d) => DV.num(d.value, c.unit, { compact: true, locale: o.locale });
      const texts = (d) => [`${d.name} ${val(d)}`, d.name];
      const gutter = (depth) => Math.min(W * 0.26, 8 + Math.max(0, ...L0.nodes.filter((d) => d.depth === depth).map((d) => DV.textW(texts(d)[0]))));
      const gl = gutter(0), gr = gutter(L0.last);
      const middle = L0.last >= 2, pad = middle ? 24 : 14, top = middle ? 18 : 0;
      const per = {};
      for (const d of L0.nodes) per[d.depth] = (per[d.depth] || 0) + 1;
      const H = Math.min(640, Math.max(o.fullscreen ? 420 : 260, top + Math.max(...Object.values(per)) * (pad + 18)));
      const { nodes, links, last } = DV.sankeyLayout(c, W, H, { x0: gl, x1: W - gr, top, pad });
      const color = (d) => DV.color(d.i);
      const tips = [];
      let bands = "", boxes = "", labels = "";
      links.forEach((l) => {
        const s = nodes[l.source], t = nodes[l.target], i = tips.length;
        tips.push(DV.tipRows(`${s.name} → ${t.name}`, [
          { name: "flow", value: DV.num(l.value, c.unit, { locale: o.locale }) },
          { name: `share of ${s.name}`, value: DV.pct(l.value, s.outV) },
        ]));
        const x0 = s.x1, x1 = t.x0, xm = (x0 + x1) / 2, h = l.width / 2;
        const d = `M${DV.px(x0)} ${DV.px(l.sy - h)}C${DV.px(xm)} ${DV.px(l.sy - h)} ${DV.px(xm)} ${DV.px(l.ty - h)} ${DV.px(x1)} ${DV.px(l.ty - h)}`
          + `L${DV.px(x1)} ${DV.px(l.ty + h)}C${DV.px(xm)} ${DV.px(l.ty + h)} ${DV.px(xm)} ${DV.px(l.sy + h)} ${DV.px(x0)} ${DV.px(l.sy + h)}Z`;
        bands += `<path data-tip="${i}" d="${d}" style="fill:${color(s)};fill-opacity:.28"/>`;
      });
      const colGap = last ? (W - gl - gr - 10) / last : W;
      const halo = "stroke:var(--k-bg);stroke-width:3;stroke-linejoin:round;paint-order:stroke";
      nodes.forEach((d) => {
        const i = tips.length, rows = [];
        if (d.inV) rows.push({ name: "in", value: DV.num(d.inV, c.unit, { locale: o.locale }) });
        if (d.outV) rows.push({ name: "out", value: DV.num(d.outV, c.unit, { locale: o.locale }) });
        tips.push(DV.tipRows(d.name, rows));
        boxes += `<rect data-tip="${i}" x="${DV.px(d.x0)}" y="${DV.px(d.y0)}" width="${DV.px(d.x1 - d.x0)}" height="${DV.px(Math.max(1, d.y1 - d.y0))}" rx="2" style="fill:${color(d)}"/>`;
        let x, y, anchor, room;
        if (d.depth === 0) { x = d.x0 - 6; y = (d.y0 + d.y1) / 2; anchor = "end"; room = gl - 8; }
        else if (d.depth === last) { x = d.x1 + 6; y = (d.y0 + d.y1) / 2; anchor = "start"; room = gr - 8; }
        else { x = (d.x0 + d.x1) / 2; y = d.y0 - 9; anchor = "middle"; room = colGap - 12; }
        const t = DV.fit(texts(d), room);
        if (!t) return;
        const withVal = t !== d.name;
        labels += `<text x="${DV.px(x)}" y="${DV.px(y)}" dy="0.32em" text-anchor="${anchor}" pointer-events="none" style="fill:var(--k-text);${halo}">${DV.esc(d.name)}${withVal ? ` <tspan style="fill:var(--k-text-2)">${DV.esc(val(d))}</tspan>` : ""}</text>`;
      });
      return { height: H, svg: bands + boxes + labels, tips };
    },
  };
})(DV);
