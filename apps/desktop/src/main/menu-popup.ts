// src/main · a native menu at the pointer (window.bise.popupMenu; bise fait mac, decision d4): the
// '…' and the right click on an agent, a project, a tab, an artifact, a process, a scheduled task,
// drawn by macOS (Electron's Menu.popup: its width, radius, rows, icon and key column are the
// system's, the mockup's 248 / r 10 / 28 / 16 + 8 / key text-3 right) from the items the renderer's
// registry built (app/menus/registry.ts). nativeItems is the one converter: the menu bar's Agent
// menu (main/agentmenu.ts) uses it too. Resolves the picked item's id, null when he closes it.
// Owner: nm-menus.
import { Menu, nativeImage, type BrowserWindow, type MenuItemConstructorOptions, type NativeImage } from 'electron';
import type { PopupAt, PopupItem } from '../shared/api';

/** An SF Symbol's image for a menu (macOS only); none elsewhere or when the name is unknown. */
export type MenuIcon = (symbol: string) => NativeImage | undefined;

/** The system's SF Symbol, macOS only (an empty image is left out). */
export const sfSymbol: MenuIcon = (symbol) => {
  if (process.platform !== 'darwin') return undefined;
  const img = nativeImage.createMenuSymbol(symbol);
  return img.isEmpty() ? undefined : img;
};

/**
 * The registry's items as Electron's template; a pick calls `pick(id)`. `register`: whether the
 * keys fire (the menu bar's) or are only shown (a popup's: the menu bar or the window's own keys do
 * the work). `dim`: every item dimmed (the menu bar's Agent menu with no agent in view, designer
 * m_42681: the bar stays stable). Pure but for `icon`.
 */
export function nativeItems(items: readonly PopupItem[], pick: (id: string) => void, icon: MenuIcon, register: boolean, dim = false): MenuItemConstructorOptions[] {
  return items.map((i): MenuItemConstructorOptions => {
    if (i.type === 'separator') return { type: 'separator' };
    const img = i.icon === undefined ? undefined : icon(i.icon);
    const off = dim ? { enabled: false } : {};
    if (i.type === 'submenu') return { label: i.label, submenu: nativeItems(i.items, pick, icon, register, dim), ...off, ...(img !== undefined ? { icon: img } : {}) };
    const id = i.id;
    return {
      id,
      label: i.label,
      ...(i.sublabel !== undefined ? { sublabel: i.sublabel } : {}),
      ...(i.accelerator !== undefined ? { accelerator: i.accelerator, registerAccelerator: register } : {}),
      ...(i.checked !== undefined ? { type: 'checkbox' as const, checked: i.checked } : {}),
      ...(img !== undefined ? { icon: img } : {}),
      ...off,
      click: () => {
        pick(id);
      },
    };
  });
}

/** Where the menu opens: under the '…' button's box, at its left edge; none: at the pointer. Pure. */
export function popupPoint(at: PopupAt | undefined): { x: number; y: number } | undefined {
  if (at === undefined) return undefined;
  return { x: Math.round(at.x), y: Math.round(at.y + at.height) };
}

/** Shows `items` as a native menu over `win` (under `at`, else at the pointer); the picked id, or null. */
export const popupMenu = (win: BrowserWindow | null, items: readonly PopupItem[], at: PopupAt | undefined): Promise<string | null> => {
  if (win === null || items.length === 0) return Promise.resolve(null);
  return new Promise((resolve) => {
    let picked: string | null = null;
    const pick = (id: string) => {
      picked = id;
    };
    Menu.buildFromTemplate(nativeItems(items, pick, sfSymbol, false)).popup({
      window: win,
      ...popupPoint(at),
      // macOS runs the picked item's click right after the menu closes: read it a tick later
      callback: () => {
        setImmediate(() => {
          resolve(picked);
        });
      },
    });
  });
};
