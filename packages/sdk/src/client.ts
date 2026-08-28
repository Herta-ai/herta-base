import { AuthClient } from "./auth";
import { CollectionClient } from "./collection";
import { FilesClient } from "./files";
import { ClientRuntime } from "./runtime";
import type { HertaBaseClientOptions, HertaRecord, HertaRequestOptions } from "./types";

export class HertaBaseClient {
  protected readonly runtime: ClientRuntime;
  readonly auth: AuthClient;
  readonly files: FilesClient;

  constructor(options: HertaBaseClientOptions = {}) {
    this.runtime = new ClientRuntime(options);
    this.auth = new AuthClient(this.runtime.transport, this.runtime.authState, { kind: "default" });
    this.files = new FilesClient(this.runtime.transport);
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
