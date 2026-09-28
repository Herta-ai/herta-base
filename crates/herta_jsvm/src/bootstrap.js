(function (nativeCall, nativeLog, nativeEnv, config, nativeCheckpoint) {
  "use strict";
  const {Object, Array, WeakMap, WeakSet, Map, Set, Promise, Error, InternalError, String, Number,
    Uint8Array, Reflect, JSON, Symbol, Proxy} = globalThis;
  // The controller's private state must not be observable through patched
  // WeakMap/Array/Promise methods or inherited accessors installed by a script.
  const hardened = new WeakSet();
  function harden(value) {
    if (!value || !["object", "function"].includes(typeof value) || hardened.has(value)) return;
    hardened.add(value);
    for (const key of Reflect.ownKeys(value)) {
      const descriptor = Object.getOwnPropertyDescriptor(value, key);
      if (descriptor.value) harden(descriptor.value);
      if (descriptor.get) harden(descriptor.get);
      if (descriptor.set) harden(descriptor.set);
    }
    harden(Object.getPrototypeOf(value));
    Object.freeze(value);
  }
  for (const intrinsic of [Object, Array, WeakMap, WeakSet, Map, Set, Promise,
    Error, InternalError, String, Number, Uint8Array, Reflect, JSON, Symbol, Proxy]) harden(intrinsic);
  const stringify = JSON.stringify.bind(JSON), parse = JSON.parse.bind(JSON);
  const freeze = Object.freeze, define = Object.defineProperty;
  const own = (value, name) => Object.prototype.hasOwnProperty.call(value, name);
  const copy = value => value === undefined ? undefined : parse(stringify(value));
  const immutable = value => {
    if (value && typeof value === "object") {
      for (const key of Object.keys(value)) immutable(value[key]);
      freeze(value);
    }
    return value;
  };
  const expose = (name, value) => define(globalThis, name, {value, writable: false, configurable: false});
  const publicErrors = new WeakMap(), records = new WeakMap(), responses = new WeakSet();
  const profiles = new WeakMap();
  const collections = new WeakMap();
  const authFields = ["id","email","password","passwordConfirm","password_hash","token_key","_hb_auth_issuance","verified","role",
    "failed_attempts","locked_until","accessToken","refreshToken","created_at","updated_at","deleted_at",
    "createdAt","updatedAt","collection","admin","access_token","refresh_token"];
  const frames = new WeakMap(), frameStack = [];
  let current = {authMode: "system", script: "", transaction: null, writes: [], after: []};
  let loading = true, script = "", invocation = null, sharedContext = {};
  let requestBytes, requestText;
  const registrations = [], handlers = new Map();
  let sequence = 0;

  function fail(code, status, message, details = null) {
    const error = new Error(message);
    const descriptor = immutable({code, status, message, details: copy(details)});
    for (const key of ["code", "message", "details"]) define(error, key, {value: descriptor[key]});
    publicErrors.set(error, descriptor);
    return error;
  }
  function hookError(message) { return fail("HB_HOOK_ERROR", 500, message); }
  for (const [name, status, code] of [
    ["BadRequestError",400,"HB_VALIDATION_ERROR"], ["ForbiddenError",403,"HB_FORBIDDEN"],
    ["NotFoundError",404,"HB_NOT_FOUND"], ["UnauthorizedError",401,"HB_UNAUTHORIZED"],
    ["ConflictError",409,"HB_CONFLICT"],
  ]) {
    expose(name, class extends Error {
      constructor(message = name, details = null) {
        super(String(message));
        const descriptor = immutable({code, status, message: String(message), details: copy(details)});
        publicErrors.set(this, descriptor);
        for (const key of ["code", "details"]) define(this, key, {value: descriptor[key]});
      }
    });
  }

  async function host(operation, arguments_ = {}, bytes) {
    if (loading) throw fail("HB_CAPABILITY_DENIED",403,"Host I/O is prohibited during initialization");
    const reply = await nativeCall(stringify({operation, arguments: arguments_,
      authMode: current.authMode, script: current.script, transaction: current.transaction}), ...(bytes === undefined ? [] : [bytes]));
    const response = parse(reply.json);
    if (!response.ok) {
      const error = response.error;
      throw fail(error.code, error.status, error.message, error.details);
    }
    return reply.bytes === undefined ? response.value
      : operation === 'files.response' ? {...response.value,bytes:reply.bytes} : reply.bytes;
  }

  function scoped(frame, callback) {
    const previous = current;
    current = frame;
    try { return callback(); } finally { current = previous; }
  }

  function options(value, defaultMode) {
    if (value === undefined) return {authMode: defaultMode, collections: []};
    if (!value || typeof value !== "object" || Array.isArray(value)) throw new BadRequestError("Invalid registration options");
    const result = copy(value);
    result.authMode ??= defaultMode;
    result.collections ??= [];
    if (!["system", "request"].includes(result.authMode) || !Array.isArray(result.collections)
      || result.collections.some(name => typeof name !== "string" || !name)) throw new BadRequestError("Invalid hook identity or collection filter");
    return result;
  }

  function register(kind, name, handler, opts) {
    if (!loading) throw hookError("Registrations are only allowed during initialization");
    if (typeof handler !== "function") throw new BadRequestError("Handler must be a function");
    if (registrations.length >= config.max_registrations) throw new BadRequestError("Registration limit exceeded");
    const id = sequence++;
    const descriptor = {id, kind, name, script, collections: opts.collections ?? [], options: opts};
    registrations.push(descriptor); handlers.set(id, handler);
    return id;
  }

  const eventNames = {
    onRecordCreate: "record.create", onRecordUpdate: "record.update", onRecordDelete: "record.delete",
    onRecordListRequest: "record.listRequest", onRecordViewRequest: "record.viewRequest",
    onRecordCreateRequest: "record.createRequest", onRecordUpdateRequest: "record.updateRequest", onRecordDeleteRequest: "record.deleteRequest",
    onCollectionCreate: "collection.create", onCollectionUpdate: "collection.update", onCollectionDelete: "collection.delete",
    onAuthLogin: "auth.login", onAuthRegister: "auth.register", onTokenRefresh: "auth.refresh",
    onBootstrap: "bootstrap", onServe: "serve", onShutdown: "shutdown",
  };
  for (const [globalName, eventName] of Object.entries(eventNames)) {
    expose(globalName, (handler, ...filters) => {
      const opts = filters.length === 1 && typeof filters[0] === "object"
        ? options(filters[0], "system") : options({collections: filters}, "system");
      if (["bootstrap", "serve", "shutdown"].includes(eventName) && (opts.authMode !== "system" || opts.collections.length))
        throw new BadRequestError("Lifecycle hooks only support system mode without collection filters");
      register("hook", eventName, handler, opts);
    });
  }

  expose("routerAdd", (method, path, handler, ...middleware) => {
    let opts;
    if (typeof method === "object") {
      opts = {...method}; handler = path;
    } else opts = {method, path, middleware};
    opts.middleware ??= [];
    if (!Array.isArray(opts.middleware)) throw new BadRequestError("middleware must be an array");
    const jsMiddleware = opts.middleware.filter(item => typeof item === "function");
    opts.middleware = opts.middleware.filter(item => typeof item !== "function");
    opts = options(opts, "request");
    const original = handler;
    if (typeof original !== "function") throw new BadRequestError("Handler must be a function");
    register("route", `${opts.method} ${opts.path}`, async e => {
      async function run(index) {
        if (index === jsMiddleware.length) return original(e);
        let called = false, active = true, done = false, invalid = false, downstream;
        const local = Object.create(e);
        define(local, "next", {value: () => {
          if (called || !active) { invalid = true; throw hookError("Middleware next called twice or after completion"); }
          called = true;
          downstream = run(index + 1).finally(() => { done = true; });
          return downstream;
        }});
        let result;
        try { result = await jsMiddleware[index](local); } finally { active = false; }
        if (invalid || (called && !done)) throw hookError("Middleware next did not complete correctly");
        if (!called && !responses.has(result)) throw hookError("Middleware must return a response or call next");
        return result ?? (called ? await downstream : undefined);
      }
      return run(0);
    }, opts);
  });
  expose("$apis", freeze({
    requireAuth: () => freeze({kind: "requireAuth"}), requireAdmin: () => freeze({kind: "requireAdmin"}),
    bodyLimit: bytes => freeze({kind: "bodyLimit", bytes}),
    rateLimit: value => freeze({kind: "rateLimit", ...copy(value)}),
  }));
  expose("cronAdd", (name, expression, handler, opts = {}) => {
    if (!opts || typeof opts !== "object" || Array.isArray(opts) || Object.keys(opts).some(key =>
      !["timezone", "maxRuntimeMs", "retries", "idempotent"].includes(key))) throw new BadRequestError("Invalid cron options");
    if (registrations.some(item => item.kind === "cron" && item.name === name)) throw new BadRequestError("Duplicate cron name");
    register("cron", name, handler, {...copy(opts), expression, authMode: "system"});
  });
  expose("cronRemove", name => {
    if (!loading) throw hookError("cronRemove is only available during initialization");
    const index = registrations.findIndex(item => item.kind === "cron" && item.name === name);
    if (index >= 0) { handlers.delete(registrations[index].id); registrations.splice(index, 1); }
  });

  class RecordModel {
    constructor(collection, data, original = null, isNew = false, readOnly = false) {
      const candidate = copy(data);
      records.set(this, {collection, candidate, original: copy(original), isNew, readOnly, dirty: {}, unset: new Set(), valid: true});
      define(this, "id", {get: () => state(this).candidate.id});
      define(this, "collectionName", {value: collection});
      freeze(this);
    }
    get(field) { return copy(state(this).candidate[field]); }
    original(field) { return copy(state(this).original?.[field]); }
    isNew() { return state(this).isNew; }
    toJSON() { return copy(state(this).candidate); }
    set(field, value) {
      const record = writable(this, field);
      if (value === undefined) throw new BadRequestError("undefined is not a record value");
      record.candidate[field] = copy(value); record.dirty[field] = copy(value); record.unset.delete(field);
    }
    unset(field) {
      const record = writable(this, field);
      delete record.candidate[field]; delete record.dirty[field]; record.unset.add(field);
    }
  }
  freeze(RecordModel.prototype);
  function recordData(record) { const metadata = records.get(record); return metadata?.target ?? metadata; }
  function recordView(record, canWrite) {
    const view = Object.create(RecordModel.prototype);
    records.set(view, {target: state(record), canWrite});
    define(view, "id", {get: () => state(view).candidate.id});
    define(view, "collectionName", {value: record.collectionName});
    return freeze(view);
  }
  function state(record) {
    const result = recordData(record);
    if (!result?.valid) throw hookError("Invalid or rolled-back record handle");
    return result;
  }
  function writable(record, field) {
    const result = state(record);
    const metadata = records.get(record);
    if (result.readOnly || (metadata.canWrite && !metadata.canWrite())) throw new BadRequestError("Record is read-only");
    if (typeof field !== "string" || ["id", "created_at", "updated_at", "deleted_at", "__proto__", "constructor", "prototype"].includes(field))
      throw new BadRequestError("Protected record field");
    if (result.protectedFields?.includes(field)) throw new BadRequestError("Protected Auth field");
    return result;
  }

  function profileData(record) {
    return Object.fromEntries(Object.entries(state(record).candidate).filter(([key]) => !authFields.includes(key)));
  }
  function profileView(record, canWrite) {
    const view = recordView(record, canWrite);
    return new Proxy({}, {
      get: (_, key) => own(profileData(record), key) ? copy(state(record).candidate[key]) : undefined,
      ownKeys: () => Object.keys(profileData(record)),
      has: (_, key) => own(profileData(record), key),
      getOwnPropertyDescriptor: (_, key) => own(profileData(record), key)
        ? {value:copy(state(record).candidate[key]),writable:true,enumerable:true,configurable:true} : undefined,
      set: (_, key, value) => { view.set(key, value); return true; },
      deleteProperty: (_, key) => { view.unset(key); return true; },
      defineProperty: () => { throw new BadRequestError("Use profile assignment"); },
      setPrototypeOf: () => false,
      preventExtensions: () => false,
    });
  }

  function response(status, kind, body, headers = {}) {
    const result = freeze({status, kind, body, headers: copy(headers)});
    responses.add(result); return result;
  }
  function requestEvent(payload) {
    const request = immutable(copy(payload.request ?? {}));
    const body = requestText ?? request.body ?? "";
    const view = freeze({
      method: request.method, path: request.path,
      pathValue: name => request.params?.[name] ?? null,
      query: name => request.query?.[name] ?? null,
      header: name => request.headers?.[String(name).toLowerCase()] ?? null,
      json: async () => { try { if (request.validUtf8 === false) throw new Error(); return typeof body === "string" ? parse(body) : copy(body); } catch { throw new BadRequestError("Invalid JSON body"); } },
      text: async () => typeof body === "string" ? body : stringify(body),
      bytes: async () => new Uint8Array(requestBytes ?? request.bytes ?? []),
    });
    return {
      request: view, auth: immutable(copy(payload.auth ?? null)), response: null,
      json(status, data, meta = null) {
        if (!Number.isInteger(status) || status < 200 || status >= 300 || status === 204) throw new BadRequestError("json requires a 2xx status with a body");
        return response(status, "json", {data, meta, error: null});
      },
      rawJson: (status, value) => response(status, "json", value),
      text: (status, value) => response(status, "text", String(value)),
      html: (status, value) => response(status, "html", String(value)),
      noContent: () => response(204, "empty", null),
      file: async key => { const file = await host("files.response", {key}); return response(200, "bytes", file.bytes instanceof Uint8Array ? file.bytes : new Uint8Array(file.bytes), file.headers); },
    };
  }

  async function chain(name, event, terminal, allowResponse = false) {
    const collection = event.collection?.name ?? event.record?.collectionName;
    const list = registrations.filter(item => item.kind === "hook" && item.name === name
      && (!item.collections.length || item.collections.includes(collection)));
    async function run(index) {
      if (index === list.length) return terminal();
      const item = list[index], parent = current;
      const frame = {...parent, script: item.script,
        authMode: parent.authMode === "request" ? "request" : item.options.authMode};
      return scoped(frame, async () => {
        let called = false, done = false, active = true, invalid = false, downstream;
        // Do not expose the backing event via Object.getPrototypeOf(local):
        // it owns unrestricted record/collection handles used by the controller.
        const local = {};
        for(const key of Object.keys(event)) {
          if(key === 'record' || key === 'next' || key === 'afterCommit' || key === 'authMode'
            || (key === 'collection' && collections.has(event)))continue;
          define(local,key,{get:()=>event[key],enumerable:true});
        }
        define(local, "authMode", {value: frame.authMode});
        if (event.record) define(local, "record", {value: recordView(event.record, () => active && !called)});
        if (profiles.has(event)) define(local, "profile", {value: profileView(profiles.get(event), () => active && !called)});
        if (collections.has(event)) define(local, "collection", {value: collectionView(collections.get(event), () => active && !called)});
        if (event.afterCommit) define(local, "afterCommit", {value: callback => {
          if (!active) throw hookError("afterCommit called after handler completion");
          event.afterCommit(callback);
        }});
        define(local, "next", {value: () => {
          if (!active || called) { invalid = true; throw hookError("next called twice or after handler completion"); }
          called = true;
          downstream = run(index + 1).finally(() => { done = true; });
          return downstream.then(() => undefined);
        }});
        freeze(local);
        let result;
        try { result = await handlers.get(item.id)(local); } finally { active = false; }
        if (invalid || (called && !done)) throw hookError("Handler next did not complete correctly");
        if (!called) {
          if (allowResponse && responses.has(result)) return result;
          throw allowResponse ? hookError("Request hook must return a response") : fail("HB_HOOK_ABORTED",409,"Hook did not call next");
        }
        const downstreamResult = await downstream;
        return result ?? downstreamResult;
      });
    }
    return run(0);
  }

  async function transactional(callback, checkpoint = false) {
    if (current.transaction) throw new ConflictError("Nested transactions are not supported");
    const transaction = await host("transaction.begin");
    const frame = {...current, transaction: transaction.id, after: [], handles: [], open: true};
    return scoped(frame, async () => {
      let result;
      try {
        result = await callback();
        frame.open = false;
        await host("transaction.commit");
      } catch (error) {
        frame.open = false;
        await host("transaction.rollback").catch(() => {});
        for (const handle of frame.handles) recordData(handle).valid = false;
        throw error;
      }
      if (checkpoint && nativeCheckpoint) nativeCheckpoint(stringify(invocation.name.endsWith("Request")
        ? {status: invocation.name === "record.createRequest" ? 201 : 200, kind:"json", body:{data:result,meta:null,error:null},headers:{}}
        : result));
      await scoped({...frame, transaction: null}, async () => {
        for (const item of frame.after) {
          try { await scoped({...current, authMode: item.mode, script: item.script}, () => item.callback(item.snapshot())); }
          catch (error) { nativeLog("error", "afterCommit callback failed", stringify({error: String(error)}), current.script); }
        }
      });
      if (checkpoint && nativeCheckpoint) nativeCheckpoint(null);
      return result;
    });
  }

  async function persist(action, record, requestBody = null, root = false, auth = null) {
    if (!current.transaction) return transactional(() => persist(action, record, requestBody), root);
    const data = state(record), id = String(data.candidate.id);
    const key = `${data.collection}/${id.startsWith(data.collection + ':') ? id.slice(data.collection.length + 1) : id}`;
    if (current.writes.includes(key) || current.writes.length >= config.max_hook_depth) {
      await host('transaction.markRollbackOnly').catch(()=>{});
      throw fail("HB_HOOK_RECURSION",409,"Recursive record write rejected");
    }
    return scoped({...current, writes: [...current.writes, key]}, async () => {
      try {
        current.handles.push(record);
        const prepared = auth ? {candidate: data.candidate, original: null, collection: auth.collection}
          : await host("record.prepare", {action, collection: data.collection, id: data.candidate.id,
            candidate: data.candidate, dirty: data.dirty, unset: [...data.unset], requestBody});
        data.candidate = copy(prepared.candidate); data.original = copy(prepared.original);
        let committedCandidate;
        const event = {name: `record.${action}`, requestId: invocation.requestId, context: sharedContext,
          record, originalRecord: prepared.original ? new RecordModel(data.collection, prepared.original, prepared.original, false, true) : null,
          collection: immutable(prepared.collection),
          afterCommit(callback) {
            if (typeof callback !== "function") throw new BadRequestError("afterCommit needs a callback");
            current.after.push({callback, mode: current.authMode, script: current.script, snapshot: () => freeze({
              name: event.name, collection: event.collection,
              record: new RecordModel(data.collection, committedCandidate, committedCandidate, false, true),
              originalRecord: event.originalRecord, requestId: event.requestId,
            })});
          },
        };
        if (action === "delete") data.readOnly = true;
        await chain(event.name, event, async () => {
          const saved = auth ? await host("auth.complete", {profile: data.candidate})
            : await host("record.write", {action, collection: data.collection, id: data.candidate.id,
              candidate: data.candidate, dirty: data.dirty, unset: [...data.unset], requestBody});
          data.candidate = copy(saved); data.readOnly = true; data.isNew = false;
        });
        committedCandidate = immutable(copy(data.candidate));
        data.readOnly = action === "delete"; data.dirty = {}; data.unset.clear();
        return record;
      } catch (error) {
        await host("transaction.markRollbackOnly").catch(() => {}); throw error;
      }
    });
  }

  async function authenticate(payload) {
    return transactional(async () => {
      let account = immutable(copy(payload.account ?? null));
      const registration = invocation.name === "auth.register";
      const profile = registration ? new RecordModel(payload.collection.name, {...payload.profile,id:payload.id}, null, true) : null;
      if (profile) state(profile).protectedFields = authFields;
      const event = {name: invocation.name, collection: immutable(copy(payload.collection)),
        requestId: invocation.requestId, context: sharedContext,
        request: requestEvent(payload).request, auth: immutable(copy(payload.auth)),
        get account() { return account; },
        afterCommit(callback) {
          if (typeof callback !== "function") throw new BadRequestError("afterCommit needs a callback");
          current.after.push({callback, mode: current.authMode, script: current.script, snapshot: () => immutable({
            name: event.name, collection: event.collection, requestId: event.requestId,
            account: copy(account), ...(profile ? {profile: copy(profileData(profile))} : {}),
          })});
        },
      };
      if (profile) profiles.set(event, profile);
      await chain(invocation.name, event, async () => {
        if (profile) {
          await persist("create", profile, null, false, payload);
          state(profile).readOnly = true;
          account = immutable(profile.toJSON());
        } else account = immutable(await host("auth.complete"));
      });
      return account;
    }, true);
  }

  async function mutateRecord(action, record) {
    try {
      const data=state(record),metadata=records.get(record),id=String(data.candidate.id);
      const key=`${data.collection}/${id.startsWith(data.collection+':')?id.slice(data.collection.length+1):id}`;
      if(current.writes.includes(key))return await persist(action ?? (data.isNew?'create':'update'),record);
      if(data.readOnly || (metadata.canWrite && !metadata.canWrite()))throw new BadRequestError('Record is read-only');
      return await persist(action ?? (data.isNew?'create':'update'),record);
    } catch(error) {
      if(current.transaction)await host('transaction.markRollbackOnly').catch(()=>{});
      throw error;
    }
  }

  function collectionView(data, canWrite) {
    return new Proxy({}, {
      get: (_, key) => own(data.candidate,key) ? copy(data.candidate[key]) : undefined,
      ownKeys: () => Object.keys(data.candidate),
      has: (_, key) => own(data.candidate,key),
      getOwnPropertyDescriptor: (_, key) => own(data.candidate,key)
        ? {value:copy(data.candidate[key]),writable:true,enumerable:true,configurable:true} : undefined,
      set: (_, key, value) => {
        if(data.readOnly || !canWrite())throw new BadRequestError('Collection is read-only');
        if(!['fields','indexes','rules','type','schema_mode'].includes(key))throw new BadRequestError('Protected collection field');
        data.candidate[key]=copy(value);return true;
      },
      deleteProperty: () => {throw new BadRequestError('Collection properties cannot be removed')},
      defineProperty: () => {throw new BadRequestError('Use collection assignment')},
      setPrototypeOf: () => false, preventExtensions: () => false,
    });
  }

  async function persistCollection(action, value, patch = false, root = false) {
    if(current.transaction) {
      await host('transaction.markRollbackOnly');
      throw new ConflictError('Schema changes require a separate top-level transaction');
    }
    return transactional(async () => {
      const prepared = await host('collections.prepare',{action,value,patch});
      const data = {candidate:copy(prepared.candidate),readOnly:action==='delete'};
      let committed;
      const event = {name:`collection.${action}`,collection:data.candidate,
        originalCollection:immutable(copy(prepared.original)),requestId:invocation.requestId,context:sharedContext,
        afterCommit(callback) {
          if(typeof callback!=='function')throw new BadRequestError('afterCommit needs a callback');
          current.after.push({callback,mode:current.authMode,script:current.script,snapshot:()=>immutable({
            name:event.name,collection:copy(committed),originalCollection:event.originalCollection,requestId:event.requestId,
          })});
        },
      };
      collections.set(event,data);
      await chain(event.name,event,async()=>{
        data.candidate=await host('collections.write',{action,original:prepared.original,candidate:data.candidate});
        data.readOnly=true;
      });
      committed=immutable(copy(data.candidate));
      return committed;
    },root);
  }

  const app = {
    logger: freeze(Object.fromEntries(["trace","debug","info","warn","error"].map(level => [level,
      (message, fields = {}) => {
        if (loading) throw fail("HB_CAPABILITY_DENIED",403,"Logging is disabled during initialization");
        nativeLog(level, String(message), stringify(fields), current.script);
      }]))),
    env: name => nativeEnv(String(name)),
    newRecord(collection, input = {}) {
      const data = copy(input);
      if (own(data, "id")) throw new BadRequestError("Record IDs are allocated by the host");
      // The host's synchronous UUID source is scoped and has no I/O.
      data.id = nativeEnv("__HB_NEW_RECORD_ID");
      const record = new RecordModel(collection, data, null, true);
      records.get(record).dirty = copy(input); return record;
    },
    async findRecordById(collection, id) {
      const data = await host("record.find", {collection, id}); return new RecordModel(collection, data, data);
    },
    async findRecordsByFilter(collection, filter = "", sort = "-created_at", limit = 30, offset = 0, bindings = {}) {
      const rows = await host("record.list", {collection, filter, sort, limit, offset, bindings});
      return rows.map(data => new RecordModel(collection, data, data));
    },
    async findFirstRecordByFilter(collection, filter = "", bindings = {}) {
      const rows = await app.findRecordsByFilter(collection, filter, "-created_at", 1, 0, bindings);
      if (!rows.length) throw new NotFoundError(); return rows[0];
    },
    save: record => mutateRecord(null, record),
    delete: record => mutateRecord("delete", record),
    transaction: callback => transactional(() => {
      const frame = current;
      return callback(freeze({...app, afterCommit(callback) {
        if (!frame.open || current.transaction !== frame.transaction) throw hookError("transaction callback is no longer active");
        if (typeof callback !== "function") throw new BadRequestError("afterCommit needs a callback");
        frame.after.push({callback, mode: current.authMode, script: current.script, snapshot: () => null});
      }}));
    }),
    db: freeze({query: (query, bindings = {}) => host("db.query", {query, bindings})}),
    mailer: freeze({send: message => host("mail.send", message)}),
    http: freeze({async send(request) {
      const result = await host("http.send", request);
      return freeze({...result, headers: immutable(result.headers), json: () => { try { return parse(result.body); } catch { throw new BadRequestError("Invalid upstream JSON"); } }});
    }}),
    realtime: freeze({publish: (topic, data, audience) => host("realtime.publish", {topic, data, audience})}),
    outbox: freeze({enqueue: (kind, payload, options) => {
      const enqueue = () => host("outbox.enqueue", {kind, payload, ...options});
      return current.transaction ? enqueue() : transactional(enqueue);
    }}),
    collections: freeze({
      findByName: value => host('collections.findByName',{value}),
      list: () => host('collections.list'),
      create: value => persistCollection('create',value),
      save: value => persistCollection('update',value),
      delete: value => persistCollection('delete',value),
    }),
    files: freeze({
      write: (key, value, options = {}) => typeof value === "string"
        ? host("files.write", {key, ...options, text: value})
        : value instanceof Uint8Array ? host("files.write", {key, ...options}, value)
        : (() => {throw new BadRequestError("write expects string or Uint8Array");})(),
      readText: key => host("files.readText", {key}),
      readBytes: async key => { const bytes = await host("files.readBytes", {key}); return bytes instanceof Uint8Array ? bytes : new Uint8Array(bytes); },
      exists: key => host("files.exists", {key}), stat: key => host("files.stat", {key}),
      list: (prefix = "", options = {}) => host("files.list", {prefix, ...options}),
      copy: (source, destination) => host("files.copy", {source, destination}),
      move: (source, destination) => host("files.move", {source, destination}),
      remove: key => host("files.remove", {key}),
    }),
  };
  expose("$app", freeze(app));

  return {
    script(name) { script = name; current.script = name; },
    registrations() { return stringify(registrations); },
    promiseHook(type, promise, parent) {
      if (type === 0) frames.set(promise, current);
      else if (type === 1) { frameStack.push(current); current = frames.get(promise) ?? current; }
      else if (type === 2) { current = frameStack.pop() ?? current; }
    },
    async run(input, bytes, text) {
      invocation = parse(input); loading = false; sharedContext = {};
      requestBytes = bytes;
      requestText = text;
      current = {authMode: invocation.authMode, script: "", transaction: null, writes: [], after: []};
      try {
        let value;
        if (invocation.registration !== null) {
          const item = registrations.find(item => item.id === invocation.registration);
          if (!item) throw hookError("Missing registration");
          current.script = item.script;
          current.authMode = invocation.authMode === "request" ? "request" : item.options.authMode;
          value = await handlers.get(item.id)(item.kind === "route" ? requestEvent(invocation.payload) : immutable(invocation.payload));
          if (item.kind === "route" && !responses.has(value)) throw hookError("Route did not return a response");
        } else if (["bootstrap","serve","shutdown"].includes(invocation.name)) {
          await chain(invocation.name, {name: invocation.name, requestId: null, context: sharedContext}, async () => {});
          value = null;
        } else if (invocation.name.startsWith('collection.')) {
          value=await persistCollection(invocation.name.slice(11),invocation.payload.value,invocation.payload.patch,true);
        } else if (["auth.register","auth.login","auth.refresh"].includes(invocation.name)) {
          value = await authenticate(invocation.payload);
        } else if (invocation.name.startsWith("record.")) {
          const payload = invocation.payload, action = invocation.name.slice(7).replace("Request", "");
          const record = action === 'list' ? null : new RecordModel(payload.collection, {...payload.original, ...payload.input, id: payload.id}, payload.original ?? null, action === "create", action === "delete" || action === "view");
          if (action === "create" || action === "update") records.get(record).dirty = copy(payload.input ?? {});
          const event = {...requestEvent(payload), name: invocation.name, requestId: invocation.requestId,
            context: sharedContext, ...(record ? {record, originalRecord: payload.original ? new RecordModel(payload.collection,payload.original,payload.original,false,true) : null} : {records:null}),
            collection: immutable(payload.schema ?? {name: payload.collection}), query: immutable(payload.query ?? {})};
          let result;
          const terminal = async () => {
            if (["create","update","delete"].includes(action)) {
              result = await persist(action, record, payload.requestBody, true);
              event.response = event.json(action === "create" ? 201 : 200, result);
            } else if (action === "list") {
              const page = await host("record.listPage", {collection: payload.collection, ...payload.query});
              result = page.rows;
              event.records = immutable(copy(result));
              event.response = event.json(200, result, {total: page.total, page: payload.page, perPage: payload.perPage});
            } else {
              result = await host("record.find", payload);
              state(record).candidate = copy(result);
              event.response = event.json(200, result);
            }
          };
          if (invocation.name.endsWith("Request")) {
            const returned = await chain(invocation.name, event, terminal, true);
            value = responses.has(returned) ? returned : event.response;
          } else { await terminal(); value = result; }
        } else {
          value = await host("event.execute", invocation);
        }
        if (responses.has(value) && value.kind === 'bytes')
          return {json:stringify({ok:true,value:{...value,body:null}}),bytes:value.body};
        return {json:stringify({ok: true, value: value ?? null})};
      } catch (error) {
        const known = publicErrors.get(error);
        return {json:stringify({ok: false, error: known ?? {code:
          error instanceof InternalError && error.message === "out of memory" ? "HB_HOOK_OOM" : "HB_HOOK_ERROR",status:500,
          message: String(error), details:null, stack: String(error?.stack ?? "")}})};
      }
    },
  };
})
