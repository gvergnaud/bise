// A tab's or a screen's error boundary (pr-crash, his m_68101): a throw while drawing one view
// (a panel tab, the centre's screen) used to unmount the whole React root, a blank window that
// looked like a crashed renderer. Now that view alone shows 'This view stopped on an error' with
// Reload (it draws again from scratch), the rest of the window stays, and the error goes to
// app.log as one line (`where`, its message, its first frames), the trace we had none of.
// Owner: pr-crash.
import { Component, type ReactNode } from 'react';
import { Button, Empty } from '@/ds';
import { panel } from '@/proto/words_panel.gen';

interface Props {
  /** what it holds, in app.log's line ('panel tab pr', 'screen thread') */
  readonly where: string;
  readonly testId?: string;
  readonly children: ReactNode;
}

interface State {
  readonly failed: boolean;
  /** bumped by Reload: its children are drawn again, from a fresh mount */
  readonly round: number;
}

/** how many frames of the stack go in the line (app.log keeps 300 chars of it) */
const FRAMES = 3;

/** The error's app.log line: where, its message, its first frames. Pure. */
export function failedLine(where: string, err: unknown): string {
  const e = err instanceof Error ? err : new Error(String(err));
  const frames = (e.stack ?? '').split('\n').filter((l) => l.trim().startsWith('at ')).slice(0, FRAMES).map((l) => l.trim());
  return [`view failed: ${where}: ${e.name}: ${e.message}`, ...frames].join(' | ');
}

export class Boundary extends Component<Props, State> {
  override state: State = { failed: false, round: 0 };

  static getDerivedStateFromError(): Partial<State> {
    return { failed: true };
  }

  override componentDidCatch(err: unknown): void {
    (window as Partial<Window>).bise?.log?.(failedLine(this.props.where, err));
  }

  override render(): ReactNode {
    const { failed, round } = this.state;
    if (failed) {
      const reload = () => {
        this.setState((s) => ({ failed: false, round: s.round + 1 }));
      };
      return (
        <Empty testId={this.props.testId ?? 'app.view-failed'} title={panel.view_failed}
          action={<Button kind="ghost" onClick={reload} testId={`${this.props.testId ?? 'app.view-failed'}.reload`}>{panel.view_failed_reload}</Button>}>
          {panel.view_failed_why}
        </Empty>
      );
    }
    return <Bounded key={round}>{this.props.children}</Bounded>;
  }
}

/** its children under a key Reload changes */
function Bounded({ children }: { children: ReactNode }) {
  return <>{children}</>;
}
