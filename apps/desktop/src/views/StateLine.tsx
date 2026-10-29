// src/views · a view's quiet state (designer m_50319): one centred text-3 line in a box at the
// view's last height, so nothing jumps when it loads; never a spinner. The card and the tab both
// draw it. Owner: mcp-apps-host.
import type { ReactNode } from 'react';
import { Press } from '@/ds';
import { views } from '@/proto/words_views.gen';
import type { ViewRef } from './port';
import { DEFAULT_H, type ViewState, type ViewsStore } from './store';

/** A view's state words: one quiet centred line in a box at the view's last height (no jump when it loads), 'try again' after a failure. */
export function StateLine({ store, r, state, height = DEFAULT_H }: { store: ViewsStore; r: ViewRef; state: ViewState; height?: number }) {
  const box = (testId: string, children: ReactNode, title?: string) => (
    <div className="vw-quiet" style={{ height }} data-testid={testId} {...(title === undefined ? {} : { title })}>
      <span className="vw-line">{children}</span>
    </div>
  );
  switch (state.kind) {
    case 'none':
      return box('views.state.loading', views.loading);
    case 'loading':
      return box('views.state.loading', views.loading);
    case 'expired':
      return box('views.state.expired', views.expired);
    case 'failed':
      return box(
        'views.state.failed',
        <>
          {views.failed}
          <Press
            look="vw-act"
            onClick={() => {
              store.retry(r);
            }}
            data-testid="views.retry"
          >
            {views.retry}
          </Press>
        </>,
        state.message,
      );
    case 'live':
      return state.phase === 'live' ? null : box('views.state.loading', views.loading);
  }
}

