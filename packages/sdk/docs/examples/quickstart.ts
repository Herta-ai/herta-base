import { HertaBaseClient, isHertaError } from "@hb/sdk";

interface Post {
  id: string;
  title: string;
  published: boolean;
}

export async function quickstart(): Promise<void> {
  const hb = new HertaBaseClient({ baseUrl: "http://localhost:8080" });
  await hb.auth.forCollection("users").login({
    email: "reader@example.com",
    password: "correct password",
  });
  try {
    const page = await hb.collection<Post>("posts").list({ filter: "published = true" });
    console.log(page.items);
  } catch (error) {
    if (isHertaError(error)) console.error(error.code, error.message);
    else throw error;
  }
}
