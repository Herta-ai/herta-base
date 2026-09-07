import { spawn } from "node:child_process";
import { mkdtemp, writeFile, rm } from "node:fs/promises";
import { once } from "node:events";
import { tmpdir } from "node:os";
import net from "node:net";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { setTimeout as delay } from "node:timers/promises";

const root = fileURLToPath(new URL("../../../../", import.meta.url));
export const adminCredentials = {
  email: "admin@example.com",
  password: "correct horse battery staple",
};

export async function freePort() {
  const probe = net.createServer();
  probe.listen(0, "127.0.0.1");
  await once(probe, "listening");
  const port = probe.address().port;
  await new Promise((resolve) => probe.close(resolve));
  return port;
}

export async function startServer(mailEnv = {}) {
  const dataDir = await mkdtemp(path.join(tmpdir(), "hertabase-mail-"));
  const configFile = path.join(dataDir, "hertabase.toml");
  await writeFile(configFile, "[server]\n");
  const port = await freePort();
  const url = `http://127.0.0.1:${port}`;
  const child = spawn(
    path.join(
      root,
      "target",
      "debug",
      process.platform === "win32" ? "hertabase.exe" : "hertabase",
    ),
    [
      "--config",
      configFile,
      "serve",
      "--db-engine",
      "memory",
      "--dev",
      "--data-dir",
      dataDir,
      "--host",
      "127.0.0.1",
      "--port",
      String(port),
    ],
    {
      cwd: root,
      windowsHide: true,
      detached: process.platform !== "win32",
      stdio: ["ignore", "pipe", "pipe"],
      env: {
        ...Object.fromEntries(
          Object.entries(process.env).filter(([name]) => !name.startsWith("HB_")),
        ),
        HB_BOOTSTRAP_ADMIN_EMAIL: adminCredentials.email,
        HB_BOOTSTRAP_ADMIN_PASSWORD: adminCredentials.password,
        HB_JWT_SECRET: "mail-integration-fixed-jwt-secret-2026",
        HB_LOG_SERVER_PERSIST_ENABLED: "false",
        HB_LOG_HTTP_PERSIST_ENABLED: "false",
        HB_MAIL_DRIVER: "smtp",
        HB_SMTP_HOST: "127.0.0.1",
        HB_SMTP_TLS: "none",
        HB_MAIL_FROM_ADDRESS: "noreply@example.com",
        ...mailEnv,
      },
    },
  );
  let output = "";
  let startupError;
  child.stdout.on("data", (chunk) => {
    output = (output + chunk).slice(-30_000);
  });
  child.stderr.on("data", (chunk) => {
    output = (output + chunk).slice(-30_000);
  });
  child.on("error", (error) => {
    startupError = error;
  });
  const exited = new Promise((resolve) => child.once("close", resolve));
  let closed;
  function close() {
    closed ??= (async () => {
      if (child.pid && child.exitCode === null && child.signalCode === null) {
        if (process.platform === "win32") {
          const killer = spawn("taskkill", ["/pid", String(child.pid), "/T", "/F"], {
            windowsHide: true,
            stdio: "ignore",
          });
          await new Promise((resolve) => {
            killer.once("error", resolve);
            killer.once("close", resolve);
          });
        } else {
          try {
            process.kill(-child.pid, "SIGTERM");
          } catch {
            child.kill("SIGTERM");
          }
        }
      }
      const timer = setTimeout(() => child.kill("SIGKILL"), 5000);
      try {
        await exited;
      } finally {
        clearTimeout(timer);
      }
      if (
        path.dirname(dataDir) !== path.resolve(tmpdir()) ||
        !path.basename(dataDir).startsWith("hertabase-mail-")
      ) {
        throw new Error("Refusing cleanup outside the owned temporary test directory");
      }
      await rm(dataDir, { recursive: true, force: true });
    })();
    return closed;
  }
  try {
    const deadline = Date.now() + 30_000;
    while (Date.now() < deadline) {
      if (startupError) throw startupError;
      if (child.exitCode !== null) throw new Error(`HertaBase exited: ${output}`);
      try {
        if (
          (await fetch(`${url}/api-doc/openapi.json`, { signal: AbortSignal.timeout(1000) })).ok
        ) {
          return { url, close, output: () => output };
        }
      } catch {
        /* Await startup within the deadline. */
      }
      await delay(100);
    }
    throw new Error(`HertaBase startup timed out: ${output}`);
  } catch (error) {
    await close();
    throw error;
  }
}

export async function post(server, endpoint, body, token) {
  const response = await fetch(`${server.url}${endpoint}`, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      ...(token ? { authorization: `Bearer ${token}` } : {}),
    },
    body: JSON.stringify(body),
    signal: AbortSignal.timeout(15_000),
  });
  return { status: response.status, body: await response.json() };
}

export async function login(server) {
  const result = await post(server, "/api/admin/auth/login", adminCredentials);
  if (result.status !== 200) throw new Error(`Admin login failed: ${JSON.stringify(result)}`);
  return result.body.data.accessToken;
}
