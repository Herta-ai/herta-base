import { AdminAuthClient } from "./auth";
import { CollectionsAdminClient, LogsAdminClient, WebProjectsAdminClient } from "./admin-resources";
import { CollectionClient } from "./collection";
import { FilesClient } from "./files";
import { ClientRuntime } from "./runtime";
import type { HertaBaseClientOptions, HertaRecord, HertaRequestOptions } from "./types";

export class HertaBaseAdminClient {
  private readonly runtime: ClientRuntime;
  readonly auth: AdminAuthClient;
  readonly files: FilesClient;
  readonly collections: CollectionsAdminClient;
  readonly logs: LogsAdminClient;
  readonly webProjects: WebProjectsAdminClient;

  constructor(options: HertaBaseClientOptions = {}) {
    this.runtime = new ClientRuntime(options);
    this.auth = new AdminAuthClient(this.runtime.transport, this.runtime.authState);
    this.files = new FilesClient(this.runtime.transport);
    this.collections = new CollectionsAdminClient(this.runtime.transport);
    this.logs = new LogsAdminClient(this.runtime.transport);
    this.webProjects = new WebProjectsAdminClient(this.runtime.transport);
  }

  collection<
    TRecord extends HertaRecord = HertaRecord,
    TCreate extends object = Record<string, unknown>,
    TUpdate extends object = Partial<TCreate>,
  >(name: string): CollectionClient<TRecord, TCreate, TUpdate> {
    return new CollectionClient(this.runtime.transport, name);
  }

  request<T>(path: string, options: HertaRequestOptions = {}): Promise<T> {
    return this.runtime.transport.request(path, options);
  }
}
