#!/usr/bin/env python3
"""Real-process lifecycle regression. Uses only Python's standard library.

Windows starts a hidden, dedicated console and delivers CTRL_BREAK through an
isolated helper. Unix delivers SIGTERM. Neither success path uses force-kill.
"""
import argparse
import concurrent.futures
import ctypes
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import sys
import tempfile
import time
import urllib.error
import urllib.request


def windows_signal(pid):
    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.FreeConsole()
    if not kernel.AttachConsole(pid):
        raise ctypes.WinError(ctypes.get_last_error())
    handler_type = ctypes.WINFUNCTYPE(ctypes.c_bool, ctypes.c_uint)
    handler = handler_type(lambda event: True)
    if not kernel.SetConsoleCtrlHandler(handler, True):
        raise ctypes.WinError(ctypes.get_last_error())
    if not kernel.GenerateConsoleCtrlEvent(1, pid):  # CTRL_BREAK_EVENT
        raise ctypes.WinError(ctypes.get_last_error())
    time.sleep(0.05)
    kernel.FreeConsole()


SCRIPT = r"""
async function count(name) {
  const n = await $app.files.exists(name) ? Number(await $app.files.readText(name)) : 0;
  await $app.files.write(name, String(n + 1));
}
onBootstrap(async e => {
  await count('bootstrap-count');
  const previous = await $app.files.exists('state') ? await $app.files.readText('state') : 'new';
  await $app.files.write('previous-state', previous);
  await $app.files.write('state', 'bootstrap');
  $app.logger.info('LIFECYCLE_BOOTSTRAP');
  const until = Date.now() + 750;
  while (Date.now() < until) {}
  await e.next();
});
onServe(async e => {
  $app.logger.info('LIFECYCLE_SERVE_BEGIN');
  const until = Date.now() + 750;
  while (Date.now() < until) {}
  if (await $app.files.readText('state') !== 'bootstrap') throw new Error('serve before bootstrap');
  await count('serve-count');
  await $app.files.write('state', 'serving');
  await e.next();
});
onShutdown(async e => {
  if (await $app.files.readText('state') !== 'serving') throw new Error('shutdown before serving');
  if (await $app.files.exists('slow-start') && !await $app.files.exists('slow-end')) throw new Error('request was not drained');
  await count('shutdown-count');
  await $app.files.write('state', 'stopped');
  $app.logger.info('LIFECYCLE_SHUTDOWN');
  await e.next();
});
routerAdd('GET', '/api/lifecycle', async e => e.json(200, {
  version: 'v1', state: await $app.files.readText('state'),
  previous: await $app.files.readText('previous-state'),
  bootstrap: await $app.files.readText('bootstrap-count'),
  serve: await $app.files.readText('serve-count'),
  shutdown: await $app.files.exists('shutdown-count') ? await $app.files.readText('shutdown-count') : '0'
}));
routerAdd('GET', '/api/lifecycle/slow', async e => {
  await $app.files.write('slow-start', 'started');
  $app.logger.info('LIFECYCLE_SLOW_START');
  const until = Date.now() + 750;
  while (Date.now() < until) {}
  await $app.files.write('slow-end', 'done');
  return e.noContent();
});
"""


def eventually(check, process, log, seconds=15):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        if process.poll() is not None:
            raise AssertionError(f"server exited {process.returncode}:\n{log.read_text(errors='replace')}")
        try:
            result = check()
            if result:
                return result
        except (urllib.error.URLError, TimeoutError, ConnectionError):
            pass
        time.sleep(0.03)
    raise AssertionError(f"condition timed out:\n{log.read_text(errors='replace')}")


def run(binary):
    with tempfile.TemporaryDirectory(prefix="hb-lifecycle-") as temp:
        root = Path(temp)
        hooks = root / "hooks"
        hooks.mkdir()
        source = hooks / "main.js"
        source.write_text(SCRIPT, encoding="utf-8")
        with socket.socket() as reservation:
            reservation.bind(("127.0.0.1", 0))
            port = reservation.getsockname()[1]
        config = root / "hb.toml"
        config.write_text("""
[server]
dev_mode = true
[database]
engine = "surrealkv"
[auth]
jwt_secret = "lifecycle-test-only-secret-at-least-32-bytes"
[jsvm]
enabled = true
pool_size = 1
execution_timeout_ms = 2000
async_timeout_ms = 5000
shutdown_timeout_ms = 3000
[jsvm.files]
enabled = true
[jsvm.cron]
enabled = false
""", encoding="utf-8")
        environment = {k: v for k, v in os.environ.items() if not k.startswith("HB_")}
        environment["RUST_LOG"] = "info"
        opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
        url = f"http://127.0.0.1:{port}/api/lifecycle"

        def request(path=""):
            with opener.open(url + path, timeout=5) as response:
                return response.status if response.status == 204 else json.load(response)["data"]

        for restart in range(2):
            source.write_text(SCRIPT if restart == 0 else SCRIPT.replace("'v1'", "'v2'"), encoding="utf-8")
            log = root / f"server-{restart}.log"
            flags = {}
            if os.name == "nt":
                startup = subprocess.STARTUPINFO()
                startup.dwFlags |= subprocess.STARTF_USESHOWWINDOW
                startup.wShowWindow = 0  # SW_HIDE
                flags = {"startupinfo": startup, "creationflags": subprocess.CREATE_NEW_CONSOLE | subprocess.CREATE_NEW_PROCESS_GROUP}
            with log.open("wb") as output:
                process = subprocess.Popen([str(binary), "--config", str(config), "serve", "--host", "127.0.0.1",
                    "--port", str(port), "--data-dir", str(root / "data"), "--hooks-dir", str(hooks)],
                    stdin=subprocess.DEVNULL, stdout=output, stderr=output, env=environment, **flags)
                try:
                    eventually(lambda: "LIFECYCLE_BOOTSTRAP" in log.read_text(errors="replace"), process, log)
                    with socket.socket() as probe:
                        probe.settimeout(0.1)
                        assert probe.connect_ex(("127.0.0.1", port)) != 0, "listener was bound during bootstrap"
                    eventually(lambda: "LIFECYCLE_SERVE_BEGIN" in log.read_text(errors="replace"), process, log)
                    with socket.create_connection(("127.0.0.1", port), timeout=0.2) as probe:
                        probe.sendall(b"GET /api/lifecycle HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
                        probe.settimeout(0.05)
                        try:
                            probe.recv(1)
                            raise AssertionError("request accepted before serve hook completed")
                        except socket.timeout:
                            pass
                    data = eventually(request, process, log)
                    assert data["state"] == "serving", data
                    assert data["bootstrap"] == str(restart + 1), data
                    assert data["serve"] == str(restart + 1), data
                    assert data["shutdown"] == str(restart), data
                    assert data["previous"] == ("new" if restart == 0 else "stopped"), data
                    if restart == 0:
                        source.write_text(SCRIPT.replace("'v1'", "'v2'"), encoding="utf-8")
                        eventually(lambda: request()["version"] == "v2", process, log)
                        source.write_text("throw new SyntaxError('failed reload');", encoding="utf-8")
                        eventually(lambda: "extension reload rejected" in log.read_text(errors="replace"), process, log)
                        data = request()
                        assert data["version"] == "v2" and data["bootstrap"] == "1" and data["serve"] == "1", data
                    with concurrent.futures.ThreadPoolExecutor(max_workers=1) as executor:
                        pending = executor.submit(request, "/slow")
                        eventually(lambda: "LIFECYCLE_SLOW_START" in log.read_text(errors="replace"), process, log)
                        if os.name == "nt":
                            subprocess.run([sys.executable, __file__, "--windows-signal", str(process.pid)], check=True, timeout=5)
                        else:
                            process.send_signal(signal.SIGTERM)
                        assert pending.result(timeout=6) == 204
                    assert process.wait(timeout=8) == 0, log.read_text(errors="replace")
                    assert "LIFECYCLE_SHUTDOWN" in log.read_text(errors="replace")
                finally:
                    if process.poll() is None:
                        process.kill()  # Failure cleanup only; never counts as graceful shutdown.
                        process.wait(timeout=5)
        print(json.dumps({"platform": sys.platform, "status": "passed", "checks": [
            "bootstrap before binding", "serve after binding before accepting", "reload does not repeat lifecycle", "failed reload keeps snapshot",
            "native graceful signal", "in-flight request drain", "shutdown retains host services", "durable shutdown across restart"]}))


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--windows-signal", type=int)
    args = parser.parse_args()
    if args.windows_signal:
        windows_signal(args.windows_signal)
    else:
        binary = args.binary or Path(__file__).resolve().parent.parent / "target" / "debug" / ("hertabase.exe" if os.name == "nt" else "hertabase")
        run(binary.resolve(strict=True))
