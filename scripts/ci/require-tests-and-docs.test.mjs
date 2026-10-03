import assert from 'node:assert/strict';
import test from 'node:test';
import { policyFailures } from './require-tests-and-docs.mjs';

const missingTests = 'Policy failure: production code changed without test updates.';
const missingDocs = 'Policy failure: production code changed without docs/OpenAPI updates.';

const cases = [
  ['Go production without tests fails', ['server/handler.go', 'README.md'], [missingTests]],
  ['Go production with Go tests and docs passes', ['server/handler.go', 'server/handler_test.go', 'docs/LOCAL_SMOKE.md'], []],
  ['Go production with tests but no docs fails', ['server/handler.go', 'server/handler_test.go'], [missingDocs]],
  ['Go tests alone are not production', ['server/handler_test.go'], []],
  ['Go dependency-only updates pass', ['server/go.mod', 'server/go.sum'], []],
  ['Go manifests do not hide production changes', ['server/go.mod', 'server/go.sum', 'server/handler.go', 'README.md'], [missingTests]],
  ['Existing JS and Rust dependency-only paths pass', ['client/package.json', 'client/pnpm-lock.yaml', 'client/src-tauri/Cargo.toml', 'client/src-tauri/Cargo.lock'], []],
  ['Existing JS production guard still fails without tests', ['src/example.ts', 'README.md'], [missingTests]],
  ['Existing JS production with tests and docs passes', ['src/example.ts', 'src/example.test.ts', 'README.md'], []],
  ['Actual production without tests or docs fails both requirements', ['server/handler.go'], [missingTests, missingDocs]],
];

for (const [name, changed, expected] of cases) {
  test(name, () => assert.deepEqual(policyFailures(changed), expected));
}
