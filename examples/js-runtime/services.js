/// <reference path="../../packages/types/index.d.ts" />
cronAdd('demo-minute', '0 * * * * *', async task => {
  $app.logger.info('Scheduled report tick', { runId: task.runId, scheduledAt: task.scheduledAt });
}, { timezone: 'UTC', maxRuntimeMs: 1000 });

routerAdd('GET', '/api/custom/demo-http', async e => {
  const response = await $app.http.send({ url: 'https://example.com/', timeoutMs: 3000 });
  return e.json(200, { status: response.status, body: response.body });
}, $apis.requireAdmin());

routerAdd('POST', '/api/custom/demo-mail', async e => {
  const input = await e.request.json();
  const receipt = await $app.mailer.send({ to: [{ address: input.address }], subject: 'HertaBase demo', text: 'Hello from the extension runtime' });
  return e.json(200, receipt);
}, $apis.requireAdmin(), $apis.bodyLimit(4096));

routerAdd('POST', '/api/custom/demo-message', async e => {
  const input = await e.request.json();
  const receipt = await $app.realtime.publish('reports/ready', { reportId: input.reportId },
    { users: [{ collection: 'demo_members', id: input.memberId }] });
  return e.json(200, receipt);
}, $apis.requireAdmin(), $apis.bodyLimit(4096));

routerAdd('POST', '/api/custom/demo-file', async e => {
  const input = await e.request.json();
  const file = await $app.files.write('exports/report.json', JSON.stringify(input), { contentType: 'application/json' });
  return e.json(201, file);
}, $apis.requireAdmin(), $apis.bodyLimit(4096));
routerAdd('GET', '/api/custom/demo-file', e => e.file('exports/report.json'), $apis.requireAdmin());

routerAdd('POST', '/api/custom/demo-outbox', async e => {
  const input = await e.request.json();
  const receipt = await $app.transaction(async tx => {
    await tx.save(tx.newRecord('demo_audit', { message: 'Queued email' }));
    const receipt = await tx.outbox.enqueue('mail.send', {
      to: [{ address: input.address }], subject: 'HertaBase committed job', text: 'This job committed with its audit record',
    }, { idempotencyKey: input.key });
    if (input.fail) throw new BadRequestError('Requested rollback of both audit and outbox');
    return receipt;
  });
  return e.json(202, receipt);
}, $apis.requireAdmin(), $apis.bodyLimit(4096));
