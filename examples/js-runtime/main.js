/// <reference path="../../packages/types/index.d.ts" />
// Initialization only registers callbacks; I/O belongs inside an event.
onBootstrap(async e => {
  const definitions = [
    { name: 'demo_audit', type: 'base', schema_mode: 'strict', fields: [{ name: 'message', type: 'text', required: true }] },
    { name: 'demo_posts', type: 'base', schema_mode: 'strict', fields: [
      { name: 'title', type: 'text', required: true }, { name: 'slug', type: 'text', required: true },
    ], rules: { list: true, view: true } },
    { name: 'demo_members', type: 'auth', schema_mode: 'strict', fields: [{ name: 'name', type: 'text', required: true }] },
  ];
  for (const definition of definitions) {
    try { await $app.collections.findByName(definition.name); }
    catch (error) {
      if (error.code !== 'HB_COLLECTION_NOT_FOUND') throw error;
      await $app.collections.create(definition);
    }
  }
  await e.next();
});

onRecordCreate(async e => {
  e.record.set('slug', String(e.record.get('title')).toLowerCase().replace(/\s+/g, '-'));
  await $app.save($app.newRecord('demo_audit', { message: 'post candidate' }));
  e.afterCommit(snapshot => $app.logger.info('post committed', { id: snapshot.record.id }));
  await e.next();
}, 'demo_posts');

onAuthRegister(async e => {
  e.profile.name ??= 'New member';
  await $app.save($app.newRecord('demo_audit', { message: 'member candidate' }));
  await e.next();
}, 'demo_members');

routerAdd('POST', '/api/custom/demo-transaction', async e => {
  const body = await e.request.json();
  const saved = await $app.transaction(async tx => {
    const record = await tx.save(tx.newRecord('demo_posts', { title: body.title }));
    if (body.fail) throw new ConflictError('Demonstration rollback');
    return record;
  });
  return e.json(201, saved);
}, $apis.requireAdmin(), $apis.bodyLimit(4096));

routerAdd('GET', '/api/custom/demo-health', e => e.noContent());
onServe(async e => { $app.logger.info('demo extensions ready'); await e.next(); });
onShutdown(async e => { $app.logger.info('demo extensions stopping'); await e.next(); });
