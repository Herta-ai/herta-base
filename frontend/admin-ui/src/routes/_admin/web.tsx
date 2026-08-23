import { useRef, useState } from 'react'
import { createFileRoute } from '@tanstack/react-router'
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import Button from '@jetbrains/ring-ui-built/components/button/button'
import Loader from '@jetbrains/ring-ui-built/components/loader/loader'
import { hbApi, type WebProjectModel } from '../../lib/api'

export const Route = createFileRoute('/_admin/web')({ component: WebProjectsPage })

function WebProjectsPage() {
  const queryClient = useQueryClient()
  const archiveRef = useRef<HTMLInputElement>(null)
  const [alias, setAlias] = useState('')
  const [spaFallback, setSpaFallback] = useState(true)
  const [cacheControl, setCacheControl] = useState('public, max-age=0, must-revalidate')
  const [notFound, setNotFound] = useState('')
  const [message, setMessage] = useState('')
  const [versions, setVersions] = useState<Record<string, string[]>>({})
  const [editing, setEditing] = useState<string | null>(null)
  const [editAlias, setEditAlias] = useState('')
  const [editSpaFallback, setEditSpaFallback] = useState(true)
  const [editCacheControl, setEditCacheControl] = useState('')
  const [editNotFound, setEditNotFound] = useState('')

  const projects = useQuery({
    queryKey: ['web-projects'],
    queryFn: async () => (await hbApi.webProjects.list()).data.data || [],
  })
  const deploy = useMutation({
    mutationFn: async () => {
      const file = archiveRef.current?.files?.[0]
      if (!file) throw new Error('Select a ZIP, tar.gz, or 7z archive first.')
      const form = new FormData()
      form.append('archive', file)
      if (alias.trim()) form.append('alias', alias.trim())
      form.append('spaFallback', String(spaFallback))
      if (cacheControl.trim()) form.append('cacheControl', cacheControl.trim())
      if (notFound.trim()) form.append('notFound', notFound.trim())
      return hbApi.webProjects.deploy(form)
    },
    onSuccess: () => {
      setMessage('Deployment completed.')
      queryClient.invalidateQueries({ queryKey: ['web-projects'] })
      if (archiveRef.current) archiveRef.current.value = ''
    },
    onError: (error: unknown) => setMessage(error instanceof Error ? error.message : 'Deployment failed.'),
  })
  const update = useMutation({
    mutationFn: () => {
      if (!editing) throw new Error('No project selected.')
      return hbApi.webProjects.patch(editing, {
        alias: editAlias.trim() || null,
        spaFallback: editSpaFallback,
        cacheControl: editCacheControl.trim(),
        notFound: editNotFound.trim() || null,
      })
    },
    onSuccess: () => {
      setEditing(null)
      setMessage('Project settings updated.')
      queryClient.invalidateQueries({ queryKey: ['web-projects'] })
    },
    onError: (error: unknown) => setMessage(error instanceof Error ? error.message : 'Update failed.'),
  })

  const loadVersions = async (project: WebProjectModel) => {
    const response = await hbApi.webProjects.versions(project.name)
    setVersions((current) => ({ ...current, [project.name]: response.data.data || [] }))
  }

  const remove = async (project: WebProjectModel) => {
    if (!window.confirm(`Delete ${project.name}? Version backups are retained.`)) return
    await hbApi.webProjects.delete(project.name)
    queryClient.invalidateQueries({ queryKey: ['web-projects'] })
  }

  const rollback = async (project: WebProjectModel, version: string) => {
    await hbApi.webProjects.rollback(project.name, version)
    setMessage(`Rolled ${project.name} back to ${version}.`)
    queryClient.invalidateQueries({ queryKey: ['web-projects'] })
  }

  const startEdit = (project: WebProjectModel) => {
    setEditing(project.name)
    setEditAlias(project.alias || '')
    setEditSpaFallback(project.spaFallback)
    setEditCacheControl(project.cacheControl)
    setEditNotFound(project.notFound || '')
  }

  return (
    <div style={{ flex: 1, padding: 20, overflow: 'auto' }}>
      <div className="jb-editor-tabs" style={{ marginBottom: 12 }}>
        <div className="jb-editor-tab active"><span className="i-ph:globe-bold text-sky-400" />Web Hosting</div>
      </div>
      <section style={{ border: '1px solid var(--jb-border)', background: 'var(--jb-panel-bg)', padding: 16, borderRadius: 6, marginBottom: 16 }}>
        <h2 style={{ margin: '0 0 12px', fontSize: 15 }}>Deploy project archive</h2>
        <div style={{ display: 'grid', gridTemplateColumns: 'minmax(240px, 1fr) minmax(180px, 1fr)', gap: 10 }}>
          <label>Archive<input ref={archiveRef} type="file" accept=".zip,.7z,.tar.gz,application/zip,application/gzip" style={{ display: 'block', marginTop: 5 }} /></label>
          <label>Alias<input value={alias} onChange={(event) => setAlias(event.target.value)} placeholder="/web/docs" /></label>
          <label>Cache-Control<input value={cacheControl} onChange={(event) => setCacheControl(event.target.value)} /></label>
          <label>Custom 404 path<input value={notFound} onChange={(event) => setNotFound(event.target.value)} placeholder="404.html" /></label>
        </div>
        <label style={{ display: 'flex', alignItems: 'center', gap: 7, margin: '12px 0' }}><input type="checkbox" checked={spaFallback} onChange={(event) => setSpaFallback(event.target.checked)} />SPA history fallback</label>
        <Button primary onClick={() => deploy.mutate()} disabled={deploy.isPending}>{deploy.isPending ? 'Deploying...' : 'Deploy'}</Button>
        {message && <span style={{ marginLeft: 10, color: 'var(--jb-accent-green)', fontSize: 12 }}>{message}</span>}
      </section>
      <section style={{ border: '1px solid var(--jb-border)', background: 'var(--jb-panel-bg)', borderRadius: 6 }}>
        <div style={{ padding: '10px 14px', borderBottom: '1px solid var(--jb-border)', fontWeight: 600 }}>Deployed projects</div>
        {projects.isLoading ? <Loader /> : (projects.data || []).length === 0 ? <div style={{ padding: 16, color: 'var(--jb-text-muted)' }}>No deployed projects.</div> : (projects.data || []).map((project) => (
          <div key={project.name} style={{ padding: 12, borderBottom: '1px solid var(--jb-border)', display: 'grid', gridTemplateColumns: '1fr auto', gap: 8 }}>
            <div><strong>{project.name}</strong><div style={{ fontSize: 12, color: 'var(--jb-text-muted)', marginTop: 3 }}>{project.alias || `/web/${project.name}`} · {project.spaFallback ? 'SPA fallback on' : 'SPA fallback off'}</div></div>
            <div style={{ display: 'flex', gap: 6, alignItems: 'start' }}><a href={project.alias || `/web/${project.name}/`} target="_blank" rel="noreferrer">Open</a><Button onClick={() => startEdit(project)}>Edit</Button><Button onClick={() => loadVersions(project)}>Versions</Button><Button onClick={() => remove(project)}>Delete</Button></div>
            {editing === project.name && <div style={{ gridColumn: '1 / -1', display: 'grid', gridTemplateColumns: 'repeat(4, minmax(120px, 1fr))', gap: 8, paddingTop: 8 }}>
              <input value={editAlias} onChange={(event) => setEditAlias(event.target.value)} placeholder="/web/alias" />
              <input value={editCacheControl} onChange={(event) => setEditCacheControl(event.target.value)} />
              <input value={editNotFound} onChange={(event) => setEditNotFound(event.target.value)} placeholder="404.html" />
              <label style={{ display: 'flex', alignItems: 'center', gap: 5 }}><input type="checkbox" checked={editSpaFallback} onChange={(event) => setEditSpaFallback(event.target.checked)} />SPA fallback</label>
              <div style={{ gridColumn: '1 / -1', display: 'flex', gap: 6 }}><Button primary onClick={() => update.mutate()} disabled={update.isPending}>Save</Button><Button onClick={() => setEditing(null)}>Cancel</Button></div>
            </div>}
            {(versions[project.name] || []).length > 0 && <div style={{ gridColumn: '1 / -1', display: 'flex', gap: 6, flexWrap: 'wrap' }}>{versions[project.name].map((version) => <Button key={version} onClick={() => rollback(project, version)}>{version}</Button>)}</div>}
          </div>
        ))}
      </section>
    </div>
  )
}
