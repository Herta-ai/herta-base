import type { ApiErrorPayload, RequestContext } from './types'

export type HertaErrorKind = 'api' | 'network' | 'timeout' | 'abort' | 'protocol' | 'configuration'

export class HertaError extends Error {
  readonly kind: HertaErrorKind
  readonly status?: number
  readonly code?: string
  readonly details?: unknown
  readonly request?: RequestContext

  constructor(
    message: string,
    options: {
      kind: HertaErrorKind
      status?: number
      code?: string
      details?: unknown
      request?: RequestContext
      cause?: unknown
    },
  ) {
    super(message, options.cause === undefined ? undefined : { cause: options.cause })
    this.name = 'HertaError'
    this.kind = options.kind
    this.status = options.status
    this.code = options.code
    this.details = options.details
    this.request = options.request
  }

  static api(error: ApiErrorPayload, request: RequestContext): HertaError {
    return new HertaError(error.message, {
      kind: 'api',
      status: error.code,
      code: error.error,
      details: error.details,
      request,
    })
  }
}

export function isHertaError(error: unknown): error is HertaError {
  return error instanceof HertaError
}
