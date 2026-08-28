import { AuthState } from "./auth-store";
import { HertaError } from "./errors";
import type {
  ApiEnvelope,
  FetchLike,
  HertaRequestOptions,
  MaybePromise,
  QueryValue,
  RequestContext,
} from "./types";
import { buildUrl, normalizeBaseUrl } from "./utils";

interface InternalRequestOptions extends HertaRequestOptions {
  retryAuth?: boolean;
}

export interface TransportResult<T> {
  data: T;
  meta: Record<string, unknown> | null;
  response: Response;
}

export interface TransportOptions {
  baseUrl: string;
  fetch: FetchLike;
  headers?: HeadersInit | (() => MaybePromise<HeadersInit>);
  timeoutMs: number;
  authState: AuthState;
}

interface FetchOutcome {
  response: Response;
  request: RequestContext;
  accessToken: string | null;
}

export class Transport {
  readonly baseUrl: string;
  private readonly fetcher: FetchLike;
  private readonly defaultHeaders?: HeadersInit | (() => MaybePromise<HeadersInit>);
  private readonly timeoutMs: number;
  readonly authState: AuthState;

  constructor(options: TransportOptions) {
    this.baseUrl = normalizeBaseUrl(options.baseUrl);
    this.fetcher = options.fetch;
    this.defaultHeaders = options.headers;
    this.timeoutMs = options.timeoutMs;
    this.authState = options.authState;
  }

  async request<T>(path: string, options: InternalRequestOptions = {}): Promise<T> {
    return (await this.requestWithMeta<T>(path, options)).data;
  }

  async requestWithMeta<T>(
    path: string,
    options: InternalRequestOptions = {},
  ): Promise<TransportResult<T>> {
    const outcome = await this.fetchWithAuthRetry(path, options);
    const envelope = await this.readEnvelope<T>(outcome.response, outcome.request);
    if (!outcome.response.ok || envelope.error) {
      throw this.envelopeError(envelope, outcome.response, outcome.request);
    }
    if (envelope.data === null) {
      throw new HertaError("Successful API response did not contain data", {
        kind: "protocol",
        status: outcome.response.status,
        request: outcome.request,
      });
    }
    return { data: envelope.data, meta: envelope.meta, response: outcome.response };
  }

  async response(path: string, options: InternalRequestOptions = {}): Promise<Response> {
    const outcome = await this.fetchWithAuthRetry(path, options);
    if (outcome.response.ok || outcome.response.status === 304) return outcome.response;
    const envelope = await this.readEnvelope<never>(outcome.response, outcome.request);
    throw this.envelopeError(envelope, outcome.response, outcome.request);
  }

  url(path: string, query?: Record<string, QueryValue>): string {
    return buildUrl(this.baseUrl, path, query);
  }

  private async fetchWithAuthRetry(
    path: string,
    options: InternalRequestOptions,
  ): Promise<FetchOutcome> {
    const first = await this.fetchOnce(path, options);
    if (options.auth !== false && options.retryAuth !== false && first.response.status === 401) {
      const envelope = await this.tryReadEnvelope(first.response);
      if (envelope?.error?.error === "HB_TOKEN_EXPIRED") {
        try {
          await this.authState.refresh(first.accessToken ?? undefined);
        } catch (cause) {
          if (cause instanceof HertaError) throw cause;
          throw new HertaError("Authentication refresh failed", {
            kind: "configuration",
            request: first.request,
            cause,
          });
        }
        return this.fetchOnce(path, { ...options, retryAuth: false });
      }
    }
    return first;
  }

  private async fetchOnce(path: string, options: InternalRequestOptions): Promise<FetchOutcome> {
    const method = (options.method ?? "GET").toUpperCase();
    const url = buildUrl(this.baseUrl, path, options.query);
    const request = { method, path, url };
    let resolvedHeaders: HeadersInit | undefined;
    try {
      resolvedHeaders =
        typeof this.defaultHeaders === "function"
          ? await this.defaultHeaders()
          : this.defaultHeaders;
    } catch (cause) {
      throw new HertaError("Resolving default request headers failed", {
        kind: "configuration",
        request,
        cause,
      });
    }
    const headers = new Headers(resolvedHeaders);
    new Headers(options.headers).forEach((value, name) => headers.set(name, value));
    headers.set("accept", headers.get("accept") ?? "application/json");

    let accessToken: string | null = null;
    if (options.auth !== false) {
      try {
        accessToken = await this.authState.accessToken();
      } catch (cause) {
        if (cause instanceof HertaError) throw cause;
        throw new HertaError("Authentication refresh failed", {
          kind: "network",
          request,
          cause,
        });
      }
      if (accessToken && !headers.has("authorization")) {
        headers.set("authorization", `Bearer ${accessToken}`);
      }
    }

    let body: BodyInit | undefined;
    try {
      body = this.prepareBody(options.body, headers);
    } catch (cause) {
      throw new HertaError("Serializing the request body failed", {
        kind: "configuration",
        request,
        cause,
      });
    }
    const timeoutMs = options.timeoutMs ?? this.timeoutMs;
    const timeoutController = new AbortController();
    let timedOut = false;
    const timeout =
      timeoutMs > 0
        ? setTimeout(() => {
            timedOut = true;
            timeoutController.abort(new Error(`Request timed out after ${timeoutMs}ms`));
          }, timeoutMs)
        : null;
    const signal = options.signal
      ? AbortSignal.any([timeoutController.signal, options.signal])
      : timeoutController.signal;

    try {
      const init: RequestInit = { method, headers, signal };
      if (body !== undefined) init.body = body;
      const response = await this.fetcher(url, init);
      return { response, request, accessToken };
    } catch (cause) {
      if (timedOut) {
        throw new HertaError(`Request timed out after ${timeoutMs}ms`, {
          kind: "timeout",
          request,
          cause,
        });
      }
      if (signal.aborted) {
        throw new HertaError("Request was aborted", { kind: "abort", request, cause });
      }
      throw new HertaError("Network request failed", { kind: "network", request, cause });
    } finally {
      if (timeout) clearTimeout(timeout);
    }
  }

  private prepareBody(body: unknown, headers: Headers): BodyInit | undefined {
    if (body === undefined || body === null) return undefined;
    if (
      typeof body === "string" ||
      body instanceof Blob ||
      body instanceof FormData ||
      body instanceof URLSearchParams ||
      body instanceof ArrayBuffer ||
      ArrayBuffer.isView(body)
    ) {
      return body as BodyInit;
    }
    headers.set("content-type", headers.get("content-type") ?? "application/json");
    return JSON.stringify(body);
  }

  private async readEnvelope<T>(
    response: Response,
    request: RequestContext,
  ): Promise<ApiEnvelope<T>> {
    const envelope = await this.tryReadEnvelope<T>(response);
    if (!envelope) {
      throw new HertaError("API response is not a valid JSON envelope", {
        kind: "protocol",
        status: response.status,
        request,
      });
    }
    return envelope;
  }

  private async tryReadEnvelope<T>(response: Response): Promise<ApiEnvelope<T> | null> {
    let value: unknown;
    try {
      value = await response.clone().json();
    } catch {
      return null;
    }
    if (!value || typeof value !== "object" || !("data" in value) || !("error" in value)) {
      return null;
    }
    return value as ApiEnvelope<T>;
  }

  private envelopeError<T>(
    envelope: ApiEnvelope<T>,
    response: Response,
    request: RequestContext,
  ): HertaError {
    if (envelope.error) return HertaError.api(envelope.error, request);
    return new HertaError(`API request failed with HTTP ${response.status}`, {
      kind: "protocol",
      status: response.status,
      request,
    });
  }
}
