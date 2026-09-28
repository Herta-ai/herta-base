#!/usr/bin/env node
// Cross-platform validation. Reports never turn skipped or unavailable checks into passes.
import { spawnSync } from 'node:child_process';
import { closeSync, mkdirSync, openSync, readFileSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const arguments_ = process.argv.slice(2);
const runtimeOnly = arguments_.includes('--runtime');
if (arguments_.some(value => !['--runtime', '--list'].includes(value))) {
  throw new Error('Usage: node scripts/verify.mjs [--runtime] [--list]');
}
const members = ['herta_core', 'herta_db', 'herta_jsvm', 'herta_auth', 'herta_storage', 'herta_mail', 'herta_http', 'herta_api', 'herta_server'];
const packages = runtimeOnly ? members.slice(0, 3) : members;
const selection = runtimeOnly ? packages.flatMap(name => ['-p', name]) : ['--workspace'];
// SurrealDB test binaries are large; bound compiler concurrency on developer machines.
// Operators can still opt into more jobs with CARGO_BUILD_JOBS.
const buildJobs = process.env.CARGO_BUILD_JOBS ?? '1';
const checks = [
  ['rust-format', 'cargo', ['fmt', ...packages.flatMap(name => ['-p', name]), '--', '--check']],
  ['rust-check', 'cargo', ['check', ...selection, '--locked']],
  ['rust-clippy', 'cargo', ['clippy', ...selection, '--all-targets', '--locked', '--', '-D', 'warnings']],
  ['rust-tests', 'cargo', ['test', ...selection, '--locked']],
];
if (!runtimeOnly) {
  checks.push(
    ['release', 'cargo', ['build', '--release', '--locked']],
    ['lifecycle', process.platform === 'win32' ? 'python' : 'python3', ['scripts/verify-lifecycle.py', '--binary',
      join('target', 'release', process.platform === 'win32' ? 'hertabase.exe' : 'hertabase')]],
    ['sdk-lint', 'pnpm', ['--filter', '@hb/sdk', 'lint']],
    ['sdk-types', 'pnpm', ['--filter', '@hb/sdk', 'typecheck']],
    ['sdk-tests', 'pnpm', ['--filter', '@hb/sdk', 'test']],
    ['extension-types', 'pnpm', ['--filter', '@hb/types', 'typecheck']],
    ['integration', 'pnpm', ['test:integration']],
  );
}
if (arguments_.includes('--list')) {
  for (const [name, command, args] of checks) process.stdout.write(`${name}: ${command} ${args.join(' ')}\n`);
  process.exit(0);
}
const startedAt = new Date().toISOString();
const destination = join(root, 'target', 'verification', `${process.platform}-${process.arch}-${startedAt.replaceAll(':', '-')}`);
mkdirSync(destination, { recursive: true });
const report = {
  platform: process.platform, architecture: process.arch, node: process.version, startedAt,
  scope: runtimeOnly ? 'runtime' : 'full', status: 'running',
  otherPlatforms: ['win32', 'linux', 'darwin'].filter(platform => platform !== process.platform)
    .map(platform => ({ platform, status: 'not_run' })),
  checks: checks.map(([name, command, args]) => ({ name, command, args, status: 'not_run' })),
};
const save = () => writeFileSync(join(destination, 'report.json'), `${JSON.stringify(report, null, 2)}\n`);
save();
for (const check of report.checks) {
  check.status = 'running'; check.startedAt = new Date().toISOString(); save();
  process.stdout.write(`Running ${check.name}\n`);
  const start = performance.now();
  const log = join(destination, `${check.name}.log`);
  check.log = log;
  const fd = openSync(log, 'w');
  try {
    if (check.name === 'extension-types') {
      const pkg = JSON.parse(readFileSync(join(root, 'packages/types/package.json'), 'utf8'));
      if (!pkg.scripts?.typecheck) throw new Error('@hb/types has no type declaration test entry yet');
    }
    const command = check.command === 'pnpm' && process.platform === 'win32' ? 'pnpm.cmd' : check.command;
    const result = spawnSync(command, check.args, {
      cwd: root, env: { ...process.env, CARGO_BUILD_JOBS: buildJobs }, stdio: ['ignore', fd, fd],
      // Only fixed pnpm arguments above go through the Windows command shim.
      shell: command === 'pnpm.cmd', windowsHide: true,
    });
    check.exitCode = result.status;
    check.status = result.status === 0 && !result.error ? 'passed' : 'failed';
    if (result.error) check.error = result.error.message;
    if (result.signal) check.signal = result.signal;
  } catch (error) {
    check.status = 'failed'; check.error = String(error);
    writeFileSync(fd, `${error}\n`);
  } finally {
    closeSync(fd);
    check.durationMs = Math.round(performance.now() - start);
    save();
  }
  process.stdout.write(`${check.name}: ${check.status}\n`);
}
report.finishedAt = new Date().toISOString();
report.status = report.checks.every(check => check.status === 'passed') ? 'passed' : 'failed';
save();
process.stdout.write(`Report: ${join(destination, 'report.json')}\n`);
process.exitCode = report.status === 'passed' ? 0 : 1;
