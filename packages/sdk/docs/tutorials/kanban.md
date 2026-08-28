# 教程：实时 Kanban

该教程对应仓库 Kanban 集成场景：工作区包含 owner/members，任务通过 Relation 关联工作区和 assignees，并带多文件 attachments。

## 类型和查询

```ts
interface Task {
  id: string;
  title: string;
  status: "todo" | "in_progress" | "in_review" | "done";
  workspace: string;
  assignees: string[];
  attachments?: string[];
  order: number;
  is_private: boolean;
}

const tasks = hb.collection<Task>("kb_tasks");
const page = await tasks.list({
  filter: `workspace = '${workspaceId}'`,
  expand: ["assignees", "workspace.owner", "workspace.members"],
  sort: "order",
});
```

展开数据位于 `expand`，`workspace` 和 `assignees` 仍保留原始 Relation ID。

## 附件追加

```ts
await tasks.updateWithFiles(
  taskId,
  { files: { attachments: [{ blob: pdf, filename: "brief.pdf" }] } },
  { appendFiles: ["attachments"] },
);
```

## 实时更新

```ts
const subscription = await tasks.subscribe({
  filter: `workspace = '${workspaceId}'`,
  onEvent(event) {
    if (event.type === "create" || event.type === "update") {
      upsertCard(event.data.record);
    } else if (event.type === "delete") {
      removeCard(event.data.record.id);
    }
  },
  onStatus(status) {
    if (status === "connected") void reloadCurrentPage();
  },
});
```

每次重新连接后重新读取当前页面，可以弥补服务端尚无 Last-Event-ID 重放的事件缺口。页面卸载时调用 `subscription.close()`。可编译版本见 [../examples/kanban.ts](../examples/kanban.ts)。
