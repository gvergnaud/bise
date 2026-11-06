// The settings nav (r2-settings, designer m_44311): on the settings route the frame's sidebar shows
// it in place of the projects, as d2's artifacts library does (app/Sidebar.tsx mounts it by
// route). '‹ Settings' first goes back where he was (esc on the page too), then the pages as
// nm-settings signed them: rows 28, a 20 tile r 5 with the page's line icon, the page in view
// current (never the accent). Reads the route and moves the router; nothing else. Owner:
// r2-settings.
import { AppIcon, ListRow, Press, type AppIconName } from '@/ds';
import { useRoute, useRouter, type Router } from '@/app/router';
import { useSetupGauge } from '@/store';
import { settings } from '@/proto/words.gen';
import { PAGES, pageOf, type Page } from './pages';

/** Leave settings: back where he was, else bise's thread (a window opened on settings). */
export function leaveSettings(router: Router): void {
  if (router.canBack()) router.back();
  else router.go({ screen: 'bise' });
}

export function SettingsNav() {
  const router = useRouter();
  const route = useRoute();
  const at = pageOf(route.screen === 'settings' ? route.page : undefined);
  // Setup's N/7 (page models-setup): faint, pink only when a feature failed (an off one waits on nobody)
  const g = useSetupGauge();
  const failed = g?.rows.some((r) => r.state === 'failed') === true;
  return (
    <nav className="b-snav app-s-list" aria-label={settings.nav_label} data-testid="settings.nav">
      <Press
        look="app-s-row back"
        onClick={() => {
          leaveSettings(router);
        }}
        data-testid="settings.nav.back"
      >
        <AppIcon name="back" size={16} />
        <span className="nm">{settings.nav_label}</span>
      </Press>
      {PAGES.map((p) => (
        <ListRow
          key={p.id}
          title={p.title}
          state={
            <span className="b-snav-tile">
              <AppIcon name={PAGE_ICON[p.id]} size={14} />
            </span>
          }
          on={p.id === at.id}
          {...(p.id === 'setup' && g !== null
            ? {
                end: (
                  <span className="b-snav-count" data-failed={failed || undefined} data-testid="settings.nav.setup.count">
                    {settings.gauge_count({ on: String(g.on), of: String(g.of) })}
                  </span>
                ),
              }
            : {})}
          onClick={() => {
            router.go({ screen: 'settings', page: p.id });
          }}
          testId={`settings.nav.${p.id}`}
        />
      ))}
    </nav>
  );
}

/** Each page's tile (mockup 10). */
const PAGE_ICON: Record<Page, AppIconName> = {
  general: 'settings',
  setup: 'check',
  voice: 'mic',
  privacy: 'eye',
  quiet: 'moon',
  approvals: 'shield',
  projects: 'folder',
  computer: 'cursor',
  keys: 'keyboard',
};
