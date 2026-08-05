import { useState } from 'react'
import { createFileRoute } from '@tanstack/react-router'
import { useSelector } from '@tanstack/react-store'
import Button from '@jetbrains/ring-ui-built/components/button/button'
import Island, { Header as IslandHeader, Content as IslandContent } from '@jetbrains/ring-ui-built/components/island/island'
import Tag from '@jetbrains/ring-ui-built/components/tag/tag'
import Input from '@jetbrains/ring-ui-built/components/input/input'
import Toggle from '@jetbrains/ring-ui-built/components/toggle/toggle'
import Text from '@jetbrains/ring-ui-built/components/text/text'
import Link from '@jetbrains/ring-ui-built/components/link/link'
import Group from '@jetbrains/ring-ui-built/components/group/group'

import { appStore } from '../store/app'
import { ThemedWrapper } from '../components/ThemedWrapper'
import './jetbrains-ide.css'

export const Route = createFileRoute('/demo')({
  component: JetBrainsIDEWorkspace,
})

function JetBrainsIDEWorkspace() {
  const [count, setCount] = useState(0)
  const [activeTab, setActiveTab] = useState<'index' | 'app' | 'store'>('index')
  const [selectedFile, setSelectedFile] = useState('src/routes/index.tsx')
  const [dockTab, setDockTab] = useState<'terminal' | 'problems' | 'logs'>('logs')
  const [demoInput, setDemoInput] = useState('JetBrains Ring UI Starter')

  // 读取与联动 TanStack Store
  const isDark = useSelector(appStore, (state) => state.dark)

  const [logs, setLogs] = useState<Array<{ id: number; time: string; type: 'info' | 'warn' | 'success'; message: string }>>([
    { id: 1, time: new Date().toLocaleTimeString(), type: 'info', message: 'Vite 8.2.0 Dev Server ready in 182ms.' },
    { id: 2, time: new Date().toLocaleTimeString(), type: 'success', message: 'TanStack Store appStore initialized with dark=' + isDark },
  ])

  const addLog = (message: string, type: 'info' | 'warn' | 'success' = 'info') => {
    setLogs((prev) => [
      ...prev,
      { id: Date.now(), time: new Date().toLocaleTimeString(), type, message },
    ])
  }

  const toggleTheme = () => {
    const newDark = !isDark
    appStore.setState((s) => ({ ...s, dark: newDark }))
    addLog(`Theme changed via appStore -> ${newDark ? 'DARK' : 'LIGHT'}`, 'warn')
  }

  const handleIncrement = () => {
    const nextCount = count + 1
    setCount(nextCount)
    addLog(`Count incremented to ${nextCount}`, 'info')
  }

  const handleReset = () => {
    setCount(0)
    addLog(`Count reset to 0`, 'warn')
  }

  return (
      <ThemedWrapper>
        <div className="jb-workspace">
          {/* 1. Header Toolbar */}
          <header className="jb-header">
            <div className="jb-header-left">
              <div className="jb-logo-badge">
                <div className="jb-logo-icon">JB</div>
                <span>IntelliJ IDEA</span>
              </div>
              <div className="jb-project-name">
                React-Ring-UI-Starter
                <span className="jb-branch-badge">
                <span className="i-ph:git-branch-bold text-11px mr-1" />
                main
              </span>
              </div>

              <div className="jb-menu-bar">
                <span className="jb-menu-item">File</span>
                <span className="jb-menu-item">Edit</span>
                <span className="jb-menu-item">View</span>
                <span className="jb-menu-item">Navigate</span>
                <span className="jb-menu-item">Code</span>
                <span className="jb-menu-item">Run</span>
                <span className="jb-menu-item">Tools</span>
              </div>
            </div>

            <div className="jb-search-everywhere">
              <span className="i-ph:magnifying-glass-bold text-13px" />
              <span>Search Everywhere</span>
              <span className="jb-kbd">Shift Shift</span>
            </div>

            <div className="jb-header-right">
              <Group>
                <Button primary onClick={handleIncrement}>
                  <span className="i-ph:play-fill text-12px mr-1" /> Run 'App'
                </Button>
                <Button onClick={() => addLog('Debugger attached.', 'success')}>
                  <span className="i-ph:bug-bold text-12px mr-1" /> Debug
                </Button>
              </Group>

              <div style={{ display: 'flex', alignItems: 'center', gap: '8px', marginLeft: '12px' }}>
                <span style={{ fontSize: '12px', color: 'var(--jb-text-muted)' }}>Dark Theme</span>
                <Toggle checked={isDark} onChange={toggleTheme} />
              </div>
            </div>
          </header>

          {/* 2. Main Workspace Layout */}
          <div className="jb-main-layout">
            {/* Left Side Tool Icon Bar */}
            <aside className="jb-tool-sidebar">
              <div className="jb-tool-icon active" title="Project (Alt+1)">
                <span className="i-ph:files-bold" />
              </div>
              <div className="jb-tool-icon" title="Structure (Alt+7)">
                <span className="i-ph:tree-structure-bold" />
              </div>
              <div className="jb-tool-icon" title="Version Control (Alt+9)">
                <span className="i-ph:git-branch-bold" />
              </div>
              <div className="jb-tool-icon" title="Terminal (Alt+F12)">
                <span className="i-ph:terminal-window-bold" />
              </div>
              <div style={{ marginTop: 'auto' }}>
                <div className="jb-tool-icon" title="Settings">
                  <span className="i-ph:gear-six-bold" />
                </div>
              </div>
            </aside>

            {/* Project Explorer Tree Panel */}
            <aside className="jb-project-panel">
              <div className="jb-panel-title">
                <span>Project</span>
                <span className="i-ph:gear-bold cursor-pointer text-14px" title="Project Settings" />
              </div>
              <div className="jb-tree-list">
                <div className="jb-tree-item" onClick={() => setSelectedFile('React-Ring-UI-Starter')}>
                  <span className="i-ph:folder-open-bold text-amber-500 text-14px" />
                  <strong>React-Ring-UI-Starter</strong>
                </div>

                <div className="jb-tree-item" style={{ paddingLeft: '24px' }}>
                  <span className="i-ph:folder-bold text-amber-400 text-14px" /> src
                </div>

                <div className="jb-tree-item" style={{ paddingLeft: '36px' }}>
                  <span className="i-ph:folder-bold text-amber-400 text-14px" /> routes
                </div>

                <div
                    className={`jb-tree-item ${selectedFile.includes('index.tsx') ? 'active' : ''}`}
                    style={{ paddingLeft: '48px' }}
                    onClick={() => {
                      setActiveTab('index')
                      setSelectedFile('src/routes/index.tsx')
                    }}
                >
                  <span className="i-ph:atom-bold text-sky-400 text-14px" /> index.tsx
                </div>

                <div
                    className={`jb-tree-item ${selectedFile.includes('app.tsx') ? 'active' : ''}`}
                    style={{ paddingLeft: '48px' }}
                    onClick={() => {
                      setActiveTab('app')
                      setSelectedFile('src/routes/app.tsx')
                    }}
                >
                  <span className="i-ph:atom-bold text-sky-400 text-14px" /> app.tsx
                </div>

                <div className="jb-tree-item" style={{ paddingLeft: '36px' }}>
                  <span className="i-ph:folder-bold text-amber-400 text-14px" /> store
                </div>

                <div
                    className={`jb-tree-item ${selectedFile.includes('app.ts') ? 'active' : ''}`}
                    style={{ paddingLeft: '48px' }}
                    onClick={() => {
                      setActiveTab('store')
                      setSelectedFile('src/store/app.ts')
                    }}
                >
                  <span className="i-ph:file-ts-bold text-blue-500 text-14px" /> app.ts
                </div>

                <div className="jb-tree-item" style={{ paddingLeft: '36px' }}>
                  <span className="i-ph:atom-bold text-sky-400 text-14px" /> App.tsx
                </div>

                <div className="jb-tree-item" style={{ paddingLeft: '36px' }}>
                  <span className="i-ph:atom-bold text-sky-400 text-14px" /> main.tsx
                </div>

                <div className="jb-tree-item" style={{ paddingLeft: '24px' }}>
                  <span className="i-ph:file-js-bold text-yellow-500 text-14px" /> package.json
                </div>

                <div className="jb-tree-item" style={{ paddingLeft: '24px' }}>
                  <span className="i-ph:gear-bold text-emerald-500 text-14px" /> vite.config.ts
                </div>
              </div>
            </aside>

            {/* Center Editor Container */}
            <main className="jb-editor-container">
              {/* Editor Tabs */}
              <div className="jb-editor-tabs">
                <div
                    className={`jb-editor-tab ${activeTab === 'index' ? 'active' : ''}`}
                    onClick={() => {
                      setActiveTab('index')
                      setSelectedFile('src/routes/index.tsx')
                    }}
                >
                  <span className="i-ph:atom-bold text-sky-400 text-13px" /> index.tsx
                  <span className="jb-tab-close">
                  <span className="i-ph:x-bold text-9px" />
                </span>
                </div>

                <div
                    className={`jb-editor-tab ${activeTab === 'store' ? 'active' : ''}`}
                    onClick={() => {
                      setActiveTab('store')
                      setSelectedFile('src/store/app.ts')
                    }}
                >
                  <span className="i-ph:file-ts-bold text-blue-500 text-13px" /> app.ts
                  <span className="jb-tab-close">
                  <span className="i-ph:x-bold text-9px" />
                </span>
                </div>

                <div
                    className={`jb-editor-tab ${activeTab === 'app' ? 'active' : ''}`}
                    onClick={() => {
                      setActiveTab('app')
                      setSelectedFile('src/routes/app.tsx')
                    }}
                >
                  <span className="i-ph:atom-bold text-sky-400 text-13px" /> app.tsx
                  <span className="jb-tab-close">
                  <span className="i-ph:x-bold text-9px" />
                </span>
                </div>
              </div>

              {/* Breadcrumbs */}
              <div className="jb-breadcrumbs">
                <span>React-Ring-UI-Starter</span>
                <span className="jb-breadcrumb-sep">›</span>
                <span>src</span>
                <span className="jb-breadcrumb-sep">›</span>
                <span>{selectedFile.split('/').pop()}</span>
                <span className="jb-breadcrumb-sep">›</span>
                <span style={{ color: 'var(--jb-accent-purple)' }}>JetBrainsIDEWorkspace()</span>
              </div>

              {/* Editor Content Area */}
              <div className="jb-editor-content">
                {/* JetBrains IDE Dashboard Header Banner */}
                <div className="jb-welcome-banner">
                  <div className="jb-banner-info">
                    <h2>JetBrains Ring UI Workspace</h2>
                    <p>Welcome to JetBrains Ring UI Starter with React 19, Vite, TanStack Router & TanStack Store.</p>
                  </div>
                  <Group style={{ display: 'flex', gap: '8px' }}>
                    <Tag readOnly>React 19.2</Tag>
                    <Tag readOnly>Ring UI 7.0</Tag>
                    <Tag readOnly>UnoCSS + Iconify</Tag>
                  </Group>
                </div>

                {/* Grid of JetBrains Inspector Cards */}
                <div className="jb-grid-container">
                  {/* Card 1: Counter & Interactive Actions */}
                  <Island className="jb-card">
                    <IslandHeader border>
                      <div className="jb-card-title">
                        <span className="i-ph:lightning-bold text-amber-400 text-16px" />
                        <span>Interactive State Inspector</span>
                      </div>
                    </IslandHeader>
                    <IslandContent>
                      <Text style={{ marginBottom: '12px', display: 'block' }}>
                        Current Count: <strong style={{ color: 'var(--jb-accent-blue)', fontSize: '18px' }}>{count}</strong>
                      </Text>
                      <Group style={{ display: 'flex', gap: '8px', flexWrap: 'wrap', marginBottom: '16px' }}>
                        <Button primary onClick={handleIncrement}>
                          <span className="i-ph:plus-bold mr-1" /> Increment
                        </Button>
                        <Button danger onClick={handleReset}>
                          <span className="i-ph:arrow-counter-clockwise-bold mr-1" /> Reset
                        </Button>
                        <Button onClick={() => addLog('Manual event triggered in editor.', 'info')}>
                          <span className="i-ph:paper-plane-tilt-bold mr-1" /> Log Event
                        </Button>
                      </Group>
                      <div style={{ background: 'rgba(0,0,0,0.15)', padding: '10px 12px', borderRadius: '6px', fontSize: '12px' }}>
                        <code>appStore.dark</code>: <span style={{ color: isDark ? '#499c54' : '#e5934e' }}>{String(isDark)}</span>
                      </div>
                    </IslandContent>
                  </Island>

                  {/* Card 2: Ring UI Component Gallery */}
                  <Island className="jb-card">
                    <IslandHeader border>
                      <div className="jb-card-title">
                        <span className="i-ph:palette-bold text-purple-400 text-16px" />
                        <span>Ring UI Controls Gallery</span>
                      </div>
                    </IslandHeader>
                    <IslandContent style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
                      <div>
                        <Text size={Text.Size.S} style={{ marginBottom: '6px', display: 'block', color: 'var(--jb-text-muted)' }}>
                          Ring UI Input:
                        </Text>
                        <Input
                            value={demoInput}
                            onChange={(e) => setDemoInput(e.target.value)}
                            placeholder="Type something..."
                        />
                      </div>

                      <div>
                        <Text size={Text.Size.S} style={{ marginBottom: '6px', display: 'block', color: 'var(--jb-text-muted)' }}>
                          Ring UI Badges & Status Tags:
                        </Text>
                        <div style={{ display: 'flex', gap: '6px', flexWrap: 'wrap' }}>
                          <Tag readOnly>TanStack Store</Tag>
                          <Tag readOnly>Fast Refresh</Tag>
                          <Tag readOnly>Theme: {isDark ? 'Dark' : 'Light'}</Tag>
                        </div>
                      </div>
                    </IslandContent>
                  </Island>

                  {/* Card 3: JetBrains Documentation & Resources */}
                  <Island className="jb-card">
                    <IslandHeader border>
                      <div className="jb-card-title">
                        <span className="i-ph:globe-bold text-sky-400 text-16px" />
                        <span>External Resources & Documentation</span>
                      </div>
                    </IslandHeader>
                    <IslandContent style={{ display: 'flex', flexDirection: 'column', gap: '10px' }}>
                      <Text size={Text.Size.S} style={{ color: 'var(--jb-text-muted)' }}>
                        Quick access links for frameworks used in this starter:
                      </Text>
                      <div style={{ display: 'grid', gridTemplateColumns: '1fr 1fr', gap: '8px' }}>
                        <Link href="https://ring-ui.jetbrains.org/" target="_blank">
                          JetBrains Ring UI ↗
                        </Link>
                        <Link href="https://react.dev/" target="_blank">
                          React 19 Docs ↗
                        </Link>
                        <Link href="https://vite.dev/" target="_blank">
                          Vite Guide ↗
                        </Link>
                        <Link href="https://tanstack.com/router/latest" target="_blank">
                          TanStack Router ↗
                        </Link>
                        <Link href="https://github.com/JetBrains/ring-ui" target="_blank">
                          GitHub Repo ↗
                        </Link>
                        <Link href="https://chat.vite.dev/" target="_blank">
                          Discord Community ↗
                        </Link>
                      </div>
                    </IslandContent>
                  </Island>
                </div>
              </div>

              {/* 3. Bottom Terminal & Output Console */}
              <div className="jb-bottom-dock">
                <div className="jb-dock-tabs">
                  <div
                      className={`jb-dock-tab ${dockTab === 'logs' ? 'active' : ''}`}
                      onClick={() => setDockTab('logs')}
                  >
                    <span className="i-ph:scroll-bold text-12px" />
                    <span>Event Console ({logs.length})</span>
                  </div>
                  <div
                      className={`jb-dock-tab ${dockTab === 'terminal' ? 'active' : ''}`}
                      onClick={() => setDockTab('terminal')}
                  >
                    <span className="i-ph:terminal-bold text-12px" />
                    <span>Terminal</span>
                  </div>
                  <div
                      className={`jb-dock-tab ${dockTab === 'problems' ? 'active' : ''}`}
                      onClick={() => setDockTab('problems')}
                  >
                    <span className="i-ph:warning-circle-bold text-12px" />
                    <span>Problems (0)</span>
                  </div>
                </div>

                <div className="jb-dock-content">
                  {dockTab === 'logs' && (
                      <div>
                        {logs.map((log) => (
                            <div key={log.id} className="jb-log-line">
                              <span className="jb-log-time">[{log.time}]</span>
                              <span className={`jb-log-${log.type}`}>[{log.type.toUpperCase()}]</span>
                              <span>{log.message}</span>
                            </div>
                        ))}
                      </div>
                  )}

                  {dockTab === 'terminal' && (
                      <div>
                        <div style={{ color: '#499c54' }}>$ pnpm run dev</div>
                        <div>Vite v8.2.0 dev server running at:</div>
                        <div>➜ Local: http://localhost:5173/</div>
                        <div>➜ Network: use --host to expose</div>
                      </div>
                  )}

                  {dockTab === 'problems' && (
                      <div style={{ color: '#499c54' }}>
                        ✓ No syntax or type errors detected in project workspace.
                      </div>
                  )}
                </div>
              </div>
            </main>
          </div>

          {/* 4. Status Bar */}
          <footer className="jb-status-bar">
            <div className="jb-status-left">
            <span className="jb-status-item">
              <span className="i-ph:git-branch-bold text-11px" /> main
            </span>
              <span className="jb-status-item">
              <span className="i-ph:check-circle-bold text-11px text-emerald-400" /> 0 errors, 0 warnings
            </span>
            </div>
            <div className="jb-status-right">
              <span className="jb-status-item">Ln 12, Col 24</span>
              <span className="jb-status-item">UTF-8</span>
              <span className="jb-status-item">TypeScript TSX</span>
              <span className="jb-status-item">384M / 2048M</span>
            </div>
          </footer>
        </div>
      </ThemedWrapper>
  )
}
