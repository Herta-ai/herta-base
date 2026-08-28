import { HertaBaseClient } from "@hb/sdk";

interface Task {
  id: string;
  title: string;
  workspace: string;
  status: "todo" | "in_progress" | "in_review" | "done";
  attachments?: string[];
}

export async function watchBoard(hb: HertaBaseClient, workspaceId: string) {
  const tasks = hb.collection<Task>("kb_tasks");
  const subscription = await tasks.subscribe({
    filter: `workspace = '${workspaceId}'`,
    onEvent(event) {
      if (event.type === "update") console.log(event.data.record.title);
    },
  });
  await tasks.updateWithFiles(
    "kb_tasks:one",
    { files: { attachments: { blob: new Blob(["brief"]), filename: "brief.pdf" } } },
    { appendFiles: ["attachments"] },
  );
  return subscription;
}
