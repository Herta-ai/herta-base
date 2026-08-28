export { AuthClient } from './auth'
export { MemoryAuthStore } from './auth-store'
export { HertaBaseClient, HertaBaseClient as HertaBaseSDK } from './client'
export { CollectionClient } from './collection'
export { HertaError, type HertaErrorKind, isHertaError } from './errors'
export { FilesClient } from './files'
export type {
  ApiEnvelope,
  ApiErrorPayload,
  AuthChangeListener,
  AuthResponse,
  AuthScope,
  AuthSession,
  AuthStore,
  AuthUser,
  ChangeEventData,
  ConnectedEventData,
  Credentials,
  FetchLike,
  FileAccessOptions,
  FileMutationOptions,
  FileReference,
  FileTokenRequest,
  FileTokenResponse,
  GetOptions,
  HertaBaseClientOptions,
  HertaRecord,
  HertaRequestOptions,
  ListOptions,
  MutationOptions,
  Page,
  PingEventData,
  ProfiledAuthUser,
  RealtimeEvent,
  RealtimeStatus,
  RealtimeSubscription,
  ReconnectOptions,
  RecordUpload,
  SubscribeOptions,
  UploadFile,
} from './types'
