import { Transport } from "./transport";
import type {
  CollectionDefinition,
  DeletedResource,
  LogEntry,
  LogListOptions,
  Page,
  UpdateCollectionDefinition,
  WebProject,
  WebProjectDeploy,
  WebProjectPatch,
} from "./types";
import { appendUploadFile, encodePath } from "./utils";

export class CollectionsAdminClient {
  constructor(private readonly transport: Transport) {}

  list(): Promise<CollectionDefinition[]> {
    return this.transport.request("/_/collections");
  }

  get(name: string): Promise<CollectionDefinition> {
    return this.transport.request(`/_/collections/${encodePath(name)}`);
  }

  create(definition: CollectionDefinition): Promise<CollectionDefinition> {
    return this.transport.request("/_/collections", { method: "POST", body: definition });
  }

  update(name: string, definition: UpdateCollectionDefinition): Promise<CollectionDefinition> {
    return this.transport.request(`/_/collections/${encodePath(name)}`, {
      method: "PATCH",
      body: definition,
    });
  }

  delete(name: string): Promise<DeletedResource> {
    return this.transport.request(`/_/collections/${encodePath(name)}`, { method: "DELETE" });
  }
}

export class LogsAdminClient {
  constructor(private readonly transport: Transport) {}

  async list(options: LogListOptions = {}): Promise<Page<LogEntry>> {
    const result = await this.transport.requestWithMeta<LogEntry[]>("/api/admin/logs", {
      query: {
        page: options.page,
        perPage: options.perPage,
        level: options.level,
        logType: options.logType,
        q: options.q,
        target: options.target,
        path: options.path,
        statusCode: options.statusCode,
        from: options.from,
        to: options.to,
      },
      signal: options.signal,
    });
    return {
      items: result.data,
      total: metaNumber(result.meta, "total", result.data.length),
      page: metaNumber(result.meta, "page", options.page ?? 1),
      perPage: metaNumber(result.meta, "perPage", options.perPage ?? 30),
    };
  }
}

export class WebProjectsAdminClient {
  constructor(private readonly transport: Transport) {}

  list(): Promise<WebProject[]> {
    return this.transport.request("/_/web-projects");
  }

  get(name: string): Promise<WebProject> {
    return this.transport.request(`/_/web-projects/${encodePath(name)}`);
  }

  deploy(input: WebProjectDeploy): Promise<WebProject> {
    const form = new FormData();
    appendUploadFile(form, "archive", input.archive);
    if (input.alias !== undefined) form.append("alias", input.alias ?? "");
    if (input.spaFallback !== undefined) form.append("spaFallback", String(input.spaFallback));
    if (input.cacheControl !== undefined) form.append("cacheControl", input.cacheControl);
    if (input.notFound !== undefined) form.append("notFound", input.notFound ?? "");
    return this.transport.request("/_/web-projects", { method: "POST", body: form });
  }

  update(name: string, patch: WebProjectPatch): Promise<WebProject> {
    return this.transport.request(`/_/web-projects/${encodePath(name)}`, {
      method: "PATCH",
      body: patch,
    });
  }

  delete(name: string): Promise<DeletedResource> {
    return this.transport.request(`/_/web-projects/${encodePath(name)}`, { method: "DELETE" });
  }

  versions(name: string): Promise<string[]> {
    return this.transport.request(`/_/web-projects/${encodePath(name)}/versions`);
  }

  rollback(name: string, version: string): Promise<WebProject> {
    return this.transport.request(`/_/web-projects/${encodePath(name)}/rollback`, {
      method: "POST",
      body: { version },
    });
  }
}

function metaNumber(meta: Record<string, unknown> | null, key: string, fallback: number): number {
  return typeof meta?.[key] === "number" ? meta[key] : fallback;
}
