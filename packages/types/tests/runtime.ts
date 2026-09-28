import '../index';
onRecordCreate(async e => {
  e.record.set('title', 'hello');
  e.afterCommit(snapshot => {
    // @ts-expect-error committed record is read-only
    snapshot.record.set('title', 'late');
  });
  await e.next();
}, { collections: ['posts'], authMode: 'request' });
onRecordDelete(async e => {
  // @ts-expect-error delete candidates are read-only
  e.record.unset('title');
  await e.next();
}, 'posts');
onRecordListRequest(async e => {
  // @ts-expect-error list events do not have a single record
  e.record.get('id');
  // @ts-expect-error request events cannot register commit callbacks
  e.afterCommit(() => {});
  await e.next();
  return e.json(200, e.records);
});
onAuthRegister(async e => { e.profile.name = 'welcome'; await e.next(); });
cronAdd('daily-report', '0 0 8 * * *', async context => {
  $app.logger.info(context.runId, { attempt: context.attemptId, scheduledAt: context.scheduledAt });
  // @ts-expect-error cron is not a middleware event
  await context.next();
  // @ts-expect-error task identity is immutable
  context.runId = 'other';
}, { timezone: 'Asia/Shanghai', maxRuntimeMs: 2000, idempotent: true, retries: 1 });
onTokenRefresh(async e => {
  // @ts-expect-error account is read-only
  e.account.role = 'admin';
  // @ts-expect-error tokens are not exposed on events
  e.refreshToken;
  await e.next();
});
onCollectionUpdate(async e => {
  e.collection.fields = [...(e.collection.fields ?? []), { name: 'added', type: 'number' }];
  // @ts-expect-error collection names cannot be changed
  e.collection.name = 'renamed';
  await e.next();
});
onCollectionDelete(async e => {
  // @ts-expect-error deletion definitions are immutable
  e.collection.rules = {};
  await e.next();
});
routerAdd('GET', '/api/custom/health', e => e.noContent(), $apis.requireAdmin());
routerAdd('GET', '/api/custom/download', e => e.file('exports/report.json'), $apis.requireAdmin());
onServe(async e => {
  const item: HbFileItem = await $app.files.write('exports/report.json', '{}', { contentType: 'application/json' });
  await $app.files.write('binary', new Uint8Array([1, 2, 3]));
  const bytes: Uint8Array = await $app.files.readBytes('binary');
  const page = await $app.files.list('exports', { limit: 1 });
  await $app.files.list('exports', { cursor: page.nextCursor });
  // @ts-expect-error file writes accept text and Uint8Array only
  await $app.files.write('bad', [1, 2, 3]);
  $app.logger.info(item.key, { size: bytes.length });
  await e.next();
});
// @ts-expect-error routes must return native response descriptors
routerAdd('GET', '/api/custom/invalid', () => ({status: 200}));
onBootstrap(async e => {
  await $app.transaction(async tx => {
    await tx.save(tx.newRecord('audit', { message: 'boot' }));
    await tx.outbox.enqueue('mail.send', { to: [{ address: 'reader@example.com' }], subject: 'committed', text: 'body' }, { idempotencyKey: 'bootstrap' });
    // @ts-expect-error unsupported outbox task
    await tx.outbox.enqueue('shell.run', { command: 'echo' }, { idempotencyKey: 'bad' });
    // @ts-expect-error HTTP jobs require HTTP request DTO
    await tx.outbox.enqueue('http.send', { subject: 'bad' }, { idempotencyKey: 'bad' });
    // @ts-expect-error nested transaction is unsupported
    await tx.transaction(async () => {});
    tx.afterCommit(() => { $app.logger.trace('committed'); });
  });
  const mail = await $app.mailer.send({ to: [{ address: 'reader@example.com' }], subject: 'hello', text: 'body' });
  const accepted: 'accepted' = mail.status;
  $app.logger.info(accepted);
  const response = await $app.http.send({ url: 'https://upstream.example/events', method: 'POST', body: '{}' });
  await $app.realtime.publish('reports/ready', { reportId: 'one' }, { users: [{ collection: 'members', id: 'one' }] });
  // @ts-expect-error audience is required
  await $app.realtime.publish('reports/ready', {});
  // @ts-expect-error audiences cannot be mixed
  await $app.realtime.publish('reports/ready', {}, { roles: [{ collection: 'members', role: 'user' }], connections: ['one'] });
  const upstream: HbJson = response.json();
  $app.logger.info(response.body, { upstream, status: response.status, ok: response.ok });
  // @ts-expect-error outbound responses are immutable
  response.headers.authorization = 'token';
  // @ts-expect-error raw sockets and CONNECT are not exposed
  await $app.http.send({ url: 'https://upstream.example', method: 'CONNECT' });
  await e.next();
});
