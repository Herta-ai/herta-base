import {useRef, useState, useMemo, type DragEvent, type ChangeEvent} from 'react'
import {createFileRoute} from '@tanstack/react-router'
import {useMutation, useQuery, useQueryClient} from '@tanstack/react-query'
import Button from '@jetbrains/ring-ui-built/components/button/button'
import Loader from '@jetbrains/ring-ui-built/components/loader/loader'
import Tooltip from '@jetbrains/ring-ui-built/components/tooltip/tooltip'
import dayjs from 'dayjs'
import {hbApi, type WebProjectModel} from '../../lib/api'
import {useI18n} from '../../lib/i18n'

export const Route = createFileRoute('/_admin/web')({component: WebProjectsPage})

const DEFAULT_CACHE_CONTROL = 'public, max-age=0, must-revalidate'

function formatFileSize(bytes: number): string {
    if (bytes === 0) return '0 B'
    const k = 1024
    const sizes = ['B', 'KB', 'MB', 'GB']
    const i = Math.floor(Math.log(bytes) / Math.log(k))
    return `${parseFloat((bytes / Math.pow(k, i)).toFixed(2))} ${sizes[i]}`
}

function formatVersionTimestamp(version: string): string {
    const parts = version.split('-')
    if (parts.length === 6) {
        const [year, month, day, hour, min, sec] = parts
        return `${year}-${month}-${day} ${hour}:${min}:${sec}`
    }
    return version
}

function WebProjectsPage() {
    const {t} = useI18n()
    const queryClient = useQueryClient()

    // Form State
    const archiveRef = useRef<HTMLInputElement>(null)
    const [selectedFile, setSelectedFile] = useState<File | null>(null)
    const [isDragOver, setIsDragOver] = useState(false)
    const [alias, setAlias] = useState('')
    const [spaFallback, setSpaFallback] = useState(true)
    const [cacheControl, setCacheControl] = useState(DEFAULT_CACHE_CONTROL)
    const [notFound, setNotFound] = useState('')

    // Notifications
    const [successMessage, setSuccessMessage] = useState<string | null>(null)
    const [errorMessage, setErrorMessage] = useState<string | null>(null)
    const [copiedUrlProject, setCopiedUrlProject] = useState<string | null>(null)

    // Modals & Panels State
    const [searchQuery, setSearchQuery] = useState('')
    const [showGuide, setShowGuide] = useState(false)
    const [editingProject, setEditingProject] = useState<WebProjectModel | null>(null)
    const [editAlias, setEditAlias] = useState('')
    const [editSpaFallback, setEditSpaFallback] = useState(true)
    const [editCacheControl, setEditCacheControl] = useState('')
    const [editNotFound, setEditNotFound] = useState('')

    // Version History Modal State
    const [versionsModalProject, setVersionsModalProject] = useState<WebProjectModel | null>(null)
    const [versionsMap, setVersionsMap] = useState<Record<string, string[]>>({})
    const [versionsLoading, setVersionsLoading] = useState(false)

    // Query Deployed Projects
    const {data: projectsData, isLoading, isFetching, refetch} = useQuery({
        queryKey: ['web-projects'],
        queryFn: async () => (await hbApi.webProjects.list()).data.data || [],
    })

    const projects = useMemo(() => projectsData || [], [projectsData])

    // Filtered Projects
    const filteredProjects = useMemo(() => {
        if (!searchQuery.trim()) return projects
        const q = searchQuery.trim().toLowerCase()
        return projects.filter(
            (p) =>
                p.name.toLowerCase().includes(q) ||
                (p.alias && p.alias.toLowerCase().includes(q)),
        )
    }, [projects, searchQuery])

    // Deploy Mutation
    const deploy = useMutation({
        mutationFn: async () => {
            const file = selectedFile || archiveRef.current?.files?.[0]
            if (!file) throw new Error(t('web.archive_required_err'))
            const form = new FormData()
            form.append('archive', file)
            if (alias.trim()) form.append('alias', alias.trim())
            form.append('spaFallback', String(spaFallback))
            if (cacheControl.trim()) form.append('cacheControl', cacheControl.trim())
            if (notFound.trim()) form.append('notFound', notFound.trim())
            return hbApi.webProjects.deploy(form)
        },
        onSuccess: () => {
            setSuccessMessage(t('web.deploy_success'))
            setErrorMessage(null)
            setSelectedFile(null)
            if (archiveRef.current) archiveRef.current.value = ''
            setAlias('')
            setNotFound('')
            setCacheControl(DEFAULT_CACHE_CONTROL)
            setSpaFallback(true)
            queryClient.invalidateQueries({queryKey: ['web-projects']})
            setTimeout(() => setSuccessMessage(null), 4000)
        },
        onError: (error: unknown) => {
            const msg = error instanceof Error ? error.message : String(error)
            setErrorMessage(t('web.deploy_failed', {error: msg}))
            setSuccessMessage(null)
        },
    })

    // Update Settings Mutation
    const update = useMutation({
        mutationFn: () => {
            if (!editingProject) throw new Error('No project selected')
            return hbApi.webProjects.patch(editingProject.name, {
                alias: editAlias.trim() || null,
                spaFallback: editSpaFallback,
                cacheControl: editCacheControl.trim() || DEFAULT_CACHE_CONTROL,
                notFound: editNotFound.trim() || null,
            })
        },
        onSuccess: () => {
            setEditingProject(null)
            setSuccessMessage(t('web.edit_success'))
            queryClient.invalidateQueries({queryKey: ['web-projects']})
            setTimeout(() => setSuccessMessage(null), 3000)
        },
        onError: (error: unknown) => {
            setErrorMessage(error instanceof Error ? error.message : 'Update failed')
        },
    })

    // Rollback Mutation
    const rollbackMutation = useMutation({
        mutationFn: async ({project, version}: { project: WebProjectModel; version: string }) => {
            return hbApi.webProjects.rollback(project.name, version)
        },
        onSuccess: (_, variables) => {
            setSuccessMessage(t('web.rollback_success', {name: variables.project.name, version: variables.version}))
            setVersionsModalProject(null)
            queryClient.invalidateQueries({queryKey: ['web-projects']})
            setTimeout(() => setSuccessMessage(null), 4000)
        },
        onError: (error: unknown) => {
            setErrorMessage(error instanceof Error ? error.message : 'Rollback failed')
        },
    })

    const openVersionsModal = async (project: WebProjectModel) => {
        setVersionsModalProject(project)
        setVersionsLoading(true)
        try {
            const response = await hbApi.webProjects.versions(project.name)
            setVersionsMap((cur) => ({...cur, [project.name]: response.data.data || []}))
        } catch {
            setVersionsMap((cur) => ({...cur, [project.name]: []}))
        } finally {
            setVersionsLoading(false)
        }
    }

    const remove = async (project: WebProjectModel) => {
        if (!window.confirm(t('web.delete_confirm', {name: project.name}))) return
        try {
            await hbApi.webProjects.delete(project.name)
            setSuccessMessage(t('web.delete_success', {name: project.name}))
            queryClient.invalidateQueries({queryKey: ['web-projects']})
            setTimeout(() => setSuccessMessage(null), 3000)
        } catch (err: unknown) {
            setErrorMessage(err instanceof Error ? err.message : 'Delete failed')
        }
    }

    const startEdit = (project: WebProjectModel) => {
        setEditingProject(project)
        setEditAlias(project.alias || '')
        setEditSpaFallback(project.spaFallback)
        setEditCacheControl(project.cacheControl || DEFAULT_CACHE_CONTROL)
        setEditNotFound(project.notFound || '')
        setErrorMessage(null)
    }

    const handleFileChange = (e: ChangeEvent<HTMLInputElement>) => {
        const file = e.target.files?.[0]
        if (file) {
            setSelectedFile(file)
            setErrorMessage(null)
        }
    }

    const handleDragOver = (e: DragEvent<HTMLDivElement>) => {
        e.preventDefault()
        e.stopPropagation()
        setIsDragOver(true)
    }

    const handleDragLeave = (e: DragEvent<HTMLDivElement>) => {
        e.preventDefault()
        e.stopPropagation()
        setIsDragOver(false)
    }

    const handleDrop = (e: DragEvent<HTMLDivElement>) => {
        e.preventDefault()
        e.stopPropagation()
        setIsDragOver(false)
        const file = e.dataTransfer.files?.[0]
        if (file) {
            setSelectedFile(file)
            setErrorMessage(null)
        }
    }

    const clearFile = () => {
        setSelectedFile(null)
        if (archiveRef.current) {
            archiveRef.current.value = ''
        }
    }

    const copyProjectUrl = (project: WebProjectModel) => {
        const route = project.alias || `/web/${project.name}/`
        const fullUrl = `${window.location.origin}${route.startsWith('/') ? route : `/${route}`}`
        navigator.clipboard.writeText(fullUrl)
        setCopiedUrlProject(project.name)
        setTimeout(() => setCopiedUrlProject(null), 2000)
    }

    const applyCachePreset = (val: string, isEdit = false) => {
        if (isEdit) {
            setEditCacheControl(val)
        } else {
            setCacheControl(val)
        }
    }

    return (
        <div style={{flex: 1, display: 'flex', flexDirection: 'column', height: '100%', overflow: 'hidden'}}>
            {/* 1. Tab Bar */}
            <div className="jb-editor-tabs" style={{justifyContent: 'space-between', paddingRight: 10}}>
                <div style={{display: 'flex', alignItems: 'center'}}>
                    <div className="jb-editor-tab active">
                        <span className="i-ph:globe-bold text-sky-400 text-13px"/>
                        <span>{t('web.title')}</span>
                        <span
                            style={{
                                fontSize: 10,
                                color: 'var(--jb-text-muted)',
                                background: 'var(--jb-border)',
                                padding: '1px 6px',
                                borderRadius: 10,
                                fontWeight: 600,
                            }}
                        >
              {projects.length}
            </span>
                    </div>
                </div>

                {/* Toolbar */}
                <div style={{display: 'flex', alignItems: 'center', gap: 6}}>
                    <Button
                        onClick={() => setShowGuide(!showGuide)}
                        style={{
                            height: 24,
                            fontSize: 11,
                            display: 'flex',
                            alignItems: 'center',
                            gap: 4,
                            background: showGuide ? 'var(--jb-active-item)' : 'transparent',
                            color: showGuide ? 'var(--jb-accent-blue)' : 'var(--jb-text)',
                        }}
                    >
                        <span className="i-ph:book-bookmark-bold text-12px"/>
                        <span>{t('web.guide.title')}</span>
                    </Button>

                    <Button
                        onClick={() => refetch()}
                        style={{height: 24, fontSize: 11, display: 'flex', alignItems: 'center', gap: 4}}
                    >
                        <span className={`i-ph:arrow-clockwise-bold text-11px ${isFetching ? 'animate-spin' : ''}`}/>
                        <span>{t('app.refresh')}</span>
                    </Button>
                </div>
            </div>

            {/* 2. Breadcrumbs */}
            <div className="jb-breadcrumbs">
                <span>HertaBase</span>
                <span className="jb-breadcrumb-sep">›</span>
                <span>Static Hosting</span>
                <span className="jb-breadcrumb-sep">›</span>
                <span style={{
                    color: 'var(--jb-accent-blue)',
                    fontWeight: 500,
                    display: 'inline-flex',
                    alignItems: 'center',
                    gap: 4
                }}>
          <span className="i-ph:globe-bold text-12px text-sky-400"/>
          <span>{t('web.projects_list')}</span>
        </span>
            </div>

            {/* 3. Main Content Area */}
            <div
                style={{
                    flex: 1,
                    overflowY: 'auto',
                    overflowX: 'hidden',
                    padding: 20,
                    display: 'flex',
                    flexDirection: 'column',
                    gap: 20,
                    boxSizing: 'border-box',
                }}
            >
                {/* Welcome / Overview Banner */}
                <div className="jb-welcome-banner">
                    <div className="jb-banner-info">
                        <h2>{t('web.title')}</h2>
                        <p>{t('web.subtitle')}</p>
                    </div>
                    <div style={{display: 'flex', gap: 8, alignItems: 'center', flexWrap: 'wrap'}}>
            <span className="jb-branch-badge" style={{display: 'flex', alignItems: 'center', gap: 4}}>
              <span className="i-ph:rocket-launch-bold text-12px text-emerald-400"/>
              <span>Atomic Deploy</span>
            </span>
                        <span className="jb-branch-badge" style={{display: 'flex', alignItems: 'center', gap: 4}}>
              <span className="i-ph:clock-counter-clockwise-bold text-12px text-purple-400"/>
              <span>Version Rollback</span>
            </span>
                        <span className="jb-branch-badge" style={{display: 'flex', alignItems: 'center', gap: 4}}>
              <span className="i-ph:sparkle-bold text-12px text-sky-400"/>
              <span>SPA Fallback</span>
            </span>
                    </div>
                </div>

                {/* Global Notifications */}
                {successMessage && (
                    <div
                        style={{
                            background: 'rgba(73, 156, 84, 0.12)',
                            border: '1px solid #499c54',
                            color: '#499c54',
                            padding: '10px 14px',
                            borderRadius: 6,
                            fontSize: 12,
                            display: 'flex',
                            alignItems: 'center',
                            justifyContent: 'space-between',
                            gap: 8,
                        }}
                    >
                        <div style={{display: 'flex', alignItems: 'center', gap: 8}}>
                            <span className="i-ph:check-circle-bold text-16px shrink-0"/>
                            <span style={{fontWeight: 500}}>{successMessage}</span>
                        </div>
                        <button
                            onClick={() => setSuccessMessage(null)}
                            style={{
                                background: 'transparent',
                                border: 'none',
                                color: '#499c54',
                                cursor: 'pointer',
                                display: 'flex',
                                padding: 0
                            }}
                        >
                            <span className="i-ph:x-bold text-13px"/>
                        </button>
                    </div>
                )}

                {errorMessage && (
                    <div
                        style={{
                            background: 'rgba(229, 57, 53, 0.12)',
                            border: '1px solid #e53935',
                            color: '#e53935',
                            padding: '10px 14px',
                            borderRadius: 6,
                            fontSize: 12,
                            display: 'flex',
                            alignItems: 'center',
                            justifyContent: 'space-between',
                            gap: 8,
                        }}
                    >
                        <div style={{display: 'flex', alignItems: 'center', gap: 8}}>
                            <span className="i-ph:warning-circle-bold text-16px shrink-0"/>
                            <span style={{fontWeight: 500}}>{errorMessage}</span>
                        </div>
                        <button
                            onClick={() => setErrorMessage(null)}
                            style={{
                                background: 'transparent',
                                border: 'none',
                                color: '#e53935',
                                cursor: 'pointer',
                                display: 'flex',
                                padding: 0
                            }}
                        >
                            <span className="i-ph:x-bold text-13px"/>
                        </button>
                    </div>
                )}

                {/* Collapsible Packaging Guide Box */}
                {showGuide && (
                    <div
                        style={{
                            background: 'var(--jb-panel-bg)',
                            border: '1px solid var(--jb-accent-blue)',
                            borderRadius: 8,
                            padding: '14px 18px',
                            fontSize: 12,
                            boxShadow: 'var(--jb-shadow)',
                            display: 'flex',
                            flexDirection: 'column',
                            gap: 10,
                        }}
                    >
                        <div style={{display: 'flex', justifyContent: 'space-between', alignItems: 'center'}}>
                            <div style={{
                                fontWeight: 700,
                                color: 'var(--jb-text-heading)',
                                display: 'flex',
                                alignItems: 'center',
                                gap: 6
                            }}>
                                <span className="i-ph:info-bold text-sky-400 text-15px"/>
                                <span>{t('web.guide.title')}</span>
                            </div>
                            <button
                                onClick={() => setShowGuide(false)}
                                style={{
                                    background: 'transparent',
                                    border: 'none',
                                    color: 'var(--jb-text-muted)',
                                    cursor: 'pointer',
                                    display: 'flex',
                                    padding: 0
                                }}
                            >
                                <span className="i-ph:x-bold text-13px"/>
                            </button>
                        </div>

                        <div style={{
                            display: 'grid',
                            gridTemplateColumns: 'repeat(auto-fit, minmax(240px, 1fr))',
                            gap: 12
                        }}>
                            <div style={{
                                background: 'var(--jb-header-bg)',
                                padding: '10px 12px',
                                borderRadius: 6,
                                border: '1px solid var(--jb-border)'
                            }}>
                                <div style={{
                                    fontWeight: 600,
                                    color: 'var(--jb-accent-blue)',
                                    marginBottom: 4,
                                    display: 'flex',
                                    alignItems: 'center',
                                    gap: 4
                                }}>
                                    <span className="i-ph:check-bold text-12px"/>
                                    <span>1. 规范的目录结构</span>
                                </div>
                                <div style={{color: 'var(--jb-text-muted)', fontSize: 11, lineHeight: 1.5}}>
                                    {t('web.guide.rule1')}
                                    <pre style={{
                                        margin: '6px 0 0',
                                        background: 'var(--jb-editor-bg)',
                                        padding: '6px 8px',
                                        borderRadius: 4,
                                        fontFamily: 'monospace'
                                    }}>
{`site.zip
└── my-app/
    ├── index.html
    └── assets/`}
                  </pre>
                                </div>
                            </div>

                            <div style={{
                                background: 'var(--jb-header-bg)',
                                padding: '10px 12px',
                                borderRadius: 6,
                                border: '1px solid var(--jb-border)'
                            }}>
                                <div style={{
                                    fontWeight: 600,
                                    color: 'var(--jb-accent-purple)',
                                    marginBottom: 4,
                                    display: 'flex',
                                    alignItems: 'center',
                                    gap: 4
                                }}>
                                    <span className="i-ph:check-bold text-12px"/>
                                    <span>2. 格式与大小</span>
                                </div>
                                <div style={{color: 'var(--jb-text-muted)', fontSize: 11, lineHeight: 1.5}}>
                                    {t('web.guide.rule2')}
                                </div>
                            </div>

                            <div style={{
                                background: 'var(--jb-header-bg)',
                                padding: '10px 12px',
                                borderRadius: 6,
                                border: '1px solid var(--jb-border)'
                            }}>
                                <div style={{
                                    fontWeight: 600,
                                    color: 'var(--jb-accent-green)',
                                    marginBottom: 4,
                                    display: 'flex',
                                    alignItems: 'center',
                                    gap: 4
                                }}>
                                    <span className="i-ph:check-bold text-12px"/>
                                    <span>3. SPA 路由支持</span>
                                </div>
                                <div style={{color: 'var(--jb-text-muted)', fontSize: 11, lineHeight: 1.5}}>
                                    {t('web.guide.rule3')}
                                </div>
                            </div>
                        </div>
                    </div>
                )}

                {/* 4. Deployment Section Card */}
                <div
                    style={{
                        border: '1px solid var(--jb-border)',
                        borderRadius: 8,
                        overflow: 'hidden',
                        backgroundColor: 'var(--jb-panel-bg)',
                        boxShadow: 'var(--jb-shadow)',
                    }}
                >
                    {/* Header */}
                    <div
                        style={{
                            padding: '10px 16px',
                            backgroundColor: 'var(--jb-header-bg)',
                            borderBottom: '1px solid var(--jb-border)',
                            display: 'flex',
                            alignItems: 'center',
                            justifyContent: 'space-between',
                        }}
                    >
                        <div style={{
                            fontSize: 13,
                            fontWeight: 600,
                            color: 'var(--jb-text-heading)',
                            display: 'flex',
                            alignItems: 'center',
                            gap: 8
                        }}>
                            <span className="i-ph:cloud-arrow-up-bold text-sky-400 text-15px"/>
                            <span>{t('web.deploy_title')}</span>
                        </div>
                    </div>

                    {/* Content */}
                    <div style={{padding: '16px 20px', display: 'flex', flexDirection: 'column', gap: 14}}>
                        <p style={{margin: 0, fontSize: 12, color: 'var(--jb-text-muted)', lineHeight: 1.5}}>
                            {t('web.deploy_desc')}
                        </p>

                        {/* Compact Drag & Drop File Upload Box */}
                        <div
                            onDragOver={handleDragOver}
                            onDragLeave={handleDragLeave}
                            onDrop={handleDrop}
                            onClick={() => archiveRef.current?.click()}
                            style={{
                                border: isDragOver
                                    ? '2px dashed var(--jb-accent-blue)'
                                    : selectedFile
                                        ? '1.5px solid var(--jb-accent-green)'
                                        : '1.5px dashed var(--jb-border)',
                                background: isDragOver
                                    ? 'rgba(53, 116, 240, 0.08)'
                                    : selectedFile
                                        ? 'rgba(73, 156, 84, 0.06)'
                                        : 'var(--jb-header-bg)',
                                borderRadius: 6,
                                padding: selectedFile ? '10px 16px' : '16px 20px',
                                cursor: 'pointer',
                                transition: 'all 0.15s ease-in-out',
                                display: 'flex',
                                alignItems: 'center',
                                justifyContent: selectedFile ? 'space-between' : 'center',
                                gap: 12,
                                boxSizing: 'border-box',
                            }}
                        >
                            <input
                                ref={archiveRef}
                                type="file"
                                accept=".zip,.7z,.tar.gz,application/zip,application/x-7z-compressed,application/gzip,application/x-tar"
                                onChange={handleFileChange}
                                style={{display: 'none'}}
                            />

                            {selectedFile ? (
                                <>
                                    <div style={{display: 'flex', alignItems: 'center', gap: 10, minWidth: 0}}>
                                        <span className="i-ph:file-zip-bold text-26px text-emerald-500 shrink-0"/>
                                        <div style={{minWidth: 0}}>
                                            <div style={{
                                                fontSize: 12,
                                                fontWeight: 700,
                                                color: 'var(--jb-text-heading)',
                                                overflow: 'hidden',
                                                textOverflow: 'ellipsis',
                                                whiteSpace: 'nowrap'
                                            }}>
                                                {selectedFile.name}
                                            </div>
                                            <div style={{fontSize: 11, color: 'var(--jb-text-muted)', marginTop: 2}}>
                                                {formatFileSize(selectedFile.size)} · {selectedFile.type || 'archive'}
                                            </div>
                                        </div>
                                    </div>
                                    <button
                                        type="button"
                                        onClick={(e) => {
                                            e.stopPropagation()
                                            clearFile()
                                        }}
                                        style={{
                                            background: 'rgba(239, 68, 68, 0.1)',
                                            border: '1px solid #ef4444',
                                            color: '#ef4444',
                                            borderRadius: 4,
                                            padding: '3px 8px',
                                            fontSize: 11,
                                            cursor: 'pointer',
                                            display: 'flex',
                                            alignItems: 'center',
                                            gap: 4,
                                            flexShrink: 0,
                                        }}
                                    >
                                        <span className="i-ph:trash-bold text-11px"/>
                                        <span>{t('web.archive_clear')}</span>
                                    </button>
                                </>
                            ) : (
                                <div style={{display: 'flex', alignItems: 'center', gap: 10}}>
                                    <span className="i-ph:file-archive-bold text-26px text-sky-400 shrink-0"/>
                                    <div>
                    <span style={{fontSize: 12, fontWeight: 600, color: 'var(--jb-text-heading)'}}>
                      {t('web.archive_drag_drop')}
                    </span>
                                        <span style={{fontSize: 11, color: 'var(--jb-text-muted)', marginLeft: 8}}>
                      ({t('web.archive_support_formats')})
                    </span>
                                    </div>
                                </div>
                            )}
                        </div>

                        {/* Options Form Grid */}
                        <div style={{
                            display: 'grid',
                            gridTemplateColumns: 'repeat(auto-fit, minmax(240px, 1fr))',
                            gap: 12
                        }}>
                            {/* Routing Alias */}
                            <div>
                                <label style={{
                                    fontSize: 12,
                                    fontWeight: 600,
                                    color: 'var(--jb-text-muted)',
                                    display: 'block',
                                    marginBottom: 4
                                }}>
                                    {t('web.alias')}
                                </label>
                                <div style={{position: 'relative', display: 'flex', alignItems: 'center'}}>
                                    <span className="i-ph:link-bold text-12px text-zinc-400 absolute left-2.5"/>
                                    <input
                                        type="text"
                                        value={alias}
                                        onChange={(e) => setAlias(e.target.value)}
                                        placeholder={t('web.alias_placeholder')}
                                        style={{
                                            width: '100%',
                                            padding: '6px 10px 6px 28px',
                                            borderRadius: 4,
                                            border: '1px solid var(--jb-border)',
                                            backgroundColor: 'var(--jb-editor-bg)',
                                            color: 'var(--jb-text)',
                                            fontSize: 12,
                                            boxSizing: 'border-box',
                                        }}
                                    />
                                </div>
                                <div style={{fontSize: 11, color: 'var(--jb-text-muted)', marginTop: 4}}>
                                    {t('web.alias_help')}
                                </div>
                            </div>

                            {/* Custom 404 Path */}
                            <div>
                                <label style={{
                                    fontSize: 12,
                                    fontWeight: 600,
                                    color: 'var(--jb-text-muted)',
                                    display: 'block',
                                    marginBottom: 4
                                }}>
                                    {t('web.not_found')}
                                </label>
                                <div style={{position: 'relative', display: 'flex', alignItems: 'center'}}>
                                    <span className="i-ph:file-dashed-bold text-12px text-zinc-400 absolute left-2.5"/>
                                    <input
                                        type="text"
                                        value={notFound}
                                        onChange={(e) => setNotFound(e.target.value)}
                                        placeholder={t('web.not_found_placeholder')}
                                        style={{
                                            width: '100%',
                                            padding: '6px 10px 6px 28px',
                                            borderRadius: 4,
                                            border: '1px solid var(--jb-border)',
                                            backgroundColor: 'var(--jb-editor-bg)',
                                            color: 'var(--jb-text)',
                                            fontSize: 12,
                                            boxSizing: 'border-box',
                                        }}
                                    />
                                </div>
                                <div style={{fontSize: 11, color: 'var(--jb-text-muted)', marginTop: 4}}>
                                    {t('web.not_found_help')}
                                </div>
                            </div>

                            {/* Cache-Control & Presets */}
                            <div style={{gridColumn: '1 / -1'}}>
                                <div style={{
                                    display: 'flex',
                                    justifyContent: 'space-between',
                                    alignItems: 'center',
                                    marginBottom: 4,
                                    flexWrap: 'wrap',
                                    gap: 4
                                }}>
                                    <label style={{fontSize: 12, fontWeight: 600, color: 'var(--jb-text-muted)'}}>
                                        {t('web.cache_control')}
                                    </label>
                                    <div style={{display: 'flex', gap: 4, flexWrap: 'wrap', alignItems: 'center'}}>
                    <span style={{fontSize: 10, color: 'var(--jb-text-muted)', marginRight: 2}}>
                      快捷预设:
                    </span>
                                        <button
                                            type="button"
                                            onClick={() => applyCachePreset('no-cache, no-store, must-revalidate')}
                                            style={{
                                                background: 'var(--jb-header-bg)',
                                                border: '1px solid var(--jb-border)',
                                                color: 'var(--jb-text)',
                                                fontSize: 10,
                                                padding: '1px 6px',
                                                borderRadius: 3,
                                                cursor: 'pointer',
                                            }}
                                        >
                                            {t('web.cache_preset.no_cache')}
                                        </button>
                                        <button
                                            type="button"
                                            onClick={() => applyCachePreset('public, max-age=3600, must-revalidate')}
                                            style={{
                                                background: 'var(--jb-header-bg)',
                                                border: '1px solid var(--jb-border)',
                                                color: 'var(--jb-text)',
                                                fontSize: 10,
                                                padding: '1px 6px',
                                                borderRadius: 3,
                                                cursor: 'pointer',
                                            }}
                                        >
                                            {t('web.cache_preset.one_hour')}
                                        </button>
                                        <button
                                            type="button"
                                            onClick={() => applyCachePreset('public, max-age=86400, must-revalidate')}
                                            style={{
                                                background: 'var(--jb-header-bg)',
                                                border: '1px solid var(--jb-border)',
                                                color: 'var(--jb-text)',
                                                fontSize: 10,
                                                padding: '1px 6px',
                                                borderRadius: 3,
                                                cursor: 'pointer',
                                            }}
                                        >
                                            {t('web.cache_preset.one_day')}
                                        </button>
                                        <button
                                            type="button"
                                            onClick={() => applyCachePreset('public, max-age=31536000, immutable')}
                                            style={{
                                                background: 'var(--jb-header-bg)',
                                                border: '1px solid var(--jb-border)',
                                                color: 'var(--jb-text)',
                                                fontSize: 10,
                                                padding: '1px 6px',
                                                borderRadius: 3,
                                                cursor: 'pointer',
                                            }}
                                        >
                                            {t('web.cache_preset.immutable')}
                                        </button>
                                    </div>
                                </div>
                                <div style={{position: 'relative', display: 'flex', alignItems: 'center'}}>
                                    <span className="i-ph:timer-bold text-12px text-zinc-400 absolute left-2.5"/>
                                    <input
                                        type="text"
                                        value={cacheControl}
                                        onChange={(e) => setCacheControl(e.target.value)}
                                        placeholder={t('web.cache_control_placeholder')}
                                        style={{
                                            width: '100%',
                                            padding: '6px 10px 6px 28px',
                                            borderRadius: 4,
                                            border: '1px solid var(--jb-border)',
                                            backgroundColor: 'var(--jb-editor-bg)',
                                            color: 'var(--jb-text)',
                                            fontSize: 12,
                                            boxSizing: 'border-box',
                                        }}
                                    />
                                </div>
                                <div style={{fontSize: 11, color: 'var(--jb-text-muted)', marginTop: 4}}>
                                    {t('web.cache_control_help')}
                                </div>
                            </div>
                        </div>

                        {/* SPA Fallback Toggle */}
                        <div
                            style={{
                                background: 'var(--jb-header-bg)',
                                border: '1px solid var(--jb-border)',
                                borderRadius: 6,
                                padding: '8px 12px',
                                display: 'flex',
                                alignItems: 'center',
                                justifyContent: 'space-between',
                            }}
                        >
                            <div style={{display: 'flex', alignItems: 'center', gap: 6}}>
                                <span className="i-ph:arrows-split-bold text-sky-400 text-14px"/>
                                <span style={{fontSize: 12, fontWeight: 600, color: 'var(--jb-text-heading)'}}>
                  {t('web.spa_fallback')}
                </span>
                                <span style={{fontSize: 11, color: 'var(--jb-text-muted)', marginLeft: 4}}>
                  ({t('web.spa_fallback_help')})
                </span>
                            </div>
                            <label style={{
                                display: 'flex',
                                alignItems: 'center',
                                gap: 5,
                                cursor: 'pointer',
                                fontSize: 12
                            }}>
                                <input
                                    type="checkbox"
                                    checked={spaFallback}
                                    onChange={(e) => setSpaFallback(e.target.checked)}
                                    style={{cursor: 'pointer'}}
                                />
                                <span style={{
                                    fontWeight: 600,
                                    color: spaFallback ? 'var(--jb-accent-blue)' : 'var(--jb-text-muted)'
                                }}>
                  {spaFallback ? 'ON' : 'OFF'}
                </span>
                            </label>
                        </div>

                        {/* Deploy Action Button */}
                        <div style={{display: 'flex', alignItems: 'center', gap: 10, marginTop: 2}}>
                            <Button
                                primary
                                onClick={() => deploy.mutate()}
                                disabled={deploy.isPending || (!selectedFile && !archiveRef.current?.files?.[0])}
                                style={{
                                    height: 28,
                                    fontSize: 12,
                                    display: 'flex',
                                    alignItems: 'center',
                                    gap: 6,
                                    padding: '0 14px',
                                }}
                            >
                                {deploy.isPending ? (
                                    <>
                                        <span className="i-ph:spinner-gap-bold animate-spin text-13px"/>
                                        <span>{t('web.deploying')}</span>
                                    </>
                                ) : (
                                    <>
                                        <span className="i-ph:rocket-launch-bold text-13px"/>
                                        <span>{t('web.deploy_btn')}</span>
                                    </>
                                )}
                            </Button>
                        </div>
                    </div>
                </div>

                {/* 5. Deployed Projects Section Card */}
                <div
                    style={{
                        border: '1px solid var(--jb-border)',
                        borderRadius: 8,
                        overflow: 'hidden',
                        backgroundColor: 'var(--jb-panel-bg)',
                        boxShadow: 'var(--jb-shadow)',
                    }}
                >
                    {/* Header */}
                    <div
                        style={{
                            padding: '10px 16px',
                            backgroundColor: 'var(--jb-header-bg)',
                            borderBottom: '1px solid var(--jb-border)',
                            display: 'flex',
                            alignItems: 'center',
                            justifyContent: 'space-between',
                            gap: 12,
                        }}
                    >
                        <div style={{
                            fontSize: 13,
                            fontWeight: 600,
                            color: 'var(--jb-text-heading)',
                            display: 'flex',
                            alignItems: 'center',
                            gap: 8
                        }}>
                            <span className="i-ph:squares-four-bold text-sky-400 text-15px"/>
                            <span>{t('web.projects_list')}</span>
                            <span style={{fontSize: 11, fontWeight: 400, color: 'var(--jb-text-muted)'}}>
                ({filteredProjects.length})
              </span>
                        </div>

                        {/* Search Filter */}
                        <div style={{width: 200, position: 'relative', display: 'flex', alignItems: 'center'}}>
                            <span className="i-ph:magnifying-glass-bold text-12px text-zinc-400 absolute left-2.5"/>
                            <input
                                type="text"
                                placeholder={t('web.search_placeholder')}
                                value={searchQuery}
                                onChange={(e) => setSearchQuery(e.target.value)}
                                style={{
                                    width: '100%',
                                    padding: '3px 20px 3px 24px',
                                    borderRadius: 4,
                                    border: '1px solid var(--jb-border)',
                                    backgroundColor: 'var(--jb-editor-bg)',
                                    color: 'var(--jb-text)',
                                    fontSize: 11,
                                    height: 22,
                                    boxSizing: 'border-box',
                                }}
                            />
                            {searchQuery && (
                                <button
                                    onClick={() => setSearchQuery('')}
                                    style={{
                                        position: 'absolute',
                                        right: 4,
                                        background: 'transparent',
                                        border: 'none',
                                        color: 'var(--jb-text-muted)',
                                        cursor: 'pointer',
                                        display: 'flex',
                                        padding: 0,
                                    }}
                                >
                                    <span className="i-ph:x-circle-fill text-11px"/>
                                </button>
                            )}
                        </div>
                    </div>

                    {/* Table Container with overflow-x auto */}
                    <div style={{overflowX: 'auto'}}>
                        {isLoading ? (
                            <div style={{
                                padding: 36,
                                display: 'flex',
                                justifyContent: 'center',
                                alignItems: 'center',
                                gap: 10
                            }}>
                                <Loader/>
                                <span style={{color: 'var(--jb-text-muted)', fontSize: 13}}>{t('app.loading')}</span>
                            </div>
                        ) : filteredProjects.length === 0 ? (
                            <div style={{padding: 48, textAlign: 'center', color: 'var(--jb-text-muted)'}}>
                                <div style={{display: 'flex', flexDirection: 'column', alignItems: 'center', gap: 10}}>
                                    <span className="i-ph:browsers-bold text-40px text-zinc-500"/>
                                    <span style={{fontSize: 13, fontWeight: 600}}>{t('web.empty')}</span>
                                </div>
                            </div>
                        ) : (
                            <table style={{width: '100%', borderCollapse: 'collapse', fontSize: 13, textAlign: 'left'}}>
                                <thead>
                                <tr style={{
                                    backgroundColor: 'var(--jb-header-bg)',
                                    borderBottom: '1px solid var(--jb-border)'
                                }}>
                                    <th style={{
                                        padding: '10px 16px',
                                        fontWeight: 600,
                                        color: 'var(--jb-text-heading)'
                                    }}>
                                        {t('web.project_name')}
                                    </th>
                                    <th style={{
                                        padding: '10px 16px',
                                        fontWeight: 600,
                                        color: 'var(--jb-text-heading)'
                                    }}>
                                        {t('web.routing_path')}
                                    </th>
                                    <th style={{
                                        padding: '10px 16px',
                                        fontWeight: 600,
                                        color: 'var(--jb-text-heading)'
                                    }}>
                                        {t('app.status')}
                                    </th>
                                    <th style={{
                                        padding: '10px 16px',
                                        fontWeight: 600,
                                        color: 'var(--jb-text-heading)'
                                    }}>
                                        {t('web.deployed_at')}
                                    </th>
                                    <th style={{
                                        padding: '10px 16px',
                                        fontWeight: 600,
                                        color: 'var(--jb-text-heading)',
                                        textAlign: 'right'
                                    }}>
                                        {t('app.actions')}
                                    </th>
                                </tr>
                                </thead>
                                <tbody>
                                {filteredProjects.map((project) => {
                                    const projectPath = project.alias || `/web/${project.name}/`
                                    const fullHref = projectPath.startsWith('/') ? projectPath : `/${projectPath}`

                                    return (
                                        <tr
                                            key={project.name}
                                            style={{
                                                borderBottom: '1px solid var(--jb-border)',
                                                transition: 'background-color 0.15s',
                                            }}
                                        >
                                            {/* Project Name */}
                                            <td style={{padding: '12px 16px', verticalAlign: 'middle'}}>
                                                <div style={{display: 'flex', alignItems: 'center', gap: 8}}>
                                                    <span
                                                        className="i-ph:globe-hemisphere-east-bold text-sky-400 text-16px shrink-0"/>
                                                    <div>
                                                        <div style={{
                                                            fontWeight: 700,
                                                            color: 'var(--jb-text-heading)',
                                                            fontSize: 13
                                                        }}>
                                                            {project.name}
                                                        </div>
                                                        <div style={{
                                                            display: 'flex',
                                                            gap: 6,
                                                            marginTop: 4,
                                                            flexWrap: 'wrap'
                                                        }}>
                                <span
                                    style={{
                                        fontSize: 10,
                                        padding: '1px 6px',
                                        borderRadius: 3,
                                        background: project.spaFallback ? 'rgba(53, 116, 240, 0.12)' : 'var(--jb-header-bg)',
                                        color: project.spaFallback ? 'var(--jb-accent-blue)' : 'var(--jb-text-muted)',
                                        border: '1px solid var(--jb-border)',
                                        fontWeight: 500,
                                    }}
                                >
                                  SPA: {project.spaFallback ? 'ON' : 'OFF'}
                                </span>

                                                            {project.notFound && (
                                                                <span
                                                                    style={{
                                                                        fontSize: 10,
                                                                        padding: '1px 6px',
                                                                        borderRadius: 3,
                                                                        background: 'var(--jb-header-bg)',
                                                                        color: 'var(--jb-text-muted)',
                                                                        border: '1px solid var(--jb-border)',
                                                                    }}
                                                                >
                                    404: {project.notFound}
                                  </span>
                                                            )}
                                                        </div>
                                                    </div>
                                                </div>
                                            </td>

                                            {/* Access Path & Copy */}
                                            <td style={{padding: '12px 16px', verticalAlign: 'middle'}}>
                                                <div style={{display: 'flex', alignItems: 'center', gap: 6}}>
                                                    <a
                                                        href={fullHref}
                                                        target="_blank"
                                                        rel="noreferrer"
                                                        style={{
                                                            color: 'var(--jb-accent-blue)',
                                                            textDecoration: 'none',
                                                            fontFamily: 'JetBrains Mono, monospace',
                                                            fontSize: 12,
                                                            fontWeight: 600,
                                                            display: 'inline-flex',
                                                            alignItems: 'center',
                                                            gap: 4,
                                                        }}
                                                    >
                                                        <span>{project.alias || `/web/${project.name}/`}</span>
                                                        <span className="i-ph:arrow-square-out-bold text-11px"/>
                                                    </a>

                                                    <Tooltip
                                                        title={copiedUrlProject === project.name ? t('web.url_copied') : t('web.copy_url')}>
                                                        <button
                                                            onClick={() => copyProjectUrl(project)}
                                                            style={{
                                                                background: 'transparent',
                                                                border: 'none',
                                                                color: copiedUrlProject === project.name ? 'var(--jb-accent-green)' : 'var(--jb-text-muted)',
                                                                cursor: 'pointer',
                                                                display: 'inline-flex',
                                                                padding: 2,
                                                            }}
                                                        >
                                                            <span
                                                                className={copiedUrlProject === project.name ? 'i-ph:check-bold text-12px' : 'i-ph:copy-bold text-12px'}/>
                                                        </button>
                                                    </Tooltip>
                                                </div>
                                            </td>

                                            {/* Status */}
                                            <td style={{padding: '12px 16px', verticalAlign: 'middle'}}>
                          <span
                              style={{
                                  fontSize: 11,
                                  padding: '2px 8px',
                                  borderRadius: 12,
                                  background: 'rgba(73, 156, 84, 0.15)',
                                  color: '#499c54',
                                  fontWeight: 600,
                                  display: 'inline-flex',
                                  alignItems: 'center',
                                  gap: 4,
                              }}
                          >
                            <span className="i-ph:check-circle-bold text-11px"/>
                            <span>{t('web.status.deployed')}</span>
                          </span>
                                            </td>

                                            {/* Deployed At */}
                                            <td style={{
                                                padding: '12px 16px',
                                                verticalAlign: 'middle',
                                                color: 'var(--jb-text-muted)',
                                                fontSize: 12
                                            }}>
                                                {project.deployedAt ? dayjs(project.deployedAt).format('YYYY-MM-DD HH:mm:ss') : '-'}
                                            </td>

                                            {/* Actions */}
                                            <td style={{
                                                padding: '12px 16px',
                                                verticalAlign: 'middle',
                                                textAlign: 'right'
                                            }}>
                                                <div style={{
                                                    display: 'flex',
                                                    gap: 6,
                                                    justifyContent: 'flex-end',
                                                    alignItems: 'center'
                                                }}>
                                                    <a
                                                        href={fullHref}
                                                        target="_blank"
                                                        rel="noreferrer"
                                                        style={{
                                                            background: 'var(--jb-panel-bg)',
                                                            border: '1px solid var(--jb-border)',
                                                            color: 'var(--jb-accent-blue)',
                                                            borderRadius: 4,
                                                            padding: '3px 8px',
                                                            fontSize: 11,
                                                            textDecoration: 'none',
                                                            fontWeight: 600,
                                                            display: 'inline-flex',
                                                            alignItems: 'center',
                                                            gap: 4,
                                                        }}
                                                    >
                                                        <span className="i-ph:arrow-square-out-bold text-11px"/>
                                                        <span>{t('web.open_site')}</span>
                                                    </a>

                                                    <Button
                                                        onClick={() => startEdit(project)}
                                                        style={{
                                                            height: 24,
                                                            fontSize: 11,
                                                            padding: '0 8px',
                                                            display: 'flex',
                                                            alignItems: 'center',
                                                            gap: 4
                                                        }}
                                                    >
                                                        <span className="i-ph:pencil-simple-line-bold text-11px"/>
                                                        <span>{t('web.edit_settings')}</span>
                                                    </Button>

                                                    <Button
                                                        onClick={() => openVersionsModal(project)}
                                                        style={{
                                                            height: 24,
                                                            fontSize: 11,
                                                            padding: '0 8px',
                                                            display: 'flex',
                                                            alignItems: 'center',
                                                            gap: 4
                                                        }}
                                                    >
                                                        <span
                                                            className="i-ph:clock-counter-clockwise-bold text-11px text-purple-400"/>
                                                        <span>{t('web.versions')}</span>
                                                    </Button>

                                                    <Button
                                                        danger
                                                        onClick={() => remove(project)}
                                                        style={{
                                                            height: 24,
                                                            fontSize: 11,
                                                            padding: '0 8px',
                                                            display: 'flex',
                                                            alignItems: 'center',
                                                            gap: 4
                                                        }}
                                                    >
                                                        <span className="i-ph:trash-bold text-11px"/>
                                                        <span>{t('app.delete')}</span>
                                                    </Button>
                                                </div>
                                            </td>
                                        </tr>
                                    )
                                })}
                                </tbody>
                            </table>
                        )}
                    </div>
                </div>
            </div>

            {/* 6. Edit Settings Modal */}
            {editingProject && (
                <div
                    style={{
                        position: 'fixed',
                        inset: 0,
                        backgroundColor: 'rgba(0, 0, 0, 0.6)',
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'center',
                        zIndex: 1000,
                        backdropFilter: 'blur(2px)',
                    }}
                    onClick={() => setEditingProject(null)}
                >
                    <div
                        onClick={(e) => e.stopPropagation()}
                        style={{
                            width: 540,
                            maxWidth: '92vw',
                            maxHeight: '85vh',
                            background: 'var(--jb-panel-bg)',
                            border: '1px solid var(--jb-border)',
                            borderRadius: 8,
                            boxShadow: 'var(--jb-shadow)',
                            display: 'flex',
                            flexDirection: 'column',
                            overflow: 'hidden',
                        }}
                    >
                        {/* Header */}
                        <div
                            style={{
                                padding: '10px 16px',
                                borderBottom: '1px solid var(--jb-border)',
                                background: 'var(--jb-header-bg)',
                                display: 'flex',
                                alignItems: 'center',
                                justifyContent: 'space-between',
                            }}
                        >
                            <h3 style={{
                                margin: 0,
                                fontSize: 13,
                                fontWeight: 700,
                                color: 'var(--jb-text-heading)',
                                display: 'flex',
                                alignItems: 'center',
                                gap: 8
                            }}>
                                <span className="i-ph:sliders-horizontal-bold text-sky-400 text-15px"/>
                                <span>{t('web.edit_modal_title', {name: editingProject.name})}</span>
                            </h3>
                            <button
                                onClick={() => setEditingProject(null)}
                                style={{
                                    background: 'transparent',
                                    border: 'none',
                                    color: 'var(--jb-text-muted)',
                                    cursor: 'pointer',
                                    display: 'flex',
                                    padding: 2
                                }}
                            >
                                <span className="i-ph:x-bold text-14px"/>
                            </button>
                        </div>

                        {/* Body */}
                        <div style={{
                            flex: 1,
                            overflowY: 'auto',
                            padding: '16px 20px',
                            display: 'flex',
                            flexDirection: 'column',
                            gap: 14
                        }}>
                            {/* Alias */}
                            <div>
                                <label style={{
                                    fontSize: 12,
                                    fontWeight: 600,
                                    color: 'var(--jb-text-muted)',
                                    display: 'block',
                                    marginBottom: 4
                                }}>
                                    {t('web.alias')}
                                </label>
                                <input
                                    type="text"
                                    value={editAlias}
                                    onChange={(e) => setEditAlias(e.target.value)}
                                    placeholder="/web/alias"
                                    style={{
                                        width: '100%',
                                        padding: '6px 10px',
                                        borderRadius: 4,
                                        border: '1px solid var(--jb-border)',
                                        backgroundColor: 'var(--jb-editor-bg)',
                                        color: 'var(--jb-text)',
                                        fontSize: 12,
                                        boxSizing: 'border-box',
                                    }}
                                />
                                <div style={{fontSize: 11, color: 'var(--jb-text-muted)', marginTop: 4}}>
                                    {t('web.alias_help')}
                                </div>
                            </div>

                            {/* Cache-Control */}
                            <div>
                                <div style={{
                                    display: 'flex',
                                    justifyContent: 'space-between',
                                    alignItems: 'center',
                                    marginBottom: 4,
                                    flexWrap: 'wrap',
                                    gap: 4
                                }}>
                                    <label style={{fontSize: 12, fontWeight: 600, color: 'var(--jb-text-muted)'}}>
                                        {t('web.cache_control')}
                                    </label>
                                    <div style={{display: 'flex', gap: 4}}>
                                        <button
                                            type="button"
                                            onClick={() => applyCachePreset('no-cache, no-store, must-revalidate', true)}
                                            style={{
                                                background: 'var(--jb-header-bg)',
                                                border: '1px solid var(--jb-border)',
                                                color: 'var(--jb-text)',
                                                fontSize: 10,
                                                padding: '1px 6px',
                                                borderRadius: 3,
                                                cursor: 'pointer'
                                            }}
                                        >
                                            {t('web.cache_preset.no_cache')}
                                        </button>
                                        <button
                                            type="button"
                                            onClick={() => applyCachePreset('public, max-age=3600, must-revalidate', true)}
                                            style={{
                                                background: 'var(--jb-header-bg)',
                                                border: '1px solid var(--jb-border)',
                                                color: 'var(--jb-text)',
                                                fontSize: 10,
                                                padding: '1px 6px',
                                                borderRadius: 3,
                                                cursor: 'pointer'
                                            }}
                                        >
                                            {t('web.cache_preset.one_hour')}
                                        </button>
                                        <button
                                            type="button"
                                            onClick={() => applyCachePreset('public, max-age=31536000, immutable', true)}
                                            style={{
                                                background: 'var(--jb-header-bg)',
                                                border: '1px solid var(--jb-border)',
                                                color: 'var(--jb-text)',
                                                fontSize: 10,
                                                padding: '1px 6px',
                                                borderRadius: 3,
                                                cursor: 'pointer'
                                            }}
                                        >
                                            {t('web.cache_preset.immutable')}
                                        </button>
                                    </div>
                                </div>
                                <input
                                    type="text"
                                    value={editCacheControl}
                                    onChange={(e) => setEditCacheControl(e.target.value)}
                                    style={{
                                        width: '100%',
                                        padding: '6px 10px',
                                        borderRadius: 4,
                                        border: '1px solid var(--jb-border)',
                                        backgroundColor: 'var(--jb-editor-bg)',
                                        color: 'var(--jb-text)',
                                        fontSize: 12,
                                        boxSizing: 'border-box',
                                    }}
                                />
                            </div>

                            {/* Custom 404 */}
                            <div>
                                <label style={{
                                    fontSize: 12,
                                    fontWeight: 600,
                                    color: 'var(--jb-text-muted)',
                                    display: 'block',
                                    marginBottom: 4
                                }}>
                                    {t('web.not_found')}
                                </label>
                                <input
                                    type="text"
                                    value={editNotFound}
                                    onChange={(e) => setEditNotFound(e.target.value)}
                                    placeholder="404.html"
                                    style={{
                                        width: '100%',
                                        padding: '6px 10px',
                                        borderRadius: 4,
                                        border: '1px solid var(--jb-border)',
                                        backgroundColor: 'var(--jb-editor-bg)',
                                        color: 'var(--jb-text)',
                                        fontSize: 12,
                                        boxSizing: 'border-box',
                                    }}
                                />
                            </div>

                            {/* SPA Fallback Checkbox */}
                            <div
                                style={{
                                    background: 'var(--jb-header-bg)',
                                    border: '1px solid var(--jb-border)',
                                    borderRadius: 6,
                                    padding: '8px 12px',
                                    display: 'flex',
                                    alignItems: 'center',
                                    justifyContent: 'space-between',
                                }}
                            >
                                <div>
                                    <div style={{fontSize: 12, fontWeight: 600, color: 'var(--jb-text-heading)'}}>
                                        {t('web.spa_fallback')}
                                    </div>
                                    <div style={{fontSize: 11, color: 'var(--jb-text-muted)'}}>
                                        {t('web.spa_fallback_help')}
                                    </div>
                                </div>
                                <input
                                    type="checkbox"
                                    checked={editSpaFallback}
                                    onChange={(e) => setEditSpaFallback(e.target.checked)}
                                    style={{width: 16, height: 16, cursor: 'pointer'}}
                                />
                            </div>
                        </div>

                        {/* Footer */}
                        <div
                            style={{
                                padding: '10px 16px',
                                borderTop: '1px solid var(--jb-border)',
                                background: 'var(--jb-header-bg)',
                                display: 'flex',
                                justifyContent: 'flex-end',
                                gap: 8,
                            }}
                        >
                            <Button onClick={() => setEditingProject(null)} style={{height: 28, fontSize: 12}}>
                                {t('app.cancel')}
                            </Button>
                            <Button
                                primary
                                onClick={() => update.mutate()}
                                disabled={update.isPending}
                                style={{height: 28, fontSize: 12, display: 'flex', alignItems: 'center', gap: 4}}
                            >
                                {update.isPending ? (
                                    <>
                                        <span className="i-ph:spinner-gap-bold animate-spin text-12px"/>
                                        <span>{t('app.loading')}</span>
                                    </>
                                ) : (
                                    <>
                                        <span className="i-ph:check-bold text-12px"/>
                                        <span>{t('app.save')}</span>
                                    </>
                                )}
                            </Button>
                        </div>
                    </div>
                </div>
            )}

            {/* 7. Version History & Rollback Modal */}
            {versionsModalProject && (
                <div
                    style={{
                        position: 'fixed',
                        inset: 0,
                        backgroundColor: 'rgba(0, 0, 0, 0.6)',
                        display: 'flex',
                        alignItems: 'center',
                        justifyContent: 'center',
                        zIndex: 1000,
                        backdropFilter: 'blur(2px)',
                    }}
                    onClick={() => setVersionsModalProject(null)}
                >
                    <div
                        onClick={(e) => e.stopPropagation()}
                        style={{
                            width: 580,
                            maxWidth: '92vw',
                            maxHeight: '85vh',
                            background: 'var(--jb-panel-bg)',
                            border: '1px solid var(--jb-border)',
                            borderRadius: 8,
                            boxShadow: 'var(--jb-shadow)',
                            display: 'flex',
                            flexDirection: 'column',
                            overflow: 'hidden',
                        }}
                    >
                        {/* Header */}
                        <div
                            style={{
                                padding: '10px 16px',
                                borderBottom: '1px solid var(--jb-border)',
                                background: 'var(--jb-header-bg)',
                                display: 'flex',
                                alignItems: 'center',
                                justifyContent: 'space-between',
                            }}
                        >
                            <div>
                                <h3 style={{
                                    margin: 0,
                                    fontSize: 13,
                                    fontWeight: 700,
                                    color: 'var(--jb-text-heading)',
                                    display: 'flex',
                                    alignItems: 'center',
                                    gap: 8
                                }}>
                                    <span className="i-ph:clock-counter-clockwise-bold text-purple-400 text-15px"/>
                                    <span>{t('web.versions_modal_title', {name: versionsModalProject.name})}</span>
                                </h3>
                            </div>
                            <button
                                onClick={() => setVersionsModalProject(null)}
                                style={{
                                    background: 'transparent',
                                    border: 'none',
                                    color: 'var(--jb-text-muted)',
                                    cursor: 'pointer',
                                    display: 'flex',
                                    padding: 2
                                }}
                            >
                                <span className="i-ph:x-bold text-14px"/>
                            </button>
                        </div>

                        {/* Description Subtitle */}
                        <div style={{
                            padding: '8px 16px',
                            background: 'var(--jb-header-bg)',
                            borderBottom: '1px solid var(--jb-border)',
                            fontSize: 11,
                            color: 'var(--jb-text-muted)'
                        }}>
                            {t('web.versions_modal_desc')}
                        </div>

                        {/* Versions List */}
                        <div style={{
                            flex: 1,
                            overflowY: 'auto',
                            padding: 16,
                            display: 'flex',
                            flexDirection: 'column',
                            gap: 8
                        }}>
                            {versionsLoading ? (
                                <div style={{
                                    padding: 24,
                                    display: 'flex',
                                    justifyContent: 'center',
                                    alignItems: 'center',
                                    gap: 8
                                }}>
                                    <Loader/>
                                    <span
                                        style={{color: 'var(--jb-text-muted)', fontSize: 12}}>{t('app.loading')}</span>
                                </div>
                            ) : (versionsMap[versionsModalProject.name] || []).length === 0 ? (
                                <div style={{padding: 32, textAlign: 'center', color: 'var(--jb-text-muted)'}}>
                                    <div style={{
                                        display: 'flex',
                                        flexDirection: 'column',
                                        alignItems: 'center',
                                        gap: 8
                                    }}>
                                        <span className="i-ph:clock-counter-clockwise-bold text-32px text-zinc-500"/>
                                        <span style={{fontSize: 13, fontWeight: 600}}>{t('web.versions_empty')}</span>
                                    </div>
                                </div>
                            ) : (
                                (versionsMap[versionsModalProject.name] || []).map((version) => {
                                    const readableTime = formatVersionTimestamp(version)

                                    return (
                                        <div
                                            key={version}
                                            style={{
                                                background: 'var(--jb-header-bg)',
                                                border: '1px solid var(--jb-border)',
                                                borderRadius: 6,
                                                padding: '10px 14px',
                                                display: 'flex',
                                                alignItems: 'center',
                                                justifyContent: 'space-between',
                                                gap: 12,
                                            }}
                                        >
                                            <div style={{display: 'flex', alignItems: 'center', gap: 10}}>
                                                <span
                                                    className="i-ph:archive-box-bold text-purple-400 text-16px shrink-0"/>
                                                <div>
                                                    <div style={{
                                                        fontWeight: 600,
                                                        color: 'var(--jb-text-heading)',
                                                        fontSize: 12,
                                                        fontFamily: 'JetBrains Mono, monospace'
                                                    }}>
                                                        {readableTime}
                                                    </div>
                                                    <div style={{
                                                        fontSize: 10,
                                                        color: 'var(--jb-text-muted)',
                                                        marginTop: 2,
                                                        fontFamily: 'monospace'
                                                    }}>
                                                        tag: {version}
                                                    </div>
                                                </div>
                                            </div>

                                            <Button
                                                primary
                                                onClick={() => {
                                                    if (window.confirm(t('web.rollback_confirm', {
                                                        name: versionsModalProject.name,
                                                        version
                                                    }))) {
                                                        rollbackMutation.mutate({
                                                            project: versionsModalProject,
                                                            version
                                                        })
                                                    }
                                                }}
                                                disabled={rollbackMutation.isPending}
                                                style={{
                                                    height: 26,
                                                    fontSize: 11,
                                                    display: 'flex',
                                                    alignItems: 'center',
                                                    gap: 4
                                                }}
                                            >
                                                {rollbackMutation.isPending ? (
                                                    <>
                                                        <span className="i-ph:spinner-gap-bold animate-spin text-11px"/>
                                                        <span>{t('web.rolling_back')}</span>
                                                    </>
                                                ) : (
                                                    <>
                                                        <span className="i-ph:arrow-counter-clockwise-bold text-11px"/>
                                                        <span>{t('web.rollback_btn')}</span>
                                                    </>
                                                )}
                                            </Button>
                                        </div>
                                    )
                                })
                            )}
                        </div>

                        {/* Footer */}
                        <div
                            style={{
                                padding: '10px 16px',
                                borderTop: '1px solid var(--jb-border)',
                                background: 'var(--jb-header-bg)',
                                display: 'flex',
                                justifyContent: 'flex-end',
                            }}
                        >
                            <Button onClick={() => setVersionsModalProject(null)} style={{height: 28, fontSize: 12}}>
                                {t('app.close')}
                            </Button>
                        </div>
                    </div>
                </div>
            )}
        </div>
    )
}
