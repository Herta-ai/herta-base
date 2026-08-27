import { spawn, ChildProcess } from 'child_process';
import net from 'net';
import path from 'path';
import fs from 'fs';
import os from 'os';
import { fileURLToPath } from 'url';

let serverProcess: ChildProcess | null = null;
let serverUrlValue: string | null = null;
let dataDirValue: string | null = null;

export function serverUrl(): string {
  if (!serverUrlValue) {
    throw new Error('server has not been started');
  }
  return serverUrlValue;
}

async function freePort(): Promise<number> {
  return new Promise((resolve, reject) => {
    const probe = net.createServer();
    probe.once('error', reject);
    probe.listen(0, '127.0.0.1', () => {
      const address = probe.address();
      if (!address || typeof address === 'string') {
        probe.close();
        reject(new Error('could not determine an available port'));
        return;
      }
      probe.close((error) => (error ? reject(error) : resolve(address.port)));
    });
  });
}

function binaryPath(workspaceRoot: string): string {
  const profile = process.env.HB_TEST_PROFILE
    ?? (process.env.npm_lifecycle_event === 'test:release' ? 'release' : 'debug');
  if (profile !== 'debug' && profile !== 'release') {
    throw new Error(`HB_TEST_PROFILE must be debug or release, received '${profile}'`);
  }
  const name = process.platform === 'win32' ? 'hertabase.exe' : 'hertabase';
  return path.join(workspaceRoot, 'target', profile, name);
}

async function waitForReady(url: string, output: () => string): Promise<void> {
  const deadline = Date.now() + 20_000;
  let lastError = 'no response';
  while (Date.now() < deadline) {
    try {
      const response = await fetch(`${url}/api-doc/openapi.json`);
      if (response.ok) {
        return;
      }
      lastError = `HTTP ${response.status}`;
    } catch (error) {
      lastError = error instanceof Error ? error.message : String(error);
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  throw new Error(`server did not become ready (${lastError})\n${output()}`);
}

export async function startServer(): Promise<string> {
  if (serverProcess) {
    return serverUrl();
  }

  const workspaceRoot = fileURLToPath(new URL('../../../../', import.meta.url));
  const executable = binaryPath(workspaceRoot);
  if (!fs.existsSync(executable)) {
    throw new Error(`missing ${executable}; run the integration package pretest first`);
  }
  const port = await freePort();
  const url = `http://127.0.0.1:${port}`;
  const dataDir = fs.mkdtempSync(path.join(os.tmpdir(), 'hertabase-blog-'));
  dataDirValue = dataDir;
  const env = {
    ...process.env,
    HB_BOOTSTRAP_ADMIN_EMAIL: 'admin@example.com',
    HB_BOOTSTRAP_ADMIN_PASSWORD: 'correct horse battery staple',
  };

  let stdout = '';
  let stderr = '';
  let settled = false;
  serverProcess = spawn(executable, [
    'serve',
    '--db-engine',
    'memory',
    '--dev',
    '--data-dir',
    dataDir,
    '--host',
    '127.0.0.1',
    '--port',
    String(port),
  ], {
    cwd: workspaceRoot,
    env,
    stdio: ['ignore', 'pipe', 'pipe'],
    detached: process.platform !== 'win32',
  });
  const processRef = serverProcess;
  processRef.stdout?.on('data', (data: Buffer) => { stdout += data.toString(); });
  processRef.stderr?.on('data', (data: Buffer) => { stderr += data.toString(); });
  const output = () => `stdout:\n${stdout}\nstderr:\n${stderr}`;
  const earlyExit = new Promise<never>((_, reject) => {
    processRef.once('error', (error) => reject(error));
    processRef.once('exit', (code, signal) => {
      if (!settled) {
        reject(new Error(`server exited before readiness (code=${code}, signal=${signal})\n${output()}`));
      }
    });
  });

  try {
    await Promise.race([waitForReady(url, output), earlyExit]);
    settled = true;
    serverUrlValue = url;
    return url;
  } catch (error) {
    settled = true;
    await stopServer();
    throw error;
  }
}

export async function stopServer(): Promise<void> {
  const processRef = serverProcess;
  const dataDir = dataDirValue;
  serverProcess = null;
  serverUrlValue = null;
  dataDirValue = null;
  if (!processRef || processRef.pid === undefined) {
    if (dataDir) {
      fs.rmSync(dataDir, { recursive: true, force: true });
    }
    return;
  }
  if (processRef.exitCode !== null || processRef.signalCode !== null) {
    if (dataDir) {
      fs.rmSync(dataDir, { recursive: true, force: true });
    }
    return;
  }

  if (process.platform === 'win32') {
    spawn('taskkill', ['/pid', String(processRef.pid), '/T', '/F'], { stdio: 'ignore' });
  } else {
    try {
      process.kill(-processRef.pid, 'SIGTERM');
    } catch {
      processRef.kill('SIGTERM');
    }
  }
  await new Promise<void>((resolve) => {
    const timer = setTimeout(() => {
      if (processRef.exitCode === null && processRef.signalCode === null) {
        processRef.kill('SIGKILL');
      }
      resolve();
    }, 5_000);
    processRef.once('exit', () => {
      clearTimeout(timer);
      resolve();
    });
  });
  if (dataDir) {
    fs.rmSync(dataDir, { recursive: true, force: true });
  }
}
