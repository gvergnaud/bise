// The renderer's root: mounts src/app's <App /> (amb-win's shell) into #root, or in the capsule's
// window (window.bise.surface === 'capsule') src/capsule's <CapsuleRoot /> (amb-web).
// Owner: amb-core (scaffold); keep it this small.
import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { App } from '@/app/App';
import { CapsuleRoot } from '@/capsule';
import { wireUsageInput } from '@/app/usage';

const root = document.getElementById('root');
if (!root) throw new Error('index.html has no #root');
// every surface in the DS's UI type (base.css .b-ui), never the browser's Times (ambient m_10689)
document.body.classList.add('b-ui');
// his input as a signal with no content, for the app's engaged minutes (app/usage.ts)
wireUsageInput();

createRoot(root).render(
  <StrictMode>
    {(window as Partial<Window>).bise?.surface === 'capsule' ? <CapsuleRoot /> : <App />}
  </StrictMode>,
);
