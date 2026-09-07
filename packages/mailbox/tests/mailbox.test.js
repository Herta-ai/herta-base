import test from "node:test";
import assert from "node:assert/strict";
import net from "node:net";
import { once } from "node:events";
import { createInterface } from "node:readline";
import { startMailbox, previewHtml } from "../src/server.js";

async function send(port, raw, recipient = "reader@example.com") {
  const socket = net.createConnection({ host: "127.0.0.1", port });
  const reader = createInterface({ input: socket, crlfDelay: Infinity });
  const lines = reader[Symbol.asyncIterator]();
  socket.setTimeout(5000, () => socket.destroy(new Error("SMTP test timed out")));
  async function response() {
    for (;;) {
      const { value, done } = await lines.next();
      if (done) throw new Error("SMTP closed unexpectedly");
      if (/^\d{3} /.test(value)) return Number(value.slice(0, 3));
    }
  }
  async function command(text) {
    socket.write(`${text}\r\n`);
    return response();
  }
  try {
    await once(socket, "connect");
    assert.equal(await response(), 220);
    assert.equal(await command("EHLO localhost"), 250);
    assert.equal(await command("MAIL FROM:<sender@example.com>"), 250);
    assert.equal(await command(`RCPT TO:<${recipient}>`), 250);
    assert.equal(await command("DATA"), 354);
    socket.write(raw.replace(/\r?\n/g, "\r\n").replace(/^\./gm, "..") + "\r\n.\r\n");
    const code = await response();
    await command("QUIT");
    return code;
  } finally {
    reader.close();
    socket.destroy();
  }
}

const raw = (id, text = "Hello") =>
  `From: sender@example.com\r\nTo: reader@example.com\r\nSubject: ${id}\r\nMessage-ID: <${id}@example.com>\r\n\r\n${text}`;
const list = (box, query = "") =>
  fetch(`${box.url}/api/messages${query}`).then((response) => response.json());

test("SMTP capture, exact filters, raw download, deletion and restart", async (t) => {
  const box = await startMailbox({ smtpPort: 0, httpPort: 0 });
  t.after(() => box.close());
  assert.equal(await send(box.smtpPort, raw("one", ".dot-stuffed\r\n正文")), 250);
  assert.equal(await send(box.smtpPort, raw("two"), "second@example.com"), 250);
  assert.equal((await list(box)).total, 2);
  const result = await list(
    box,
    "?recipient=READER%40example.com&messageId=%3Cone%40example.com%3E",
  );
  assert.equal(result.total, 1);
  const id = result.messages[0].id;
  const mail = await fetch(`${box.url}/api/messages/${id}`).then((r) => r.json());
  assert.equal(mail.envelope.from, "sender@example.com");
  assert.deepEqual(mail.envelope.to, ["reader@example.com"]);
  assert.match(mail.text, /^\.dot-stuffed/);
  const download = await fetch(`${box.url}/api/messages/${id}/raw`);
  assert.match(download.headers.get("content-disposition"), /attachment/);
  assert.match(await download.text(), /Message-ID: <one@example.com>/);
  assert.equal((await fetch(`${box.url}/api/messages/${id}`, { method: "DELETE" })).status, 200);
  assert.equal((await fetch(`${box.url}/api/messages/${id}`)).status, 404);
  await fetch(`${box.url}/api/messages`, { method: "DELETE" });
  assert.equal((await list(box)).storedBytes, 0);
  await send(box.smtpPort, raw("before-restart"));
  await box.close();
  const restarted = await startMailbox({ smtpPort: box.smtpPort, httpPort: box.httpPort });
  t.after(() => restarted.close());
  assert.equal((await list(restarted)).total, 0);
});

test("capacity evicts oldest messages and rejects oversized SMTP DATA", async (t) => {
  const box = await startMailbox({
    smtpPort: 0,
    httpPort: 0,
    maxMessages: 2,
    maxMessageBytes: 512,
    maxTotalBytes: 1024,
  });
  t.after(() => box.close());
  for (const id of ["one", "two", "three"]) assert.equal(await send(box.smtpPort, raw(id)), 250);
  assert.deepEqual(
    (await list(box)).messages.map((mail) => mail.subject),
    ["three", "two"],
  );
  assert.equal(await send(box.smtpPort, raw("large", "x".repeat(1024))), 552);
  assert.equal((await list(box)).total, 2);
  const bytesBox = await startMailbox({
    smtpPort: 0,
    httpPort: 0,
    maxMessages: 100,
    maxMessageBytes: 512,
    maxTotalBytes: 260,
  });
  t.after(() => bytesBox.close());
  for (const id of ["one", "two", "three"])
    assert.equal(await send(bytesBox.smtpPort, raw(id)), 250);
  const result = await list(bytesBox);
  assert.ok(result.storedBytes <= 260);
  assert.equal(result.messages[0].subject, "three");
  assert.ok(result.total < 3);
});

test("preview removes active content, navigation and remote resources", async (t) => {
  const source =
    '<meta http-equiv="refresh" content="0;url=https://example.com"><script>alert(1)</script><img src="https://example.com/pixel"><a href="https://example.com">link</a><p onclick="alert(1)"><strong>Hello</strong></p>';
  const preview = previewHtml(source);
  assert.match(preview, /<strong>Hello<\/strong>/);
  assert.doesNotMatch(preview, /<script|<img|onclick|http-equiv="refresh"|href=|src=/);
  const box = await startMailbox({ smtpPort: 0, httpPort: 0 });
  t.after(() => box.close());
  await send(
    box.smtpPort,
    `From: sender@example.com\r\nTo: reader@example.com\r\nSubject: HTML\r\nContent-Type: text/html; charset=utf-8\r\n\r\n${source}`,
  );
  const id = (await list(box)).messages[0].id;
  const response = await fetch(`${box.url}/api/messages/${id}/preview`);
  assert.match(response.headers.get("content-security-policy"), /sandbox/);
  assert.match(response.headers.get("content-security-policy"), /default-src 'none'/);
  assert.doesNotMatch(await response.text(), /<img|<script|onclick/);
  const page = await fetch(box.url).then((r) => r.text());
  assert.match(page, /sandbox/);
  assert.equal(
    (
      await fetch(`${box.url}/api/messages`, {
        method: "DELETE",
        headers: { Origin: "https://example.com" },
      })
    ).status,
    403,
  );
  assert.equal((await list(box)).total, 1);
});

test("failed startup releases its SMTP port", async (t) => {
  const occupied = net.createServer();
  occupied.listen(0, "127.0.0.1");
  await once(occupied, "listening");
  t.after(() => new Promise((resolve) => occupied.close(resolve)));
  await assert.rejects(
    startMailbox({ smtpPort: 0, httpPort: occupied.address().port }),
    /EADDRINUSE/,
  );
});
