// stays JS: an eslint.config.ts needs jiti, a dependency for what this file does as is.
// ESLint for bise desktop: typed rules (typescript-eslint strictTypeChecked)
// on every .ts/.tsx through the two tsconfigs, React hooks rules on the
// renderer, plain JS rules on the scripts. `pnpm lint` fails on any warning.
import js from '@eslint/js';
import tseslint from 'typescript-eslint';
import reactHooks from 'eslint-plugin-react-hooks';
import globals from 'globals';

export default tseslint.config(
  { ignores: ['out/', 'dist/', 'node_modules/', '.harness-cache/', 'src/proto/', 'tests/nightly/rec/',
    // emitted from their .ts (scripts/ts_emit.ts): the .ts is linted
    'tests/harness/coverage/hook.js', 'tests/harness/pageapp/main.js', 'tests/harness/paths/hook.js'] },
  js.configs.recommended,
  {
    files: ['**/*.ts', '**/*.tsx', '**/*.mts'],
    extends: [...tseslint.configs.strictTypeChecked],
    languageOptions: {
      parserOptions: {
        projectService: true,
        tsconfigRootDir: import.meta.dirname,
      },
    },
    rules: {
      '@typescript-eslint/restrict-template-expressions': ['error', { allowNumber: true }],
    },
  },
  {
    // the harness and its checks load under node's strip-only types (pnpm test:land sets
    // NODE_OPTIONS=--experimental-strip-types): only erasable syntax there, tsc's erasableSyntaxOnly.
    // A parameter property in tests/harness/isolation.ts made test:land load 0 tests (land-ts).
    files: ['tests/harness/**/*.ts', 'tests/harness/**/*.mts', 'tests/checks/**/*.ts', 'tests/checks/**/*.mts'],
    rules: {
      '@typescript-eslint/parameter-properties': ['error', { prefer: 'class-property' }],
      'no-restricted-syntax': ['error',
        { selector: 'TSEnumDeclaration', message: 'no enum in the harness: node strips types only (test:land); use a const object or a union' },
        { selector: 'TSModuleDeclaration[declare!=true][kind!="global"]', message: 'no namespace in the harness: node strips types only (test:land)' },
        { selector: 'TSImportEqualsDeclaration', message: 'no `import x = require()` in the harness: node strips types only (test:land)' },
        { selector: 'TSExportAssignment', message: 'no `export =` in the harness: node strips types only (test:land)' },
      ],
    },
  },
  {
    // a hook may live in any .ts file too (composer/pm/dictation.ts, shared/partial.ts, capsule/data.ts…)
    files: ['src/**/*.tsx', 'src/**/*.ts'],
    extends: [reactHooks.configs.flat['recommended-latest']],
    languageOptions: { globals: globals.browser },
  },
  {
    // a feature test (fast-try p5, architect m_29407) reads only what a screen shows and the
    // commands sent: it imports the helpers' inputs and runner, never the app's internals; no
    // snapshot, no fake, no mock
    files: ['tests/feature/**/*.test.ts'],
    rules: {
      'no-restricted-imports': ['error', {
        patterns: [
          { regex: '^(?!vitest$|\\./inputs$|\\./run$)', message: 'a feature test imports vitest, ./inputs and ./run only (tests/feature/run.ts)' },
        ],
      }],
      'no-restricted-syntax': ['error',
        { selector: "CallExpression[callee.property.name=/^(toMatchSnapshot|toMatchInlineSnapshot|toMatchFileSnapshot)$/]", message: 'no snapshot in a feature test: say what the screen shows' },
        { selector: "MemberExpression[object.name='vi']", message: 'no fake or mock in a feature test: give typed inputs (./inputs)' },
      ],
    },
  },
  {
    // the nightly invariants on a real core go through the same runner: the same rules, plus the
    // window-local events main builds (src/shared/local.ts)
    files: ['tests/nightly/*.test.ts'],
    rules: {
      'no-restricted-imports': ['error', {
        patterns: [
          { regex: '^(?!vitest$|\\.\\./feature/inputs$|\\.\\./feature/run$|@/shared/local$)', message: 'a nightly check imports vitest, ../feature/inputs, ../feature/run and @/shared/local only' },
        ],
      }],
      'no-restricted-syntax': ['error',
        { selector: "CallExpression[callee.property.name=/^(toMatchSnapshot|toMatchInlineSnapshot|toMatchFileSnapshot)$/]", message: 'no snapshot in a nightly check: invariants only' },
        { selector: "MemberExpression[object.name='vi']", message: 'no fake or mock in a nightly check: the recording is the input' },
      ],
    },
  },
  {
    files: ['**/*.mjs', '**/*.js'],
    languageOptions: { globals: globals.node },
  },
);
