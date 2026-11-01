// Fake data, one chart per kind: the tests and the demo page draw these.
const days = (n, start, f) => Array.from({ length: n }, (_, i) => [new Date(Date.UTC(2026, 8, start + i)).toISOString().slice(0, 10), f(i)]);

export const FIXTURES = {
  time_series: {
    title: "Daily active users",
    subtitle: "September 2026 · fake data",
    series: [
      { name: "desktop", points: days(30, 1, (i) => Math.round(1200 + i * 18 + 140 * Math.sin(i / 2.2) - (i % 7 >= 5 ? 260 : 0))) },
      { name: "terminal", points: days(30, 1, (i) => Math.round(640 + i * 9 + 60 * Math.cos(i / 3) - (i % 7 >= 5 ? 120 : 0))) },
    ],
  },
  ring_chart: {
    title: "Where the week went",
    unit: "h",
    center: "this week",
    slices: [
      { label: "bise", value: 21.5 }, { label: "Meetings", value: 6 }, { label: "Review", value: 4.5 },
      { label: "Mail", value: 3 }, { label: "Hiring", value: 2.5 }, { label: "Reading", value: 1.5 },
      { label: "Admin", value: 1 }, { label: "Travel", value: 0.75 }, { label: "Other stuff", value: 0.5 },
    ],
  },
  bar_chart: {
    title: "Slowest endpoints",
    subtitle: "p95 latency, last 24 h · fake data",
    unit: "ms",
    limit: 8,
    bars: [
      ["/api/search", 842], ["/api/export", 611], ["/api/threads/:id", 388], ["/api/upload", 352],
      ["/api/agents", 240], ["/api/pages/:id", 198], ["/api/views/call", 164], ["/api/settings", 121],
      ["/api/me", 44], ["/api/health", 3],
    ].map(([label, value]) => ({ label, value })),
  },
  bucket_chart: {
    title: "Commits per week",
    subtitle: "fake repo · 2026",
    bucket: "week",
    buckets: Array.from({ length: 16 }, (_, i) => new Date(Date.UTC(2026, 6, 6 + 7 * i)).toISOString().slice(0, 10)),
    series: [
      { name: "features", values: [12, 18, 9, 22, 30, 27, 14, 8, 19, 25, 33, 29, 21, 17, 26, 31] },
      { name: "fixes", values: [8, 6, 11, 9, 14, 10, 7, 5, 12, 9, 16, 13, 8, 10, 11, 9] },
      { name: "docs", values: [2, 3, 1, 4, 2, 5, 3, 1, 2, 4, 3, 6, 2, 3, 4, 2] },
    ],
  },
  treemap: {
    title: "Disk usage",
    subtitle: "fake laptop",
    unit: "GB",
    items: [
      { name: "Photos", children: [{ name: "2024", value: 38 }, { name: "2025", value: 52 }, { name: "2026", value: 27 }] },
      { name: "Code", children: [{ name: "bise", children: [{ name: "target", value: 41 }, { name: "node_modules", value: 9 }, { name: "src", value: 1.2 }] }, { name: "other repos", value: 18 }] },
      { name: "Apps", value: 44 },
      { name: "Music", value: 21 },
      { name: "Mail", value: 6 },
      { name: "System", value: 14 },
    ],
  },
  sankey: {
    title: "October budget",
    unit: "€",
    subtitle: "fake household",
    links: [
      { from: "Salary", to: "Budget", value: 4200 }, { from: "Freelance", to: "Budget", value: 900 },
      { from: "Budget", to: "Housing", value: 1650 }, { from: "Budget", to: "Food", value: 780 },
      { from: "Budget", to: "Transport", value: 310 }, { from: "Budget", to: "Savings", value: 1500 },
      { from: "Budget", to: "Leisure", value: 860 }, { from: "Housing", to: "Rent", value: 1400 },
      { from: "Housing", to: "Energy", value: 250 }, { from: "Leisure", to: "Restaurants", value: 420 },
      { from: "Leisure", to: "Trips", value: 440 },
    ],
  },
};
