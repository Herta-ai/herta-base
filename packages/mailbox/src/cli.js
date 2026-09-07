import { startMailbox } from "./server.js";

try {
  const mailbox = await startMailbox({
    smtpPort: Number(process.env.HB_MAILBOX_SMTP_PORT ?? 1025),
    httpPort: Number(process.env.HB_MAILBOX_HTTP_PORT ?? 8025),
    maxMessageBytes: Number(process.env.HB_MAILBOX_MAX_MESSAGE_BYTES ?? 2 * 1024 * 1024),
    maxMessages: Number(process.env.HB_MAILBOX_MAX_MESSAGES ?? 100),
    maxTotalBytes: Number(process.env.HB_MAILBOX_MAX_TOTAL_BYTES ?? 32 * 1024 * 1024),
  });
  console.log(
    `SMTP: 127.0.0.1:${mailbox.smtpPort}\nInbox: ${mailbox.url}\nMail is kept in memory and cleared on restart.`,
  );
  for (const signal of ["SIGINT", "SIGTERM"]) {
    process.once(signal, () => {
      void mailbox.close();
    });
  }
} catch (error) {
  console.error(`Mailbox failed: ${error.message}`);
  process.exitCode = 1;
}
