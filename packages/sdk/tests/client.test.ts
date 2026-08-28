import { describe, expect, it, vi } from "vitest";

import { HertaBaseClient, HertaError } from "../src/index";

function jsonResponse<T>(
  data: T,
  meta: Record<string, unknown> | null = null,
  status = 200,
): Response {
  return new Response(JSON.stringify({ data, meta, error: null }), {
    status,
    headers: { "content-type": "application/json" },
  });
}

describe("HertaBaseClient", () => {
  it("encodes record IDs and serializes list options", async () => {
    const urls: string[] = [];
    const fetcher = vi.fn(async (input: RequestInfo | URL) => {
      urls.push(String(input));
      return jsonResponse([{ id: "posts:one", title: "One" }], { total: 1, page: 2, perPage: 10 });
    });
    const client = new HertaBaseClient({ baseUrl: "https://example.test/", fetch: fetcher });
    const posts = client.collection<{ id: string; title: string }>("blog_posts");

    const page = await posts.list({
      page: 2,
      perPage: 10,
      sort: ["-created_at", "title"],
      expand: ["author", "comments.user"],
      filter: "status = 'published'",
    });
    await posts.get("blog_posts:one");

    expect(page).toEqual({
      items: [{ id: "posts:one", title: "One" }],
      total: 1,
      page: 2,
      perPage: 10,
    });
    const listUrl = new URL(urls[0]!);
    expect(listUrl.pathname).toBe("/api/collections/blog_posts/records");
    expect(listUrl.searchParams.get("sort")).toBe("-created_at,title");
    expect(listUrl.searchParams.get("expand")).toBe("author,comments.user");
    expect(listUrl.searchParams.get("filter")).toBe("status = 'published'");
    expect(new URL(urls[1]!).pathname).toBe("/api/collections/blog_posts/records/blog_posts%3Aone");
  });

  it("unwraps API errors into HertaError", async () => {
    const fetcher = vi.fn(
      async () =>
        new Response(
          JSON.stringify({
            data: null,
            meta: null,
            error: {
              code: 400,
              message: "Validation failed",
              error: "HB_VALIDATION_ERROR",
              details: { field: "title" },
            },
          }),
          { status: 400, headers: { "content-type": "application/json" } },
        ),
    );
    const client = new HertaBaseClient({ baseUrl: "https://example.test", fetch: fetcher });

    await expect(client.collection("posts").create({ title: "" })).rejects.toMatchObject({
      name: "HertaError",
      kind: "api",
      status: 400,
      code: "HB_VALIDATION_ERROR",
      details: { field: "title" },
    } satisfies Partial<HertaError>);
  });

  it("builds multipart updates and appends file fields", async () => {
    let receivedBody: FormData | undefined;
    let receivedUrl = "";
    const fetcher = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
      receivedUrl = String(input);
      receivedBody = init?.body as FormData;
      return jsonResponse({ id: "tasks:one", attachments: ["stored.png"] });
    });
    const client = new HertaBaseClient({ baseUrl: "https://example.test", fetch: fetcher });

    await client.collection("tasks").updateWithFiles(
      "tasks:one",
      {
        data: { title: "Updated" },
        files: {
          attachments: { blob: new Blob(["png"], { type: "image/png" }), filename: "design.png" },
        },
      },
      { appendFiles: ["attachments"] },
    );

    expect(new URL(receivedUrl).searchParams.get("appendFiles")).toBe("attachments");
    expect(receivedBody).toBeInstanceOf(FormData);
    expect(receivedBody!.get("data")).toBe(JSON.stringify({ title: "Updated" }));
    expect((receivedBody!.get("attachments") as File).name).toBe("design.png");
  });

  it("returns raw file responses and creates token URLs", async () => {
    let receivedHeaders = new Headers();
    const fetcher = vi.fn(async (_input: RequestInfo | URL, init?: RequestInit) => {
      receivedHeaders = new Headers(init?.headers);
      return new Response("content", { status: 206, headers: { etag: '"one"' } });
    });
    const client = new HertaBaseClient({ baseUrl: "https://example.test", fetch: fetcher });
    const reference = {
      collection: "tasks",
      recordId: "tasks:one",
      field: "attachments",
      filename: "stored file.png",
    };

    const url = client.files.buildDownloadUrl(reference, "file-token");
    const response = await client.files.download(reference, { range: "bytes=0-9" });

    expect(new URL(url).searchParams.get("token")).toBe("file-token");
    expect(new URL(url).pathname).toContain("/tasks%3Aone/attachments/stored%20file.png");
    expect(response.status).toBe(206);
    expect(receivedHeaders.get("range")).toBe("bytes=0-9");
  });

  it("reports timeout and malformed envelopes separately", async () => {
    const hanging = new HertaBaseClient({
      baseUrl: "https://example.test",
      timeoutMs: 5,
      fetch: (_input, init) =>
        new Promise((_resolve, reject) => {
          init?.signal?.addEventListener("abort", () => reject(init.signal?.reason));
        }),
    });
    await expect(hanging.request("/api/test")).rejects.toMatchObject({ kind: "timeout" });

    const malformed = new HertaBaseClient({
      baseUrl: "https://example.test",
      fetch: async () => new Response("not json"),
    });
    await expect(malformed.request("/api/test")).rejects.toMatchObject({ kind: "protocol" });
  });
});
