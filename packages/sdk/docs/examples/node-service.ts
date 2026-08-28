import { HertaBaseAdminClient } from "@hb/sdk/admin";

export async function readErrors(baseUrl: string, email: string, password: string) {
  const admin = new HertaBaseAdminClient({
    baseUrl,
    headers: { "user-agent": "hertabase-sync/1.0" },
  });
  await admin.auth.login({ email, password });
  return admin.logs.list({ level: "error", perPage: 100 });
}
