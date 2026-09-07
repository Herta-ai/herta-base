import test from "node:test";
import assert from "node:assert/strict";
import { startMailbox } from "@hb/mailbox";
import { freePort, startServer, post, login } from "./server.js";

const endpoint = "/api/admin/mail/send";
const message = () => ({
  to: [{ address: "reader@example.com", name: "读者" }, { address: "second@example.com" }],
  subject: "HertaBase 邮件链路测试",
  text: "你好，邮件已到达。",
  html: "<strong>你好，邮件已到达。</strong>",
  headers: { "X-Event-Id": "mail-integration-123" },
});
const inbox = (box) => fetch(`${box.url}/api/messages`).then((response) => response.json());

test(
  "administrator HTTP → Rust → SMTP → captured MIME, auth and validation",
  { timeout: 60_000 },
  async (t) => {
    const box = await startMailbox({ smtpPort: 0, httpPort: 0 });
    t.after(() => box.close());
    const server = await startServer({ HB_SMTP_PORT: String(box.smtpPort) });
    t.after(() => server.close());
    const token = await login(server);
    const openapi = await fetch(`${server.url}/api-doc/openapi.json`).then((r) => r.json());
    assert.ok(openapi.paths[endpoint].post);
    assert.equal((await post(server, endpoint, message())).status, 401);
    const registered = await post(server, "/api/auth/register", {
      email: "user@example.com",
      password: "correct horse battery staple",
    });
    assert.equal(registered.status, 201);
    assert.equal(
      (await post(server, endpoint, message(), registered.body.data.accessToken)).status,
      403,
    );
    for (const invalid of [
      { ...message(), subject: "subject\r\nBcc: victim@example.com" },
      { ...message(), headers: { Bcc: "victim@example.com" } },
      { ...message(), headers: { "你-Header": "value" } },
      { ...message(), from: { address: "other@example.com" } },
      { ...message(), to: [] },
      { ...message(), bcc: [] },
    ])
      assert.equal((await post(server, endpoint, invalid, token)).status, 400);
    assert.equal((await inbox(box)).total, 0);

    const sent = await post(server, endpoint, message(), token);
    assert.equal(sent.status, 200, JSON.stringify(sent.body));
    assert.equal(sent.body.data.status, "accepted");
    const receipt = sent.body.data;
    const query = new URLSearchParams({
      messageId: receipt.messageId,
      recipient: "reader@example.com",
    });
    const matches = await fetch(`${box.url}/api/messages?${query}`).then((r) => r.json());
    assert.equal(matches.total, 1);
    const mail = await fetch(`${box.url}/api/messages/${matches.messages[0].id}`).then((r) =>
      r.json(),
    );
    assert.equal(mail.messageId, receipt.messageId);
    assert.equal(mail.subject, message().subject);
    assert.equal(mail.text.trim(), message().text);
    assert.equal(mail.html.trim(), message().html);
    assert.equal(mail.from[0].address, "noreply@example.com");
    assert.equal(mail.from[0].name, "HertaBase");
    assert.equal(mail.to[0].name, "读者");
    assert.deepEqual(mail.envelope, {
      from: "noreply@example.com",
      to: ["reader@example.com", "second@example.com"],
    });
    assert.equal(mail.headers["x-event-id"], "mail-integration-123");
    assert.equal((await inbox(box)).total, 1);
    for (const body of [{ text: "plain-only" }, { html: "<p>html-only</p>" }]) {
      const sent = await post(
        server,
        endpoint,
        { to: message().to, subject: "single-part", ...body },
        token,
      );
      assert.equal(sent.status, 200);
    }
    assert.equal((await inbox(box)).total, 3);
    assert.ok(!server.output().includes(message().text));
  },
);

for (const scenario of [
  {
    name: "disabled",
    env: { HB_MAIL_DRIVER: "disabled" },
    code: "HB_CAPABILITY_UNAVAILABLE",
    status: 503,
  },
  {
    name: "recipient rejection",
    behavior: { rejectRecipient: true },
    code: "HB_MAIL_SEND_FAILED",
    status: 502,
  },
  {
    name: "DATA rejection",
    behavior: { rejectData: true },
    code: "HB_MAIL_SEND_FAILED",
    status: 502,
  },
  {
    name: "timeout",
    behavior: { delayMs: 2000 },
    env: { HB_MAIL_TIMEOUT_MS: "400" },
    code: "HB_MAIL_TIMEOUT",
    status: 504,
  },
  {
    name: "STARTTLS unavailable",
    env: { HB_SMTP_TLS: "starttls" },
    code: "HB_MAIL_SEND_FAILED",
    status: 502,
  },
  {
    name: "implicit TLS cannot use plaintext",
    env: { HB_SMTP_TLS: "required", HB_MAIL_TIMEOUT_MS: "400" },
    code: "HB_MAIL_SEND_FAILED",
    status: 502,
  },
  {
    name: "authentication unavailable",
    env: { HB_SMTP_USERNAME: "private-user", HB_SMTP_PASSWORD: "private-password" },
    code: "HB_MAIL_SEND_FAILED",
    status: 502,
  },
]) {
  test(`SMTP failure: ${scenario.name}`, { timeout: 45_000 }, async (t) => {
    const box = await startMailbox({ smtpPort: 0, httpPort: 0, behavior: scenario.behavior });
    t.after(() => box.close());
    const server = await startServer({ HB_SMTP_PORT: String(box.smtpPort), ...scenario.env });
    t.after(() => server.close());
    const result = await post(server, endpoint, message(), await login(server));
    assert.equal(result.status, scenario.status, JSON.stringify(result));
    assert.equal(result.body.error.error, scenario.code);
    assert.equal((await inbox(box)).total, 0);
    for (const secret of ["private-user", "private-password"]) {
      assert.ok(!JSON.stringify(result.body).includes(secret));
      assert.ok(!server.output().includes(secret));
    }
  });
}

test("connection refused reports SMTP failure", { timeout: 45_000 }, async (t) => {
  const server = await startServer({ HB_SMTP_PORT: String(await freePort()) });
  t.after(() => server.close());
  const result = await post(server, endpoint, message(), await login(server));
  assert.equal(result.status, 502);
  assert.equal(result.body.error.error, "HB_MAIL_SEND_FAILED");
});
