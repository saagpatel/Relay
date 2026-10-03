import { execSync } from 'node:child_process';
import { pathToFileURL } from 'node:url';

export function policyFailures(changed) {
  const isTest = (file) => /^tests\//.test(file) || /\.(test|spec)\.[cm]?[jt]sx?$/.test(file) || /_test\.go$/.test(file);
  const isGoManifest = (file) => /(^|\/)go\.(mod|sum)$/.test(file);
  const isProdCode = (file) => /^(src|app|server|api|lib)\//.test(file) && !isTest(file) && !isGoManifest(file);
  const isDoc = (file) => /^docs\//.test(file) || /^openapi\//.test(file) || file === 'README.md';

  if (!changed.some(isProdCode)) return [];
  const failures = [];
  if (!changed.some(isTest)) failures.push('Policy failure: production code changed without test updates.');
  if (!changed.some(isDoc)) failures.push('Policy failure: production code changed without docs/OpenAPI updates.');
  return failures;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const defaultBaseRef = (() => {
    try {
      return execSync('git symbolic-ref refs/remotes/origin/HEAD', { encoding: 'utf8' }).trim().replace('refs/remotes/', '');
    } catch {
      return 'origin/main';
    }
  })();

  const baseRef = process.env.GITHUB_BASE_REF ? `origin/${process.env.GITHUB_BASE_REF}` : defaultBaseRef;
  const changed = execSync(`git diff --name-only ${baseRef}...HEAD`, { encoding: 'utf8' })
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean);
  const failures = policyFailures(changed);
  if (failures.length) {
    console.error(failures[0]);
    process.exit(1);
  }
  console.log('Policy checks passed.');
}
