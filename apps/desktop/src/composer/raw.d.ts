// Text files imported whole by the bundler (Vite's `?raw`): the emoji table, the hub's jsonl
// fixtures a test reads as they are (tasks/what.test.ts: work_what.jsonl).
declare module '*.tsv?raw' {
  const text: string;
  export default text;
}
declare module '*.jsonl?raw' {
  const text: string;
  export default text;
}
