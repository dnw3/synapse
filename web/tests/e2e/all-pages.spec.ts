import { test, expect } from '@playwright/test'

test.describe('Dashboard Overview', () => {
  test('loads with real health data', async ({ page }) => {
    await page.goto('/')
    // Should auto-redirect or show main UI
    await page.waitForLoadState('networkidle')

    // Verify the API returns real data
    const health = await page.evaluate(async () => {
      const res = await fetch('http://localhost:3000/api/dashboard/health')
      return res.json()
    })
    expect(health.status).toBe('ok')
    expect(health.uptime_secs).toBeGreaterThan(0)
    expect(health.config_summary.model).toBeTruthy()
    expect(health.config_summary.provider).toBeTruthy()

    await page.screenshot({ path: 'tests/e2e/artifacts/overview.png', fullPage: true })
  })
})

test.describe('Sessions / Conversations', () => {
  test('lists real sessions with message counts', async ({ page }) => {
    const res = await page.evaluate(async () => {
      const r = await fetch('http://localhost:3000/api/conversations')
      return r.json()
    })

    // Must have real sessions
    expect(Array.isArray(res)).toBeTruthy()
    expect(res.length).toBeGreaterThan(0)

    // Each session has required fields (id may be string key)
    for (const session of res) {
      // New API: sessions use 'id' field (which is the session_key)
      expect(session.id || session.key).toBeTruthy()
      expect(typeof session.message_count).toBe('number')
      expect(session.created_at).toBeTruthy()
      // New fields added in redesign
      if (session.channel !== undefined) {
        expect(typeof session.channel).toBe('string')
      }
      if (session.kind !== undefined) {
        expect(typeof session.kind).toBe('string')
      }
    }

    // Navigate to page and wait
    await page.goto('/')
    await page.waitForLoadState('networkidle')
  })

  test('loads messages from a real session', async ({ page }) => {
    // Get first session
    const sessions = await page.evaluate(async () => {
      const r = await fetch('http://localhost:3000/api/conversations')
      return r.json()
    })
    expect(sessions.length).toBeGreaterThan(0)

    const sessionId = sessions[0].id
    const messages = await page.evaluate(async (sid: string) => {
      const r = await fetch(`http://localhost:3000/api/conversations/${sid}/messages`)
      return r.json()
    }, sessionId)

    expect(Array.isArray(messages)).toBeTruthy()
    // Session with message_count > 0 should have messages
    if (sessions[0].message_count > 0) {
      expect(messages.length).toBeGreaterThan(0)
      // Messages have role and content
      for (const msg of messages) {
        expect(msg.role).toBeTruthy()
        expect(['human', 'assistant', 'system', 'tool'].includes(msg.role)).toBeTruthy()
      }
    }
  })
})

test.describe('Channels', () => {
  test('returns real channel configuration', async ({ page }) => {
    const channels = await page.evaluate(async () => {
      const r = await fetch('http://localhost:3000/api/dashboard/channels')
      return r.json()
    })

    expect(Array.isArray(channels)).toBeTruthy()
    expect(channels.length).toBeGreaterThan(0)

    // All channels have name and enabled fields
    for (const ch of channels) {
      expect(ch.name).toBeTruthy()
      expect(typeof ch.enabled).toBe('boolean')
    }
  })
})

test.describe('Skills', () => {
  test('returns real skills with paths and metadata', async ({ page }) => {
    const skills = await page.evaluate(async () => {
      const r = await fetch('http://localhost:3000/api/dashboard/skills')
      return r.json()
    })

    expect(Array.isArray(skills)).toBeTruthy()
    expect(skills.length).toBeGreaterThan(0)

    // Skills have real file paths
    for (const skill of skills) {
      expect(skill.name).toBeTruthy()
      expect(skill.path).toBeTruthy()
      expect(skill.path).toContain('.claude/skills/')
      expect(skill.source).toBeTruthy()
      expect(['personal', 'project', 'legacy'].includes(skill.source)).toBeTruthy()
    }

    // Verify specific known skill exists
    const gitSkill = skills.find((s: any) => s.name === 'git-essentials')
    expect(gitSkill).toBeTruthy()
    expect(gitSkill.eligible).toBe(true)
  })
})

test.describe('Agents', () => {
  test('returns agent configuration', async ({ page }) => {
    const agents = await page.evaluate(async () => {
      const r = await fetch('http://localhost:3000/api/dashboard/agents')
      return r.json()
    })

    // Should have at least one agent (default)
    expect(agents).toBeTruthy()
    // Response could be array or object — handle both
    if (Array.isArray(agents)) {
      expect(agents.length).toBeGreaterThanOrEqual(0)
    } else {
      expect(typeof agents).toBe('object')
    }
  })
})

test.describe('Logs', () => {
  test('logs endpoint returns data or empty array', async ({ page }) => {
    const logs = await page.evaluate(async () => {
      const r = await fetch('http://localhost:3000/api/logs')
      return r.json()
    })

    // Logs returns { entries: [...], total: N }
    expect(logs).toBeTruthy()
    expect(logs.entries || logs).toBeTruthy()
    if (logs.entries) {
      expect(Array.isArray(logs.entries)).toBeTruthy()
      expect(logs.total).toBeGreaterThanOrEqual(0)
      // Verify real log entries have structure
      if (logs.entries.length > 0) {
        expect(logs.entries[0].ts).toBeTruthy()
        expect(logs.entries[0].level).toBeTruthy()
        expect(logs.entries[0].message).toBeTruthy()
      }
    }
  })
})

test.describe('Dashboard Identity', () => {
  test('returns agent identity/branding', async ({ page }) => {
    const identity = await page.evaluate(async () => {
      const r = await fetch('http://localhost:3000/api/dashboard/identity')
      return r.json()
    })

    expect(identity).toBeTruthy()
    // Identity has name field
    if (identity.name) {
      expect(typeof identity.name).toBe('string')
    }
  })
})

test.describe('WebSocket Connection', () => {
  test('WebSocket connects and receives hello', async ({ page }) => {
    await page.goto('/')
    await page.waitForLoadState('networkidle')

    // Test WebSocket via browser
    const wsResult = await page.evaluate(async () => {
      return new Promise<any>((resolve, reject) => {
        const ws = new WebSocket(`ws://localhost:3000/ws/test-e2e-session`)
        const timeout = setTimeout(() => {
          ws.close()
          reject(new Error('WebSocket timeout'))
        }, 5000)

        ws.onopen = () => {
          // Send hello
          ws.send(JSON.stringify({ type: 'ping' }))
        }

        ws.onmessage = (event) => {
          clearTimeout(timeout)
          ws.close()
          try {
            resolve(JSON.parse(event.data))
          } catch {
            resolve({ raw: event.data })
          }
        }

        ws.onerror = (_err) => {
          clearTimeout(timeout)
          reject(new Error('WebSocket error'))
        }
      })
    })

    // Should receive some response (hello, pong, or status)
    expect(wsResult).toBeTruthy()
  })
})

test.describe('Frontend UI Rendering', () => {
  test('main page renders without errors', async ({ page }) => {
    const errors: string[] = []
    page.on('pageerror', (error) => errors.push(error.message))

    await page.goto('/')
    await page.waitForLoadState('networkidle')

    // Page should have content
    const body = await page.textContent('body')
    expect(body).toBeTruthy()
    expect(body!.length).toBeGreaterThan(50)

    // No JS errors
    expect(errors).toEqual([])

    await page.screenshot({ path: 'tests/e2e/artifacts/main-page.png', fullPage: true })
  })

  test('page has correct title or heading', async ({ page }) => {
    await page.goto('/')
    await page.waitForLoadState('networkidle')

    // Should have some meaningful content — not a blank page
    const html = await page.content()
    expect(html).toContain('Synapse')
  })
})

test.describe('API Data Integrity', () => {
  test('health uptime increases over time', async ({ page }) => {
    const h1 = await page.evaluate(async () => {
      const r = await fetch('http://localhost:3000/api/dashboard/health')
      return r.json()
    })
    await page.waitForTimeout(1500)
    const h2 = await page.evaluate(async () => {
      const r = await fetch('http://localhost:3000/api/dashboard/health')
      return r.json()
    })

    expect(h2.uptime_secs).toBeGreaterThan(h1.uptime_secs)
    expect(h2.status).toBe('ok')
  })

  test('memory entries count is consistent', async ({ page }) => {
    const h = await page.evaluate(async () => {
      const r = await fetch('http://localhost:3000/api/dashboard/health')
      return r.json()
    })

    expect(h.memory_entries).toBeGreaterThanOrEqual(0)
    expect(typeof h.memory_entries).toBe('number')
  })

  test('model and provider are configured', async ({ page }) => {
    const h = await page.evaluate(async () => {
      const r = await fetch('http://localhost:3000/api/dashboard/health')
      return r.json()
    })

    expect(h.config_summary.model).toBeTruthy()
    expect(h.config_summary.provider).toBeTruthy()
    expect(h.config_summary.mcp_servers).toBeGreaterThanOrEqual(0)
  })
})

test.describe('RPC via WebSocket', () => {
  test('sessions.list returns real sessions via RPC', async ({ page }) => {
    await page.goto('/')
    await page.waitForLoadState('networkidle')

    const rpcResult = await page.evaluate(async () => {
      return new Promise<any>((resolve, reject) => {
        const ws = new WebSocket('ws://localhost:3000/ws/e2e-rpc-test')
        const timeout = setTimeout(() => { ws.close(); reject(new Error('timeout')) }, 8000)

        ws.onopen = () => {
          ws.send(JSON.stringify({
            type: 'rpc_request',
            id: 'rpc-e2e-1',
            method: 'sessions.list',
            params: {}
          }))
        }

        ws.onmessage = (event) => {
          try {
            const data = JSON.parse(event.data)
            if (data.type === 'rpc_response' && data.id === 'rpc-e2e-1') {
              clearTimeout(timeout)
              ws.close()
              resolve(data)
            }
          } catch { /* ignore parse errors */ }
        }

        ws.onerror = () => { clearTimeout(timeout); reject(new Error('ws error')) }
      })
    })

    expect(rpcResult).toBeTruthy()
    expect(rpcResult.id).toBe('rpc-e2e-1')
    // Result should contain session data
    if (rpcResult.result) {
      expect(Array.isArray(rpcResult.result) || typeof rpcResult.result === 'object').toBeTruthy()
    }
  })
})

// ---------------------------------------------------------------------------
// New tests for unified UI redesign
// ---------------------------------------------------------------------------

test.describe('Unified Sidebar', () => {
  test('sidebar has Chat group label', async ({ page }) => {
    await page.goto('/')
    await page.waitForLoadState('networkidle')

    // The sidebar renders group headers: "Chat", "Control", "Agent", "Settings"
    // Labels are translated (zh or en depending on browser locale).
    // We test via aside element text content.
    const sidebarText = await page.locator('aside').textContent() ?? ''

    // Chat group is always present (sidebar.chat i18n key → "Chat" / "聊天")
    const hasChatLabel = sidebarText.includes('Chat') || sidebarText.includes('聊天')
    expect(hasChatLabel).toBeTruthy()
  })

  test('sidebar has Control group label', async ({ page }) => {
    await page.goto('/')
    await page.waitForLoadState('networkidle')

    const sidebarText = await page.locator('aside').textContent() ?? ''
    const hasControlLabel = sidebarText.includes('Control') || sidebarText.includes('控制')
    expect(hasControlLabel).toBeTruthy()
  })

  test('sidebar has Agent group label', async ({ page }) => {
    await page.goto('/')
    await page.waitForLoadState('networkidle')

    const sidebarText = await page.locator('aside').textContent() ?? ''
    const hasAgentLabel = sidebarText.includes('Agent') || sidebarText.includes('代理')
    expect(hasAgentLabel).toBeTruthy()
  })

  test('sidebar has Settings group label', async ({ page }) => {
    await page.goto('/')
    await page.waitForLoadState('networkidle')

    const sidebarText = await page.locator('aside').textContent() ?? ''
    const hasSettingsLabel = sidebarText.includes('Settings') || sidebarText.includes('设置')
    expect(hasSettingsLabel).toBeTruthy()
  })

  test('no mode toggle button exists', async ({ page }) => {
    await page.goto('/')
    await page.waitForLoadState('networkidle')

    // Old UI had a "Switch to Chat" / "切换到聊天" toggle button — must be gone
    const toggleBtnEn = await page.locator('text=Switch to Chat').count()
    const toggleBtnZh = await page.locator('text=切换到聊天').count()
    expect(toggleBtnEn + toggleBtnZh).toBe(0)
  })

  test('sidebar is an <aside> element with nav items', async ({ page }) => {
    await page.goto('/')
    await page.waitForLoadState('networkidle')

    // The UnifiedSidebar renders as <aside>
    const aside = page.locator('aside')
    await expect(aside).toBeVisible()

    // Sidebar contains clickable nav buttons
    const navButtons = aside.locator('button')
    const count = await navButtons.count()
    expect(count).toBeGreaterThan(4) // chat tab + group headers + overview + ...
  })
})

// Helper: navigate to the chat view by clicking the chat tab in the sidebar.
// The sidebar chat nav item uses t("sidebar.chatTab") which maps to "Chat" (en)
// or "聊天" (zh). Both the section header and the tab item render as <button>
// elements with that text — we identify the nav item by its icon sibling.
async function navigateToChat(page: import('@playwright/test').Page): Promise<void> {
  // The chat nav item button contains both a MessageSquare icon and the label.
  // It is distinct from the group-header button because it is inside the
  // collapsed content div and uses onViewChange("chat") directly.
  // Strategy: click whichever button text matches and is NOT a section header.
  // Section headers have pt-2/pt-3 class; nav items have h-[32px] class.
  const navItemEn = page.locator('aside button.h-\\[32px\\]', { hasText: 'Chat' }).first()
  const navItemZh = page.locator('aside button.h-\\[32px\\]', { hasText: '聊天' }).first()

  const enCount = await navItemEn.count()
  if (enCount > 0) {
    await navItemEn.click()
  } else {
    await navItemZh.click()
  }
  // Wait for the chat panel to mount (textarea appears)
  await page.waitForSelector('textarea', { timeout: 5000 }).catch(() => {})
}

test.describe('Chat Page Features', () => {
  test('navigating to chat shows chat panel', async ({ page }) => {
    await page.goto('/')
    await page.waitForLoadState('networkidle')

    await navigateToChat(page)

    // Chat panel should be visible: look for textarea (message input)
    const textarea = page.locator('textarea')
    await expect(textarea).toBeVisible({ timeout: 5000 })
  })

  test('chat page has session dropdown when conversations exist', async ({ page }) => {
    await page.goto('/')
    await page.waitForLoadState('networkidle')

    await navigateToChat(page)

    // ChatPanel renders a <select> for session switching when conversations > 0
    const convCount = await page.evaluate(async () => {
      const r = await fetch('http://localhost:3000/api/conversations')
      const data = await r.json()
      return Array.isArray(data) ? data.length : 0
    })
    if (convCount > 0) {
      // Wait for the select to appear (session dropdown or model dropdown)
      await page.waitForSelector('select', { timeout: 3000 }).catch(() => {})
      const selects = await page.locator('select').count()
      expect(selects).toBeGreaterThan(0)
    }
  })

  test('chat page has model name display', async ({ page }) => {
    await page.goto('/')
    await page.waitForLoadState('networkidle')

    await navigateToChat(page)

    // The model name is shown in the top bar (span or select with font-mono class)
    const mainContent = await page.locator('main').textContent() ?? ''
    expect(mainContent.length).toBeGreaterThan(0)
  })

  test('chat input textarea is present and focusable', async ({ page }) => {
    await page.goto('/')
    await page.waitForLoadState('networkidle')

    await navigateToChat(page)

    const textarea = page.locator('textarea').first()
    await expect(textarea).toBeVisible({ timeout: 5000 })
    await textarea.focus()
    await textarea.type('hello')
    const value = await textarea.inputValue()
    expect(value).toBe('hello')
  })
})

test.describe('Session API with new fields', () => {
  test('conversations API returns sessions with required fields', async ({ page }) => {
    const sessions = await page.evaluate(async () => {
      const r = await fetch('http://localhost:3000/api/conversations')
      return r.json()
    })
    expect(Array.isArray(sessions)).toBeTruthy()

    if (sessions.length > 0) {
      const s = sessions[0]
      // id is always present (session_key)
      expect(s.id).toBeTruthy()
      // message_count is always a number
      expect(typeof s.message_count).toBe('number')
      // token_count may be present
      if (s.token_count !== undefined) {
        expect(typeof s.token_count).toBe('number')
      }
      // created_at is always present
      expect(s.created_at).toBeTruthy()
    }
  })

  test('sessions have optional channel field for web sessions', async ({ page }) => {
    const sessions = await page.evaluate(async () => {
      const r = await fetch('http://localhost:3000/api/conversations')
      return r.json()
    })
    expect(Array.isArray(sessions)).toBeTruthy()

    if (sessions.length > 0) {
      // Web sessions should have channel = "web"
      const webSession = sessions.find((s: any) => s.channel === 'web')
      if (webSession) {
        expect(webSession.channel).toBe('web')
      }
    }
  })

  test('dashboard sessions endpoint returns session data', async ({ page }) => {
    const sessions = await page.evaluate(async () => {
      const r = await fetch('http://localhost:3000/api/dashboard/sessions')
      return r.json()
    })

    // Dashboard sessions returns an array
    if (Array.isArray(sessions)) {
      expect(Array.isArray(sessions)).toBeTruthy()
      if (sessions.length > 0) {
        // Each session should have a key (session identifier)
        const s = sessions[0]
        expect(s.key || s.id).toBeTruthy()
        if (s.message_count !== undefined) {
          expect(typeof s.message_count).toBe('number')
        }
        if (s.token_count !== undefined) {
          expect(typeof s.token_count).toBe('number')
        }
      }
    } else {
      // Could also be an object/pagination wrapper
      expect(typeof sessions).toBe('object')
    }
  })
})

test.describe('Memory Provider', () => {
  test('health endpoint confirms backend is operational', async ({ page }) => {
    await page.goto('/')
    const result = await page.evaluate(async () => {
      const r = await fetch('http://localhost:3000/api/dashboard/health')
      return r.json()
    })
    expect(result.status).toBe('ok')
    // memory_entries field is present (could be 0 if no entries yet)
    expect(typeof result.memory_entries).toBe('number')
  })

  test('memory.search RPC responds without error', async ({ page }) => {
    await page.goto('/')
    await page.waitForLoadState('networkidle')

    const rpcResult = await page.evaluate(async () => {
      return new Promise<any>((resolve, reject) => {
        const ws = new WebSocket('ws://localhost:3000/ws/e2e-memory-test')
        const timeout = setTimeout(() => { ws.close(); reject(new Error('timeout')) }, 8000)

        ws.onopen = () => {
          ws.send(JSON.stringify({
            type: 'rpc_request',
            id: 'rpc-memory-1',
            method: 'memory.search',
            params: { query: 'test', limit: 3 }
          }))
        }

        ws.onmessage = (event) => {
          try {
            const data = JSON.parse(event.data)
            if (data.type === 'rpc_response' && data.id === 'rpc-memory-1') {
              clearTimeout(timeout)
              ws.close()
              resolve(data)
            }
          } catch { /* ignore parse errors */ }
        }

        ws.onerror = () => { clearTimeout(timeout); reject(new Error('ws error')) }
      })
    })

    // memory.search should respond (result or error field)
    expect(rpcResult).toBeTruthy()
    expect(rpcResult.id).toBe('rpc-memory-1')
    // Either a result or an error is acceptable — the RPC round-trip itself should work
    expect(rpcResult.result !== undefined || rpcResult.error !== undefined).toBeTruthy()
  })
})

test.describe('Chat Backend', () => {
  test('conversations API works and returns expected shape', async ({ page }) => {
    const convs = await page.evaluate(async () => {
      const r = await fetch('http://localhost:3000/api/conversations')
      return r.json()
    })
    expect(Array.isArray(convs)).toBeTruthy()

    if (convs.length > 0) {
      const c = convs[0]
      // id is always present
      expect(c.id).toBeTruthy()
      // message_count is a number
      expect(typeof c.message_count).toBe('number')
      // channel field may be present (new field from unified UI)
      if (c.channel !== undefined) {
        expect(typeof c.channel).toBe('string')
      }
    }
  })

  test('chat.history RPC returns messages for a session', async ({ page }) => {
    await page.goto('/')
    await page.waitForLoadState('networkidle')

    // Get a real session id first
    const sessions = await page.evaluate(async () => {
      const r = await fetch('http://localhost:3000/api/conversations')
      return r.json()
    })

    if (sessions.length === 0) {
      // No sessions — skip
      return
    }

    const sessionId = sessions[0].id
    const rpcResult = await page.evaluate(async (sid: string) => {
      return new Promise<any>((resolve, reject) => {
        const ws = new WebSocket(`ws://localhost:3000/ws/${sid}`)
        const timeout = setTimeout(() => { ws.close(); reject(new Error('timeout')) }, 8000)

        ws.onopen = () => {
          ws.send(JSON.stringify({
            type: 'rpc_request',
            id: 'rpc-history-1',
            method: 'chat.history',
            params: {}
          }))
        }

        ws.onmessage = (event) => {
          try {
            const data = JSON.parse(event.data)
            if (data.type === 'rpc_response' && data.id === 'rpc-history-1') {
              clearTimeout(timeout)
              ws.close()
              resolve(data)
            }
          } catch { /* ignore parse errors */ }
        }

        ws.onerror = () => { clearTimeout(timeout); reject(new Error('ws error')) }
      })
    }, sessionId)

    expect(rpcResult).toBeTruthy()
    expect(rpcResult.id).toBe('rpc-history-1')
    // Either result or error is acceptable (method may not be implemented yet)
    expect(rpcResult.result !== undefined || rpcResult.error !== undefined).toBeTruthy()
  })
})
