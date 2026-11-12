// src/shared · a view's key, one string for the window's views store, the layer's frames list and
// Electron main's pointer-downs (docs/mcp-apps.md C7; docs/plugin-apps.md A5): a tool call's view
// 'project/agent/call', a document's 'doc:project/a:<artifact>' or 'doc:project/e:<entry>'. Main
// keys a frame by the key the window gave it, and finds a ui/message's view by callViewKey. Pure.
// Owner: apps-host.

/** A tool call's view key. Non-strings read as ''. Pure. */
export function callViewKey(project: unknown, agent: unknown, call: unknown): string {
  const s = (v: unknown): string => (typeof v === 'string' ? v : '');
  return `${s(project)}/${s(agent)}/${s(call)}`;
}
