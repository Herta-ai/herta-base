import { Transport } from "./transport";
import type {
  FileAccessOptions,
  FileReference,
  FileTokenRequest,
  FileTokenResponse,
  QueryValue,
} from "./types";
import { encodePath } from "./utils";

export class FilesClient {
  constructor(private readonly transport: Transport) {}

  issueToken(request: FileTokenRequest): Promise<FileTokenResponse> {
    return this.transport.request("/api/files/token", {
      method: "POST",
      body: request,
    });
  }

  buildDownloadUrl(reference: FileReference, token?: string): string {
    return this.transport.url(this.path(reference), token ? { token } : undefined);
  }

  download(reference: FileReference, options: FileAccessOptions = {}): Promise<Response> {
    return this.access("GET", reference, options);
  }

  head(reference: FileReference, options: FileAccessOptions = {}): Promise<Response> {
    return this.access("HEAD", reference, options);
  }

  private access(
    method: "GET" | "HEAD",
    reference: FileReference,
    options: FileAccessOptions,
  ): Promise<Response> {
    const headers = new Headers();
    if (options.range) headers.set("range", options.range);
    if (options.ifNoneMatch) headers.set("if-none-match", options.ifNoneMatch);
    const query: Record<string, QueryValue> | undefined = options.token
      ? { token: options.token }
      : undefined;
    return this.transport.response(this.path(reference), {
      method,
      query,
      headers,
      signal: options.signal,
      timeoutMs: options.timeoutMs,
      auth: !options.token,
    });
  }

  private path(reference: FileReference): string {
    return `/api/files/${encodePath(reference.collection)}/${encodePath(reference.recordId)}/${encodePath(reference.field)}/${encodePath(reference.filename)}`;
  }
}
