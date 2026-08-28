import type { Transport } from './transport'
import type {
  FileMutationOptions,
  GetOptions,
  HertaRecord,
  ListOptions,
  MutationOptions,
  Page,
  QueryValue,
  RealtimeSubscription,
  RecordUpload,
  SubscribeOptions,
} from './types'
import { subscribeToCollection } from './realtime'
import { encodePath, uploadForm } from './utils'

export class CollectionClient<
  TRecord extends HertaRecord = HertaRecord,
  TCreate extends object = Record<string, unknown>,
  TUpdate extends object = Partial<TCreate>,
> {
  constructor(
    private readonly transport: Transport,
    readonly name: string,
  ) {}

  async list(options: ListOptions = {}): Promise<Page<TRecord>> {
    const result = await this.transport.requestWithMeta<TRecord[]>(this.rootPath(), {
      query: {
        page: options.page,
        perPage: options.perPage,
        sort: options.sort,
        filter: options.filter,
        expand: options.expand,
      },
      signal: options.signal,
      timeoutMs: options.timeoutMs,
    })
    return {
      items: result.data,
      total: numberMeta(result.meta, 'total', result.data.length),
      page: numberMeta(result.meta, 'page', options.page ?? 1),
      perPage: numberMeta(result.meta, 'perPage', options.perPage ?? 30),
    }
  }

  get(id: string, options: GetOptions = {}): Promise<TRecord> {
    return this.transport.request(this.recordPath(id), {
      query: { expand: options.expand },
      signal: options.signal,
      timeoutMs: options.timeoutMs,
    })
  }

  create(data: TCreate, options: MutationOptions = {}): Promise<TRecord> {
    return this.transport.request(this.rootPath(), {
      method: 'POST',
      body: data,
      signal: options.signal,
      timeoutMs: options.timeoutMs,
    })
  }

  createWithFiles(upload: RecordUpload<TCreate>, options: MutationOptions = {}): Promise<TRecord> {
    return this.transport.request(this.rootPath(), {
      method: 'POST',
      body: uploadForm(upload),
      signal: options.signal,
      timeoutMs: options.timeoutMs,
    })
  }

  update(id: string, data: TUpdate, options: MutationOptions = {}): Promise<TRecord> {
    return this.transport.request(this.recordPath(id), {
      method: 'PATCH',
      body: data,
      signal: options.signal,
      timeoutMs: options.timeoutMs,
    })
  }

  updateWithFiles(
    id: string,
    upload: RecordUpload<TUpdate>,
    options: FileMutationOptions = {},
  ): Promise<TRecord> {
    const query: Record<string, QueryValue> = { appendFiles: options.appendFiles }
    return this.transport.request(this.recordPath(id), {
      method: 'PATCH',
      query,
      body: uploadForm(upload),
      signal: options.signal,
      timeoutMs: options.timeoutMs,
    })
  }

  delete(id: string, options: MutationOptions = {}): Promise<TRecord> {
    return this.transport.request(this.recordPath(id), {
      method: 'DELETE',
      signal: options.signal,
      timeoutMs: options.timeoutMs,
    })
  }

  subscribe(options: SubscribeOptions<TRecord> = {}): Promise<RealtimeSubscription<TRecord>> {
    return subscribeToCollection(this.transport, this.name, options)
  }

  private rootPath(): string {
    return `/api/collections/${encodePath(this.name)}/records`
  }

  private recordPath(id: string): string {
    return `${this.rootPath()}/${encodePath(id)}`
  }
}

function numberMeta(meta: Record<string, unknown> | null, key: string, fallback: number): number {
  const value = meta?.[key]
  return typeof value === 'number' ? value : fallback
}
