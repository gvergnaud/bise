// Playwright config for the README film's take only (readme-film): one test, one worker.
import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: '.',
  testMatch: ['readme.spec.ts'],
  workers: 1,
  reporter: [['line']],
  outputDir: '../../.harness-cache/film-results',
});
