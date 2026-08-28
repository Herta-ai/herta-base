export type MaybePromise<T> = T | Promise<T>

export type FetchLike = (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>

export interface ApiErrorPayload {
  code: number
  message: string
  error: string
  details?: unknown
}

export interface ApiEnvelope<T> {
  data: T | null
  meta: Record<string, unknown> | null
  error: ApiErrorPayload | null
}

export interface RequestContext {
  method: string
  path: string
  url: string
}

export interface AuthUser {
  id: string
  collection: string
  email: string
  role: string
  verified: boolean
  admin: boolean
  createdAt?: unknown
  updatedAt?: unknown
}

export type ProfiledAuthUser<TProfile extends object = object> = AuthUser & TProfile

export interface AuthResponse<TProfile extends object = object> {
  accessToken: string
  refreshToken: string
  tokenType: 'Bearer' | string
  expiresIn: number
  user: ProfiledAuthUser<TProfile>
}

export type AuthScope
  = | { kind: 'default' }
    | { kind: 'collection', collection: string }
    | { kind: 'admin' }

export interface AuthSession<TProfile extends object = object> extends AuthResponse<TProfile> {
  expiresAt: number
  scope: AuthScope
}

export interface AuthStore {
  get: () => MaybePromise<AuthSession | null>
  set: (session: AuthSession) => MaybePromise<void>
  clear: () => MaybePromise<void>
}

export type AuthChangeListener = (session: AuthSession | null) => void

export interface Credentials<TProfile extends object = object> {
  email: string
  password: string
  profile?: TProfile
}

export interface HertaBaseClientOptions {
  baseUrl?: string
  fetch?: FetchLike
  headers?: HeadersInit | (() => MaybePromise<HeadersInit>)
  timeoutMs?: number
  authStore?: AuthStore
  refreshSkewMs?: number
}

export type QueryValue = string | number | boolean | readonly string[] | null | undefined

export interface HertaRequestOptions {
  method?: string
  query?: Record<string, QueryValue>
  body?: unknown
  headers?: HeadersInit
  signal?: AbortSignal
  timeoutMs?: number
  auth?: boolean
}

export interface HertaRecord {
  id: string
  expand?: Record<string, unknown>
}

export interface ListOptions {
  page?: number
  perPage?: number
  sort?: string | readonly string[]
  filter?: string
  expand?: string | readonly string[]
  signal?: AbortSignal
  timeoutMs?: number
}

export interface GetOptions {
  expand?: string | readonly string[]
  signal?: AbortSignal
  timeoutMs?: number
}

export interface MutationOptions {
  signal?: AbortSignal
  timeoutMs?: number
}

export interface Page<T> {
  items: T[]
  total: number
  page: number
  perPage: number
}

export type UploadFile = Blob | { blob: Blob, filename?: string }

export interface RecordUpload<TData extends object = Record<string, unknown>> {
  data?: TData
  files: Record<string, UploadFile | readonly UploadFile[]>
}

export interface FileMutationOptions extends MutationOptions {
  appendFiles?: readonly string[]
}

export interface FileTokenRequest {
  collection: string
  recordId: string
  field: string
}

export interface FileTokenResponse {
  token: string
  expiresIn: number
}

export interface FileReference extends FileTokenRequest {
  filename: string
}

export interface FileAccessOptions {
  token?: string
  range?: string
  ifNoneMatch?: string
  signal?: AbortSignal
  timeoutMs?: number
}

export interface ReconnectOptions {
  enabled?: boolean
  initialDelayMs?: number
  maxDelayMs?: number
  multiplier?: number
  jitter?: number
  maxAttempts?: number
}

export type RealtimeStatus = 'connecting' | 'connected' | 'reconnecting' | 'closed'

export interface ConnectedEventData {
  subscriptionId: string
  collection: string
  timestamp: string
}

export interface ChangeEventData<TRecord> {
  id: string
  action: 'create' | 'update' | 'delete'
  record: TRecord
  timestamp: string
}

export interface PingEventData {
  timestamp: string
}

export type RealtimeEvent<TRecord>
  = | { type: 'connected', id?: string, data: ConnectedEventData }
    | { type: 'create' | 'update' | 'delete', id?: string, data: ChangeEventData<TRecord> }
    | { type: 'ping', id?: string, data: PingEventData }
    | { type: 'error', id?: string, data: ApiEnvelope<never> }

export interface SubscribeOptions<TRecord> {
  filter?: string
  signal?: AbortSignal
  reconnect?: boolean | ReconnectOptions
  onEvent?: (event: RealtimeEvent<TRecord>) => void
  onStatus?: (status: RealtimeStatus) => void
  onError?: (error: Error) => void
}

export interface RealtimeSubscription<TRecord> {
  readonly status: RealtimeStatus
  onEvent: (listener: (event: RealtimeEvent<TRecord>) => void) => () => void
  onStatus: (listener: (status: RealtimeStatus) => void) => () => void
  onError: (listener: (error: Error) => void) => () => void
  close: () => void
}

export type ApiRule = string | boolean | null

export interface CollectionRules {
  list?: ApiRule
  view?: ApiRule
  create?: ApiRule
  update?: ApiRule
  delete?: ApiRule
}

export type FieldType
  = | 'text'
    | 'number'
    | 'bool'
    | 'datetime'
    | 'json'
    | 'file'
    | 'relation'
    | 'select'
    | 'email'
    | 'url'

export interface FieldDefinition {
  name: string
  type: FieldType
  required?: boolean
  options?: Record<string, unknown> | null
}

export interface IndexDefinition {
  name: string
  fields: string[]
  unique?: boolean
}

export interface CollectionDefinition {
  name: string
  type: 'base' | 'auth'
  schema_mode: 'schema-less' | 'strict' | 'mixed'
  fields?: FieldDefinition[]
  indexes?: IndexDefinition[]
  rules?: CollectionRules
}

export interface UpdateCollectionDefinition {
  fields?: FieldDefinition[]
  indexes?: IndexDefinition[]
  rules?: CollectionRules | null
}

export interface LogEntry extends HertaRecord {
  created_at: string
  log_type: 'server' | 'request'
  level: string
  message: string
  target: string
  method?: string
  path?: string
  status_code?: number
  referer?: string
  remote_ip?: string
  user_agent?: string
  auth_type?: string
  user_id?: string
  user_collection?: string
}

export interface LogListOptions {
  page?: number
  perPage?: number
  level?: 'trace' | 'debug' | 'info' | 'warn' | 'error'
  logType?: 'server' | 'request'
  q?: string
  target?: string
  path?: string
  statusCode?: number
  from?: string
  to?: string
  signal?: AbortSignal
}

export interface WebProject {
  name: string
  alias: string | null
  spaFallback: boolean
  cacheControl: string
  notFound: string | null
  deployedAt: string
  deployed: boolean
}

export interface WebProjectPatch {
  alias?: string | null
  spaFallback?: boolean
  cacheControl?: string
  notFound?: string | null
}

export interface WebProjectDeploy extends WebProjectPatch {
  archive: UploadFile
}

export interface DeletedResource {
  name: string
  deleted: boolean
}
