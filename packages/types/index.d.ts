/// <reference lib="es2022" />
// Globals backed by implemented host bridges. Additional external adapters
// remain under implementation and will extend this surface when verified.
export {};
declare const hbResponseBrand: unique symbol;
declare const hbMiddlewareBrand: unique symbol;
declare global {
  type HbJson = null | boolean | number | string | HbJson[] | { [key: string]: HbJson };
  type HbObject = { [key: string]: HbJson };
  type HbReadonly<T> = T extends object ? { readonly [K in keyof T]: HbReadonly<T[K]> } : T;
  type HbMode = 'system' | 'request';
  interface HbHookOptions { collections?: string[]; authMode?: HbMode }
  type HbHook<E> = (event: E) => void | Promise<void>;
  interface HbRegistrar<E> {
    (handler: HbHook<E>, ...collections: string[]): void;
    (handler: HbHook<E>, options: HbHookOptions): void;
  }
  interface HbBaseEvent {
    readonly name: string; readonly requestId: string | null; readonly authMode: HbMode;
    readonly context: { [key: string]: unknown };
    next(): Promise<void>;
  }
  interface HbReadonlyRecord {
    readonly id: string; readonly collectionName: string;
    /** Nested values are independent copies. */
    get(field: string): HbJson | undefined;
    original(field: string): HbJson | undefined;
    isNew(): boolean;
    toJSON(): HbObject;
  }
  interface HbRecord extends HbReadonlyRecord {
    /** Only before this handler calls next(); timing is enforced at runtime. */
    set(field: string, value: HbJson): void;
    unset(field: string): void;
  }
  type HbRule = string | boolean | null;
  interface HbRules { list?: HbRule; view?: HbRule; create?: HbRule; update?: HbRule; delete?: HbRule }
  interface HbField {
    name: string;
    type: 'text' | 'number' | 'bool' | 'datetime' | 'json' | 'file' | 'relation' | 'select' | 'email' | 'url';
    required?: boolean; options?: HbObject | null;
  }
  interface HbIndex { name: string; fields: string[]; unique?: boolean }
  interface HbCollection {
    readonly name: string; readonly version?: string;
    type: 'base' | 'auth'; schema_mode: 'schema-less' | 'strict' | 'mixed';
    fields?: HbField[]; indexes?: HbIndex[]; rules?: HbRules;
  }
  interface HbVersionedCollection extends HbCollection { readonly version: string }
  interface HbRecordSnapshot {
    readonly name: string; readonly requestId: string | null;
    readonly collection: HbReadonly<HbCollection>;
    readonly record: HbReadonlyRecord; readonly originalRecord: HbReadonlyRecord | null;
  }
  interface HbRecordEvent<R extends HbReadonlyRecord = HbRecord> extends HbBaseEvent {
    readonly record: R; readonly originalRecord: HbReadonlyRecord | null;
    readonly collection: HbReadonly<HbCollection>;
    afterCommit(callback: (snapshot: HbRecordSnapshot) => void | Promise<void>): void;
  }
  type RecordEvent = HbRecordEvent;
  const onRecordCreate: HbRegistrar<HbRecordEvent>;
  const onRecordUpdate: HbRegistrar<HbRecordEvent>;
  const onRecordDelete: HbRegistrar<HbRecordEvent<HbReadonlyRecord>>;
  interface HbRequest {
    readonly method: string; readonly path: string;
    pathValue(name: string): string | null;
    query(name: string): string | string[] | null;
    header(name: string): string | null;
    json(): Promise<HbJson>; text(): Promise<string>; bytes(): Promise<Uint8Array>;
  }
  interface HbResponse { readonly [hbResponseBrand]: true }
  interface RequestEvent {
    readonly request: HbRequest; readonly auth: HbReadonly<HbObject> | null;
    readonly response: HbResponse | null;
    json(status: number, data: unknown, meta?: unknown): HbResponse;
    rawJson(status: number, data: unknown): HbResponse;
    text(status: number, text: string): HbResponse; html(status: number, html: string): HbResponse;
    noContent(): HbResponse;
    file(key: string): Promise<HbResponse>;
  }
  interface HbRecordRequest<R extends HbReadonlyRecord> extends HbBaseEvent, RequestEvent {
    readonly record: R; readonly originalRecord: HbReadonlyRecord | null;
    readonly collection: HbReadonly<HbCollection>;
  }
  interface HbListRequest extends HbBaseEvent, RequestEvent {
    readonly collection: HbReadonly<HbCollection>; readonly query: HbReadonly<HbObject>;
    readonly records: readonly HbReadonly<HbObject>[] | null;
  }
  interface HbRequestRegistrar<E> {
    (handler: (event: E) => void | HbResponse | Promise<void | HbResponse>, ...collections: string[]): void;
    (handler: (event: E) => void | HbResponse | Promise<void | HbResponse>, options: HbHookOptions): void;
  }
  const onRecordCreateRequest: HbRequestRegistrar<HbRecordRequest<HbRecord>>;
  const onRecordUpdateRequest: HbRequestRegistrar<HbRecordRequest<HbRecord>>;
  const onRecordDeleteRequest: HbRequestRegistrar<HbRecordRequest<HbReadonlyRecord>>;
  const onRecordViewRequest: HbRequestRegistrar<HbRecordRequest<HbReadonlyRecord>>;
  const onRecordListRequest: HbRequestRegistrar<HbListRequest>;
  interface HbCollectionSnapshot {
    readonly name: string; readonly requestId: string | null;
    readonly collection: HbReadonly<HbVersionedCollection>;
    readonly originalCollection: HbReadonly<HbVersionedCollection> | null;
  }
  interface HbCollectionEvent<C = HbCollection> extends HbBaseEvent {
    /** Nested values are copies; assign fields/indexes/rules back to apply changes. */
    readonly collection: C; readonly originalCollection: HbReadonly<HbVersionedCollection> | null;
    afterCommit(callback: (snapshot: HbCollectionSnapshot) => void | Promise<void>): void;
  }
  const onCollectionCreate: HbRegistrar<HbCollectionEvent>;
  const onCollectionUpdate: HbRegistrar<HbCollectionEvent<HbVersionedCollection>>;
  const onCollectionDelete: HbRegistrar<HbCollectionEvent<HbReadonly<HbVersionedCollection>>>;
  interface HbAccount {
    readonly id: string; readonly collection: string; readonly email: string;
    readonly role: string; readonly verified: boolean; readonly admin: boolean;
    readonly createdAt?: HbJson; readonly updatedAt?: HbJson;
    readonly [field: string]: HbJson | undefined;
  }
  interface HbAuthSnapshot {
    readonly name: string; readonly requestId: string | null;
    readonly collection: HbReadonly<HbCollection> | { readonly name: string };
    readonly account: HbReadonly<HbAccount>;
  }
  interface HbAuthEvent extends HbBaseEvent {
    readonly collection: HbAuthSnapshot['collection']; readonly request: HbRequest;
    readonly auth: HbReadonly<HbObject> | null; readonly account: HbReadonly<HbAccount> | null;
  }
  interface HbRegisterEvent extends HbAuthEvent {
    /** Clean profile only. Assign before next(); nested values are copies. */
    readonly profile: HbObject;
    afterCommit(callback: (snapshot: HbAuthSnapshot & { readonly profile: HbReadonly<HbObject> }) => void | Promise<void>): void;
  }
  interface HbTokenEvent extends HbAuthEvent {
    readonly account: HbReadonly<HbAccount>;
    afterCommit(callback: (snapshot: HbAuthSnapshot) => void | Promise<void>): void;
  }
  const onAuthRegister: HbRegistrar<HbRegisterEvent>;
  const onAuthLogin: HbRegistrar<HbTokenEvent>;
  const onTokenRefresh: HbRegistrar<HbTokenEvent>;
  function onBootstrap(handler: HbHook<HbBaseEvent>, options?: { authMode?: 'system' }): void;
  function onServe(handler: HbHook<HbBaseEvent>, options?: { authMode?: 'system' }): void;
  function onShutdown(handler: HbHook<HbBaseEvent>, options?: { authMode?: 'system' }): void;
  interface HbCronOptions {
    timezone?: string; maxRuntimeMs?: number; retries?: number; idempotent?: boolean;
  }
  interface HbCronContext {
    readonly name: string; readonly runId: string; readonly attemptId: string; readonly scheduledAt: string;
  }
  /** Registration only. Six numeric fields, IANA timezone, AND calendar matching. */
  function cronAdd(name: string, expression: string, handler: (context: HbCronContext) => void | Promise<void>, options?: HbCronOptions): void;
  function cronRemove(name: string): void;
  type HbMethod = 'GET' | 'HEAD' | 'POST' | 'PUT' | 'PATCH' | 'DELETE';
  type HbRouteHandler = (event: RequestEvent) => HbResponse | Promise<HbResponse>;
  type HbMiddleware = (event: RequestEvent & { next(): Promise<HbResponse> }) => HbResponse | void | Promise<HbResponse | void>;
  interface HbNativeMiddleware { readonly [hbMiddlewareBrand]: true }
  interface HbRouteOptions { method: HbMethod; path: string; authMode?: HbMode; middleware?: (HbNativeMiddleware | HbMiddleware)[] }
  function routerAdd(method: HbMethod, path: string, handler: HbRouteHandler, ...middleware: (HbNativeMiddleware | HbMiddleware)[]): void;
  function routerAdd(options: HbRouteOptions, handler: HbRouteHandler): void;
  const $apis: {
    requireAuth(): HbNativeMiddleware; requireAdmin(): HbNativeMiddleware; bodyLimit(bytes: number): HbNativeMiddleware;
    rateLimit(options: { limit: number; windowMs: number; key?: 'ip' | 'auth' }): HbNativeMiddleware;
  };
  interface AppLogger {
    trace(message: string, fields?: HbObject): void; debug(message: string, fields?: HbObject): void;
    info(message: string, fields?: HbObject): void; warn(message: string, fields?: HbObject): void;
    error(message: string, fields?: HbObject): void;
  }
  interface HbMailAddress { address: string; name?: string | null }
  interface HbMailMessage {
    from?: HbMailAddress | null; to: HbMailAddress[]; subject: string;
    text?: string | null; html?: string | null; headers?: { [name: string]: string };
  }
  interface HbHttpRequest {
    url: string; method?: HbMethod | 'OPTIONS'; headers?: { [name: string]: string };
    /** UTF-8 text. GET and HEAD must omit body. */
    body?: string | null; timeoutMs?: number | null;
  }
  interface HbHttpResponse {
    readonly status: number; readonly ok: boolean;
    readonly headers: { readonly [name: string]: string }; readonly body: string;
    /** Parses the bounded UTF-8 body; invalid JSON throws BadRequestError. */
    json(): HbJson;
  }
  type HbMessageAudience
    = { users: { collection: string; id: string }[]; roles?: never; connections?: never }
    | { roles: { collection: string; role: string }[]; users?: never; connections?: never }
    | { connections: string[]; users?: never; roles?: never };
  interface HbDatabase {
    newRecord(collection: string, input?: HbObject): HbRecord;
    findRecordById(collection: string, id: string): Promise<HbRecord>;
    findRecordsByFilter(collection: string, filter?: string, sort?: string, limit?: number, offset?: number, bindings?: HbObject): Promise<HbRecord[]>;
    findFirstRecordByFilter(collection: string, filter?: string, bindings?: HbObject): Promise<HbRecord>;
    save(record: HbRecord): Promise<HbRecord>; delete(record: HbRecord): Promise<HbReadonlyRecord>;
    readonly db: { query(query: string, bindings?: HbObject): Promise<HbObject[]> };
  }
  interface HbFileItem {
    readonly key: string; readonly size: number; readonly contentType: string;
    readonly version: string; readonly updatedAt: string;
  }
  interface HbFiles {
    write(key: string, data: string | Uint8Array, options?: { contentType?: string }): Promise<HbFileItem>;
    readText(key: string): Promise<string>; readBytes(key: string): Promise<Uint8Array>;
    exists(key: string): Promise<boolean>; stat(key: string): Promise<HbFileItem>;
    /** Literal prefix; default 100, maximum 500. Cursors do not freeze a directory snapshot. */
    list(prefix?: string, options?: { limit?: number; cursor?: string | null }): Promise<{ items: HbFileItem[]; nextCursor: string | null }>;
    copy(source: string, destination: string): Promise<HbFileItem>;
    /** A failed source cleanup throws HB_FILE_MOVE_PARTIAL after creating destination. */
    move(source: string, destination: string): Promise<HbFileItem>;
    remove(key: string): Promise<null>;
  }
  interface HbOutboxReceipt {
    readonly jobId: string; readonly kind: 'mail.send' | 'http.send';
    readonly state: 'pending' | 'leased' | 'sending' | 'accepted' | 'failed' | 'unknown';
    readonly attempts: number; readonly createdAt: number; readonly updatedAt: number;
    readonly nextAttemptAt: number; readonly errorCode: string | null; readonly result: HbObject | null;
  }
  interface HbOutbox {
    enqueue(kind: 'mail.send', payload: HbMailMessage, options: { idempotencyKey: string }): Promise<HbOutboxReceipt>;
    enqueue(kind: 'http.send', payload: HbHttpRequest, options: { idempotencyKey: string }): Promise<HbOutboxReceipt>;
  }
  interface HbTransaction extends HbDatabase {
    readonly outbox: HbOutbox;
    afterCommit(callback: () => void | Promise<void>): void;
  }
  interface App extends HbDatabase {
    readonly logger: AppLogger; env(name: string): string | null;
    transaction<T>(callback: (transaction: HbTransaction) => T | Promise<T>): Promise<T>;
    readonly mailer: { send(message: HbMailMessage): Promise<{ messageId: string; status: 'accepted' }> };
    readonly http: { send(request: HbHttpRequest): Promise<HbHttpResponse> };
    readonly files: HbFiles;
    readonly outbox: HbOutbox;
    readonly realtime: { publish(topic: string, data: HbJson, audience: HbMessageAudience): Promise<{ queued: number; dropped: number }> };
    readonly collections: {
      findByName(name: string): Promise<HbVersionedCollection>; list(): Promise<HbVersionedCollection[]>;
      create(definition: HbCollection): Promise<HbReadonly<HbVersionedCollection>>;
      save(definition: HbVersionedCollection): Promise<HbReadonly<HbVersionedCollection>>;
      delete(definition: string | HbVersionedCollection): Promise<HbReadonly<HbVersionedCollection>>;
    };
  }
  const $app: App;
  class BadRequestError extends Error { readonly code: 'HB_VALIDATION_ERROR'; readonly details: HbJson; constructor(message?: string, details?: HbJson) }
  class ForbiddenError extends Error { readonly code: 'HB_FORBIDDEN'; readonly details: HbJson; constructor(message?: string, details?: HbJson) }
  class NotFoundError extends Error { readonly code: 'HB_NOT_FOUND'; readonly details: HbJson; constructor(message?: string, details?: HbJson) }
  class UnauthorizedError extends Error { readonly code: 'HB_UNAUTHORIZED'; readonly details: HbJson; constructor(message?: string, details?: HbJson) }
  class ConflictError extends Error { readonly code: 'HB_CONFLICT'; readonly details: HbJson; constructor(message?: string, details?: HbJson) }
}
