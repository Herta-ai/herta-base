import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { randomUUID } from "node:crypto";
import { setTimeout as delay } from "node:timers/promises";
import { SMTPServer } from "smtp-server";
import { simpleParser } from "mailparser";
import sanitizeHtml from "sanitize-html";

const assets = {
  "/": ["index.html", "text/html; charset=utf-8"],
  "/app.js": ["app.js", "text/javascript; charset=utf-8"],
  "/style.css": ["style.css", "text/css; charset=utf-8"],
};

const previewPolicy =
  "default-src 'none'; style-src 'unsafe-inline'; base-uri 'none'; form-action 'none'; sandbox";

export function previewHtml(html) {
  // Remove navigation and active content as well as enforcing CSP/sandbox.
  const clean = sanitizeHtml(html, {
    allowedTags: [
      "p",
      "br",
      "div",
      "span",
      "b",
      "strong",
      "i",
      "em",
      "u",
      "s",
      "h1",
      "h2",
      "h3",
      "h4",
      "ul",
      "ol",
      "li",
      "blockquote",
      "pre",
      "code",
      "hr",
      "table",
      "thead",
      "tbody",
      "tr",
      "th",
      "td",
    ],
    allowedAttributes: {
      "*": ["style"],
      td: ["colspan", "rowspan", "style"],
      th: ["colspan", "rowspan", "style"],
    },
  });
  return `<!doctype html><html><head><meta charset="utf-8"><meta http-equiv="Content-Security-Policy" content="${previewPolicy}"></head><body>${clean}</body></html>`;
}

function smtpError(message, responseCode) {
  return Object.assign(new Error(message), { responseCode });
}

function listen(server, port, host) {
  return new Promise((resolve, reject) => {
    const failed = (error) => reject(error);
    server.once("error", failed);
    server.listen(port, host, () => {
      server.removeListener("error", failed);
      resolve();
    });
  });
}

/** Local capture only. behavior is an injectable fixture, never an HTTP control. */
export async function startMailbox({
  smtpPort = 1025,
  httpPort = 8025,
  maxMessageBytes = 2 * 1024 * 1024,
  maxMessages = 100,
  maxTotalBytes = 32 * 1024 * 1024,
  behavior = {},
} = {}) {
  for (const [name, value] of Object.entries({ maxMessageBytes, maxMessages, maxTotalBytes })) {
    if (!Number.isSafeInteger(value) || value <= 0)
      throw new Error(`${name} must be a positive integer`);
  }
  for (const port of [smtpPort, httpPort]) {
    if (!Number.isInteger(port) || port < 0 || port > 65535) throw new Error("invalid port");
  }
  const messages = new Map();
  let totalBytes = 0;
  let closing = false;
  let closePromise;
  const abort = new AbortController();
  const smtpSockets = new Set();
  const httpSockets = new Set();
  const publicAssets = new Map(
    await Promise.all(
      Object.entries(assets).map(async ([route, [file, type]]) => [
        route,
        { type, body: await readFile(new URL(`../public/${file}`, import.meta.url)) },
      ]),
    ),
  );

  function remove(id) {
    const message = messages.get(id);
    if (!message) return false;
    totalBytes -= message.size;
    messages.delete(id);
    return true;
  }

  function clear() {
    messages.clear();
    totalBytes = 0;
  }
  function detail({ raw, ...message }) {
    return message;
  }

  const smtp = new SMTPServer({
    name: "hertabase-mailbox.local",
    banner: "HertaBase memory mailbox",
    authOptional: true,
    disabledCommands: ["AUTH", "STARTTLS"],
    hidePIPELINING: true,
    disableReverseLookup: true,
    logger: false,
    maxClients: 10,
    size: maxMessageBytes,
    socketTimeout: 30_000,
    onRcptTo(_address, _session, callback) {
      callback(
        behavior.rejectRecipient ? smtpError("Recipient rejected by test fixture", 550) : undefined,
      );
    },
    onData(stream, session, callback) {
      let size = 0;
      const chunks = [];
      let completed = false;
      const done = (error) => {
        if (!completed) {
          completed = true;
          callback(error);
        }
      };
      stream.on("error", () => done(smtpError("Message stream failed", 451)));
      stream.on("data", (chunk) => {
        size += chunk.length;
        if (size <= Math.min(maxMessageBytes, maxTotalBytes)) chunks.push(chunk);
        else chunks.length = 0;
      });
      stream.on("end", async () => {
        try {
          if (stream.sizeExceeded || size > Math.min(maxMessageBytes, maxTotalBytes)) {
            return done(smtpError("Message exceeds mailbox size limit", 552));
          }
          if (behavior.rejectData) return done(smtpError("DATA rejected by test fixture", 554));
          const raw = Buffer.concat(chunks);
          const parsed = await simpleParser(raw, {
            skipHtmlToText: true,
            skipTextToHtml: true,
            skipImageLinks: true,
            maxHtmlLengthToParse: maxMessageBytes,
          });
          if (behavior.delayMs) await delay(behavior.delayMs, undefined, { signal: abort.signal });
          if (closing) return done(smtpError("Mailbox is shutting down", 421));
          while (messages.size >= maxMessages || totalBytes + size > maxTotalBytes) {
            remove(messages.keys().next().value);
          }
          const message = {
            id: randomUUID(),
            receivedAt: new Date().toISOString(),
            size,
            messageId: parsed.messageId ?? null,
            subject: parsed.subject ?? "",
            from: parsed.from?.value ?? [],
            to: parsed.to?.value ?? [],
            text: parsed.text ?? "",
            html: parsed.html || "",
            headers: Object.fromEntries(
              [...parsed.headers].map(([key, value]) => [
                key,
                typeof value === "string" ? value : JSON.stringify(value),
              ]),
            ),
            headerLines: parsed.headerLines,
            envelope: {
              from: session.envelope.mailFrom?.address ?? "",
              to: session.envelope.rcptTo.map(({ address }) => address),
            },
            raw,
          };
          messages.set(message.id, message);
          totalBytes += size;
          done(); // Acceptance is acknowledged only after the mail is in memory.
        } catch {
          done(smtpError("Cannot capture message", 451));
        }
      });
    },
  });
  // SMTPServer exposes its underlying net.Server for bound-port discovery and cleanup.
  smtp.server.on("connection", (socket) => {
    smtpSockets.add(socket);
    socket.once("close", () => smtpSockets.delete(socket));
  });
  smtp.on("error", () => {}); // Per-connection errors must not crash the local capture service.

  const http = createServer((req, res) => {
    const json = (status, body) => {
      res.writeHead(status, { "content-type": "application/json; charset=utf-8" });
      res.end(JSON.stringify(body));
    };
    res.setHeader("cache-control", "no-store");
    res.setHeader("x-content-type-options", "nosniff");
    res.setHeader(
      "content-security-policy",
      "default-src 'self'; frame-src 'self'; object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
    );
    try {
      const port = http.address().port;
      const allowedHosts = [`127.0.0.1:${port}`, `localhost:${port}`];
      if (!allowedHosts.includes(req.headers.host))
        return json(403, { error: "Loopback Host required" });
      if (
        req.headers.origin &&
        !allowedHosts.some((host) => req.headers.origin === `http://${host}`)
      ) {
        return json(403, { error: "Cross-origin requests are not allowed" });
      }
      const url = new URL(req.url, `http://127.0.0.1:${port}`);
      const asset = publicAssets.get(url.pathname);
      if (asset && req.method === "GET") {
        res.writeHead(200, { "content-type": asset.type });
        return res.end(asset.body);
      }
      if (url.pathname === "/api/messages") {
        if (req.method === "DELETE") {
          clear();
          return json(200, { deleted: true });
        }
        if (req.method !== "GET") return json(405, { error: "Method not allowed" });
        const recipient = url.searchParams.get("recipient")?.toLowerCase();
        const messageId = url.searchParams.get("messageId");
        const data = [...messages.values()]
          .reverse()
          .filter(
            (message) =>
              (!recipient ||
                message.envelope.to.some((address) => address.toLowerCase() === recipient)) &&
              (!messageId || message.messageId === messageId),
          )
          .map(({ id, receivedAt, size, messageId, subject, from, to, envelope }) => ({
            id,
            receivedAt,
            size,
            messageId,
            subject,
            from,
            to,
            envelope,
          }));
        return json(200, { messages: data, total: data.length, storedBytes: totalBytes });
      }
      const match = /^\/api\/messages\/([a-f0-9-]+)(?:\/(raw|preview))?$/.exec(url.pathname);
      if (match) {
        const message = messages.get(match[1]);
        if (!message) return json(404, { error: "Message not found" });
        if (req.method === "DELETE" && !match[2]) {
          remove(message.id);
          return json(200, { deleted: true });
        }
        if (req.method !== "GET") return json(405, { error: "Method not allowed" });
        if (match[2] === "raw") {
          res.writeHead(200, {
            "content-type": "message/rfc822",
            "content-disposition": `attachment; filename="${message.id}.eml"`,
          });
          return res.end(message.raw);
        }
        if (match[2] === "preview") {
          res.writeHead(200, {
            "content-type": "text/html; charset=utf-8",
            "content-security-policy": previewPolicy,
          });
          return res.end(previewHtml(message.html));
        }
        return json(200, detail(message));
      }
      return json(404, { error: "Not found" });
    } catch {
      return json(400, { error: "Invalid request" });
    }
  });
  http.on("connection", (socket) => {
    httpSockets.add(socket);
    socket.once("close", () => httpSockets.delete(socket));
  });

  function close() {
    closePromise ??= (async () => {
      closing = true;
      abort.abort();
      for (const socket of [...smtpSockets, ...httpSockets]) socket.destroy();
      await Promise.all([
        new Promise((resolve) => smtp.close(resolve)),
        new Promise((resolve) => http.close(resolve)),
      ]);
      clear();
    })();
    return closePromise;
  }

  try {
    await listen(smtp, smtpPort, "127.0.0.1");
    await listen(http, httpPort, "127.0.0.1");
  } catch (error) {
    await close();
    throw error;
  }
  return {
    smtpPort: smtp.server.address().port,
    httpPort: http.address().port,
    url: `http://127.0.0.1:${http.address().port}`,
    close,
  };
}
