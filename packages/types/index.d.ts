/// <reference lib="es2022" />

declare global {
  interface RecordEvent {
    record: {
      get(key: string): unknown;
      set(key: string, value: unknown): void;
    };
    auth?: {
      id: string;
      [key: string]: unknown;
    };
    next(): Promise<void>;
  }

  interface RequestEvent {
    json(status: number, data: unknown): unknown;
  }

  interface AppLogger {
    info(msg: string, ctx?: Record<string, unknown>): void;
    warn(msg: string, ctx?: Record<string, unknown>): void;
    error(msg: string, ctx?: Record<string, unknown>): void;
    debug(msg: string, ctx?: Record<string, unknown>): void;
  }

  interface App {
    logger: AppLogger;
  }

  const $app: App;

  function onRecordCreate(
    handler: (event: RecordEvent) => void | Promise<void>,
    ...collections: string[]
  ): void;

  function routerAdd(
    method: string,
    path: string,
    handler: (event: RequestEvent) => unknown
  ): void;

  function cronAdd(name: string, expression: string, handler: () => unknown): void;
}

export {};
