import { HertaBaseClient } from "@hb/sdk";

interface BlogUser {
  id: string;
  email: string;
  displayName: string;
}
interface BlogPost {
  id: string;
  title: string;
  content: string;
  author: string;
  is_published: boolean;
  expand?: { author?: BlogUser };
}
type NewPost = Omit<BlogPost, "id" | "expand">;

export async function loadBlog(baseUrl: string, email: string, password: string) {
  const hb = new HertaBaseClient({ baseUrl });
  const auth = hb.auth.forCollection("blog_users");
  const session = await auth.login<{ displayName: string }>({ email, password });
  const posts = hb.collection<BlogPost, NewPost>("blog_posts");
  await posts.create({
    title: "SDK guide",
    content: "Hello",
    author: session.user.id,
    is_published: true,
  });
  return posts.list({ filter: "is_published = true", expand: "author" });
}
