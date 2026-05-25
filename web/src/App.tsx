import { useState, useEffect, useCallback, useRef } from 'react'

interface TableInfo {
  name: string
  schema: string
  sql: string
  row_count: number | null
  columns: string[]
}

interface TablesResponse {
  tables: TableInfo[]
  views: TableInfo[]
}

interface DataResponse {
  columns: string[]
  rows: string[][]
  total: number
  offset: number
  limit: number
}

type Tab = 'browse' | 'schema' | 'query' | 'export'
type NavTab = 'tables' | 'views'

const headers = (ct = 'application/json') => ({ 'Content-Type': ct })

const SQL_KEYWORDS: string[] = [
  'SELECT', 'FROM', 'WHERE', 'INSERT', 'INTO', 'VALUES', 'UPDATE', 'SET', 'DELETE',
  'CREATE', 'TABLE', 'DROP', 'ALTER', 'ADD', 'COLUMN', 'INDEX', 'VIEW', 'TRIGGER',
  'JOIN', 'LEFT', 'RIGHT', 'INNER', 'OUTER', 'CROSS', 'ON', 'AS', 'AND', 'OR', 'NOT',
  'NULL', 'IS', 'IN', 'BETWEEN', 'LIKE', 'ORDER', 'BY', 'GROUP', 'HAVING', 'LIMIT',
  'OFFSET', 'UNION', 'ALL', 'DISTINCT', 'CASE', 'WHEN', 'THEN', 'ELSE', 'END',
  'EXISTS', 'PRIMARY', 'KEY', 'FOREIGN', 'REFERENCES', 'DEFAULT', 'CHECK', 'UNIQUE',
  'IF', 'BEGIN', 'COMMIT', 'ROLLBACK', 'TRANSACTION', 'WITH', 'RECURSIVE',
  'OVER', 'PARTITION', 'ROW_NUMBER', 'RANK', 'DENSE_RANK', 'COUNT', 'SUM', 'AVG',
  'MIN', 'MAX', 'COALESCE', 'CAST', 'EXPLAIN', 'ANALYZE', 'VACUUM', 'REINDEX',
  'RETURNING', 'CONFLICT', 'NOTHING', 'BOOLEAN', 'INTEGER', 'BIGINT',
  'SERIAL', 'BIGSERIAL', 'TIMESTAMP', 'INTERVAL', 'JSONB', 'ARRAY', 'UUID',
  'TRUE', 'FALSE', 'TYPE', 'ENUM', 'SEQUENCE', 'SCHEMA', 'DATABASE',
]

function getSuggestions(input: string, tableNames: string[], viewNames: string[], columns: [string, string[]][]): string[] {
  if (!input) return []
  const lastWord = input.split(/[^a-zA-Z0-9_.]/).pop()?.toUpperCase() ?? ''
  if (!lastWord) return []
  const matches: string[] = []
  for (const kw of SQL_KEYWORDS) {
    if (kw.startsWith(lastWord)) matches.push(kw)
  }
  for (const name of tableNames) {
    if (name.toUpperCase().startsWith(lastWord)) matches.push(name)
  }
  for (const name of viewNames) {
    if (name.toUpperCase().startsWith(lastWord)) matches.push(name)
  }
  for (const [tbl, cols] of columns) {
    for (const col of cols) {
      if (col.toUpperCase().startsWith(lastWord)) {
        const entry = `${tbl}.${col}`
        if (!matches.includes(entry)) matches.push(entry)
      }
    }
  }
  return matches.slice(0, 20)
}

function highlightSQL(sql: string): JSX.Element {
  const parts: JSX.Element[] = []
  const chars = [...sql]
  let i = 0
  let key = 0

  const push = (text: string, color: string) => {
    parts.push(<span key={key++} style={{ color }}>{text}</span>)
  }

  while (i < chars.length) {
    const c = chars[i]
    if (c === "'") {
      let s = "'"
      i++
      while (i < chars.length) {
        if (chars[i] === "'") {
          s += chars[i++]
          if (i < chars.length && chars[i] === "'") { s += chars[i++] } else break
        } else { s += chars[i++] }
      }
      push(s, '#86efac')
    } else if (c === '-' && chars[i + 1] === '-') {
      push(sql.slice(i), '#52525b')
      break
    } else if (c === '/' && chars[i + 1] === '*') {
      let s = '/*'
      i += 2
      while (i < chars.length) {
        if (chars[i] === '*' && chars[i + 1] === '/') { s += '*/'; i += 2; break }
        s += chars[i++]
      }
      push(s, '#52525b')
    } else if (/[a-zA-Z_]/.test(c)) {
      let word = ''
      while (i < chars.length && /[a-zA-Z0-9_]/.test(chars[i])) { word += chars[i++] }
      if (SQL_KEYWORDS.includes(word.toUpperCase())) {
        push(word, '#7dd3fc')
      } else {
        push(word, '#d4d4d8')
      }
    } else if (/[0-9]/.test(c)) {
      let num = ''
      while (i < chars.length && /[0-9.]/.test(chars[i])) { num += chars[i++] }
      push(num, '#fbbf24')
    } else if ('=<>!|'.includes(c)) {
      let op = c
      if (i + 1 < chars.length && (chars[i + 1] === '=' || (c === '|' && chars[i + 1] === '|'))) { op += chars[++i] }
      i++
      push(op, '#6ee7b7')
    } else if ('(),;'.includes(c)) {
      push(c, '#71717a'); i++
    } else {
      push(c, '#d4d4d8'); i++
    }
  }
  return <>{parts}</>
}

function writeToClipboard(text: string): Promise<void> {
  if (navigator.clipboard && window.isSecureContext) {
    return navigator.clipboard.writeText(text)
  }
  return new Promise((resolve, reject) => {
    const ta = document.createElement('textarea')
    ta.value = text
    ta.style.position = 'fixed'
    ta.style.left = '-9999px'
    document.body.appendChild(ta)
    ta.select()
    try {
      document.execCommand('copy')
      resolve()
    } catch (e) { reject(e) }
    finally { document.body.removeChild(ta) }
  })
}

export default function App() {
  const [tables, setTables] = useState<TableInfo[]>([])
  const [views, setViews] = useState<TableInfo[]>([])
  const [navTab, setNavTab] = useState<NavTab>('tables')
  const [tab, setTab] = useState<Tab>('browse')
  const [selectedSchema, setSelectedSchema] = useState('')
  const [selectedName, setSelectedName] = useState('')
  const [data, setData] = useState<DataResponse | null>(null)
  const [sqlInput, setSqlInput] = useState('')
  const [sqlResults, setSqlResults] = useState<{ columns: string[]; rows: string[][] }[] | null>(null)
  const [selectedResultIdx, setSelectedResultIdx] = useState(0)
  const [sqlError, setSqlError] = useState('')
  const [loading, setLoading] = useState(false)
  const [searchQuery, setSearchQuery] = useState('')
  const [page, setPage] = useState(0)
  const [connError, setConnError] = useState('')
  const [dbInfo, setDbInfo] = useState<{ server: string; database: string } | null>(null)
  const [toast, setToast] = useState('')
  const toastTimer = useRef<ReturnType<typeof setTimeout>>()
  const pageSize = 500
  const queryRef = useRef<HTMLTextAreaElement>(null)

  const showToast = useCallback((msg: string) => {
    setToast(msg)
    clearTimeout(toastTimer.current)
    toastTimer.current = setTimeout(() => setToast(''), 1800)
  }, [])

  const fetchTables = useCallback(async () => {
    try {
      const infoRes = await fetch('/api/info')
      const info = await infoRes.json()
      setDbInfo(info)
      if (info.error) { setConnError(info.error); return }
      setConnError('')

      const res = await fetch('/api/tables')
      const json: TablesResponse = await res.json()
      if (json.error) { setConnError(json.error); return }
      setTables(json.tables || [])
      setViews(json.views || [])
    } catch (e: any) {
      setConnError(e.message)
    }
  }, [])

  useEffect(() => { fetchTables() }, [fetchTables])

  const selectTable = useCallback(async (schema: string, name: string) => {
    setSelectedSchema(schema)
    setSelectedName(name)
    setPage(0)
    setTab('browse')
    setLoading(true)
    try {
      const res = await fetch(`/api/table/${schema}/${name}/data?offset=0&limit=${pageSize}`)
      const json: DataResponse = await res.json()
      setData(json)
    } catch (e) {
      console.error('Failed to fetch data', e)
    }
    setLoading(false)
  }, [])

  const loadPage = useCallback(async (newPage: number) => {
    if (!selectedSchema || !selectedName) return
    setLoading(true)
    const offset = newPage * pageSize
    try {
      const res = await fetch(`/api/table/${selectedSchema}/${selectedName}/data?offset=${offset}&limit=${pageSize}`)
      const json: DataResponse = await res.json()
      setData(json)
      setPage(newPage)
    } catch (e) {
      console.error('Failed to fetch page', e)
    }
    setLoading(false)
  }, [selectedSchema, selectedName])

  const runQuery = useCallback(async () => {
    if (!sqlInput.trim()) return
    setLoading(true)
    setSqlError('')
    setSqlResults(null)
    try {
      const res = await fetch('/api/query', {
        method: 'POST',
        headers: headers(),
        body: JSON.stringify({ sql: sqlInput }),
      })
      const json = await res.json()
      if (json.error) {
        setSqlError(json.error)
      } else if (json.results) {
        setSqlResults(json.results)
        setSelectedResultIdx(0)
      }
    } catch (e: any) {
      setSqlError(e.message)
    }
    setLoading(false)
  }, [sqlInput])

  const currentList = navTab === 'tables' ? tables : views
  const filtered = searchQuery
    ? currentList.filter(t => t.name.toLowerCase().includes(searchQuery.toLowerCase()) || t.schema.toLowerCase().includes(searchQuery.toLowerCase()))
    : currentList

  const selectedTable = [...tables, ...views].find(t => t.schema === selectedSchema && t.name === selectedName)

  const allTableNames = tables.map(t => `${t.schema}.${t.name}`)
  const allViewNames = views.map(v => `${v.schema}.${v.name}`)
  const allColumns: [string, string[]][] = tables.map(t => [`${t.schema}.${t.name}`, t.columns])

  return (
    <div style={{ display: 'flex', height: '100vh', overflow: 'hidden' }}>
      {toast && (
        <div style={{ position: 'fixed', bottom: 24, left:'50%', transform:'translateX(-50%)', zIndex:999, padding:'8px 20px', borderRadius:8, background:'var(--card)', border:'1px solid var(--border)', color:'var(--cyan)', fontSize:12, fontWeight:600, boxShadow:'0 4px 16px rgba(0,0,0,.5)', pointerEvents:'none' }}>
          ✓ {toast}
        </div>
      )}
      {/* Sidebar */}
      <div style={{ width: 240, borderRight: `1px solid var(--border)`, display: 'flex', flexDirection: 'column', flexShrink: 0 }}>
        <div style={{ padding: '12px 14px', borderBottom: `1px solid var(--border)`, fontWeight: 600, fontSize: 12, color: 'var(--cyan)', letterSpacing: '.05em', textTransform: 'uppercase' }}>
          VOLTRUS
          <div style={{ fontWeight: 300, fontSize: 11, color: 'var(--cyan)', textTransform: 'none', marginTop: 1 }}>PostgresTUI</div>
          {dbInfo?.server && <div style={{ fontWeight: 400, fontSize: 10, color: 'var(--muted)', textTransform: 'none', marginTop: 2 }}>
            {dbInfo.server}/{dbInfo.database}
          </div>}
        </div>

        {connError && (
          <div style={{ padding: '8px 14px', borderBottom: `1px solid var(--border)`, fontSize: 11, color: 'var(--error)', background: 'rgba(239,68,68,.08)' }}>
            Connection failed — retrying...
            <div style={{ fontSize: 10, color: 'var(--muted)', marginTop: 2, fontFamily: "'JetBrains Mono', monospace", wordBreak: 'break-all' }}>{connError}</div>
          </div>
        )}

        {/* Nav tabs */}
        <div style={{ display: 'flex', borderBottom: `1px solid var(--border)` }}>
          {(['tables', 'views'] as NavTab[]).map(t => (
            <button
              key={t}
              onClick={() => { setNavTab(t); setSearchQuery('') }}
              style={{
                flex: 1, padding: '8px 0', fontSize: 11, fontWeight: 600, textTransform: 'uppercase',
                letterSpacing: '.05em', borderBottom: navTab === t ? '2px solid var(--cyan)' : '2px solid transparent',
                color: navTab === t ? 'var(--cyan)' : 'var(--muted)', transition: 'color .15s, border-color .15s',
              }}
            >
              {t} ({t === 'tables' ? tables.length : views.length})
            </button>
          ))}
        </div>

        {/* Search */}
        <div style={{ padding: '6px 10px', borderBottom: `1px solid var(--border)` }}>
          <input
            type="text"
            placeholder="Filter..."
            value={searchQuery}
            onChange={e => setSearchQuery(e.target.value)}
            style={{ width: '100%', fontSize: 11, padding: '4px 8px' }}
          />
        </div>

        {/* List */}
        <div style={{ flex: 1, overflowY: 'auto' }}>
          {filtered.map(t => {
            const active = t.schema === selectedSchema && t.name === selectedName
            const display = t.schema === 'public' ? t.name : `${t.schema}.${t.name}`
            const count = t.row_count?.toLocaleString() ?? '?'
            return (
              <button
                key={`${t.schema}.${t.name}`}
                onClick={() => selectTable(t.schema, t.name)}
                style={{
                  display: 'flex', justifyContent: 'space-between', alignItems: 'center',
                  width: '100%', padding: '6px 14px', textAlign: 'left',
                  background: active ? 'var(--hover-overlay)' : 'transparent',
                  borderLeft: active ? '2px solid var(--cyan)' : '2px solid transparent',
                  color: active ? 'var(--cyan)' : 'var(--text-dim)',
                  transition: 'background .1s',
                }}
                onMouseEnter={e => { if (!active) e.currentTarget.style.background = 'var(--hover-overlay)' }}
                onMouseLeave={e => { if (!active) e.currentTarget.style.background = 'transparent' }}
              >
                <span style={{ fontWeight: active ? 600 : 500, fontSize: 12, overflow: 'hidden', textOverflow: 'ellipsis', whiteSpace: 'nowrap' }}>
                  {display}
                </span>
                <span style={{ fontSize: 10, color: 'var(--muted)', fontFamily: "'JetBrains Mono', monospace", flexShrink: 0, marginLeft: 8 }}>
                  {count}
                </span>
              </button>
            )
          })}
        </div>
      </div>

      {/* Main content */}
      <div style={{ flex: 1, display: 'flex', flexDirection: 'column', overflow: 'hidden' }}>
        {selectedTable ? (
          <>
            {/* Content tabs */}
            <div style={{ display: 'flex', borderBottom: `1px solid var(--border)`, padding: '0 16px' }}>
              {([
                ['browse', 'Browse', 'L'],
                ['schema', 'Schema', 'H'],
                ['query', 'Query', ';'],
                ['export', 'Export', 'X'],
              ] as [Tab, string, string][]).map(([key, label, hint]) => (
                <button
                  key={key}
                  onClick={() => setTab(key)}
                  style={{
                    padding: '10px 18px', fontSize: 12, fontWeight: 600, textTransform: 'uppercase',
                    letterSpacing: '.05em',
                    borderBottom: tab === key ? '2px solid var(--cyan)' : '2px solid transparent',
                    color: tab === key ? 'var(--cyan)' : 'var(--muted)',
                    transition: 'color .15s, border-color .15s',
                  }}
                >
                  {label} <span style={{ fontSize: 10, opacity: .5 }}>[{hint}]</span>
                </button>
              ))}
            </div>

            {/* Tab content */}
            <div style={{ flex: 1, overflow: 'auto', padding: 16 }}>
              {tab === 'browse' && <BrowseTab data={data} loading={loading} page={page} pageSize={pageSize} onPage={loadPage} selectedTable={selectedTable} showToast={showToast} />}
              {tab === 'schema' && <SchemaTab table={selectedTable} />}
              {tab === 'query' && (
                <QueryTab
                  sql={sqlInput} setSql={setSqlInput}
                  results={sqlResults} selectedResultIdx={selectedResultIdx} setSelectedResultIdx={setSelectedResultIdx}
                  error={sqlError}
                  loading={loading} onRun={runQuery} queryRef={queryRef}
                  tableNames={allTableNames} viewNames={allViewNames} columns={allColumns}
                  showToast={showToast}
                />
              )}
              {tab === 'export' && <ExportTab schema={selectedSchema} name={selectedName} table={selectedTable} />}
            </div>
          </>
        ) : (
          <div style={{ flex: 1, display: 'flex', alignItems: 'center', justifyContent: 'center', color: 'var(--muted)' }}>
            Select a table from the sidebar
          </div>
        )}
      </div>
    </div>
  )
}

function BrowseTab({ data, loading, page, pageSize, onPage, selectedTable, showToast }: {
  data: DataResponse | null
  loading: boolean
  page: number
  pageSize: number
  onPage: (p: number) => void
  selectedTable: TableInfo
  showToast: (msg: string) => void
}) {
  if (loading && !data) return <div style={{ color: 'var(--muted)', padding: 20 }}>Loading...</div>
  if (!data) return null

  const total = data.total
  const hasNext = (page + 1) * pageSize < total
  const hasPrev = page > 0
  const rowStart = total > 0 ? page * pageSize + 1 : 0
  const rowEnd = page * pageSize + data.rows.length

  return (
    <div style={{ height: '100%', display: 'flex', flexDirection: 'column' }}>
      <div style={{ marginBottom: 12, display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
        <div>
          <span style={{ fontWeight: 700, fontSize: 14, color: 'var(--cyan)' }}>
            {selectedTable.schema === 'public' ? selectedTable.name : `${selectedTable.schema}.${selectedTable.name}`}
          </span>
          <span style={{ color: 'var(--muted)', marginLeft: 12, fontSize: 12 }}>
            {rowStart}–{rowEnd} of {total.toLocaleString()} rows · {data.columns.length} cols
          </span>
        </div>
        <div style={{ display: 'flex', gap: 6 }}>
          <button onClick={() => onPage(page - 1)} disabled={!hasPrev} style={{ ...btnStyle, opacity: hasPrev ? 1 : 0.3 }}>← Prev</button>
          <button onClick={() => onPage(page + 1)} disabled={!hasNext} style={{ ...btnStyle, opacity: hasNext ? 1 : 0.3 }}>Next →</button>
          <button onClick={() => copyData(data, 'tsv', showToast)} style={btnStyle}>Copy TSV</button>
          <button onClick={() => copyData(data, 'json', showToast)} style={btnStyle}>Copy JSON</button>
        </div>
      </div>

      <div style={{ flex: 1, overflow: 'auto', border: `1px solid var(--border)`, borderRadius: 6 }}>
        <table style={{ borderCollapse: 'collapse', width: 'max-content', minWidth: '100%' }}>
          <thead>
            <tr>
              <th style={thStyle}>#</th>
              {data.columns.map((col, i) => (
                <th key={i} style={thStyle}>{col}</th>
              ))}
            </tr>
          </thead>
          <tbody>
            {data.rows.map((row, ri) => (
              <tr key={ri} style={{ background: ri % 2 !== 0 ? 'rgba(255,255,255,.02)' : 'transparent' }}>
                <td style={{ ...tdStyle, color: 'var(--muted)', fontFamily: "'JetBrains Mono', monospace", fontSize: 11 }}>
                  {rowStart + ri}
                </td>
                {row.map((cell, ci) => (
                  <td key={ci} style={{
                    ...tdStyle,
                    fontFamily: "'JetBrains Mono', monospace",
                    fontSize: 12,
                    color: cell === 'null' ? 'var(--muted)' : cell === '[Blob]' ? 'var(--muted)' : 'var(--text)',
                    fontStyle: cell === 'null' ? 'italic' : 'normal',
                    maxWidth: 400,
                    overflow: 'hidden',
                    textOverflow: 'ellipsis',
                    whiteSpace: 'nowrap',
                  }}>
                    {cell}
                  </td>
                ))}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  )
}

function SchemaTab({ table }: { table: TableInfo }) {
  return (
    <div>
      <div style={{ marginBottom: 12 }}>
        <span style={{ fontWeight: 700, fontSize: 14, color: 'var(--cyan)' }}>
          {table.schema === 'public' ? table.name : `${table.schema}.${table.name}`}
        </span>
        <span style={{ color: 'var(--muted)', marginLeft: 12, fontSize: 12 }}>
          {table.columns.length} columns · {(table.row_count ?? 0).toLocaleString()} rows
        </span>
      </div>

      {table.sql ? (
        <pre style={{
          background: 'var(--card)', border: `1px solid var(--border)`, borderRadius: 6,
          padding: 16, fontFamily: "'JetBrains Mono', monospace", fontSize: 12, lineHeight: 1.6,
          color: 'var(--text-dim)', overflow: 'auto', whiteSpace: 'pre-wrap',
        }}>
          {table.sql}
        </pre>
      ) : (
        <div style={{ color: 'var(--muted)', fontStyle: 'italic' }}>(schema definition not available)</div>
      )}

      <div style={{ marginTop: 16 }}>
        <div style={{ fontWeight: 600, fontSize: 12, marginBottom: 8, color: 'var(--text-dim)' }}>Columns</div>
        <div style={{ display: 'flex', flexWrap: 'wrap', gap: 6 }}>
          {table.columns.map((col, i) => (
            <span key={i} style={{
              padding: '3px 8px', borderRadius: 4, fontSize: 11,
              fontFamily: "'JetBrains Mono', monospace",
              background: 'var(--card)', border: '1px solid var(--border)',
              color: 'var(--text-dim)',
            }}>
              {col}
            </span>
          ))}
        </div>
      </div>
    </div>
  )
}

function QueryTab({ sql, setSql, results, selectedResultIdx, setSelectedResultIdx, error, loading, onRun, queryRef, tableNames, viewNames, columns, showToast }: {
  sql: string
  setSql: (s: string) => void
  results: { columns: string[]; rows: string[][] }[] | null
  selectedResultIdx: number
  setSelectedResultIdx: (n: number) => void
  error: string
  loading: boolean
  onRun: () => void
  queryRef: React.RefObject<HTMLTextAreaElement | null>
  tableNames: string[]
  viewNames: string[]
  columns: [string, string[]][]
  showToast: (msg: string) => void
}) {
  const [suggestions, setSuggestions] = useState<string[]>([])
  const [sugIdx, setSugIdx] = useState(-1)
  const sugRef = useRef<HTMLDivElement>(null)
  const highlightRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (queryRef.current) {
      queryRef.current.style.height = 'auto'
      queryRef.current.style.height = queryRef.current.scrollHeight + 'px'
    }
  }, [sql, queryRef])

  const syncScroll = () => {
    if (highlightRef.current && queryRef.current) {
      highlightRef.current.scrollTop = queryRef.current.scrollTop
      highlightRef.current.scrollLeft = queryRef.current.scrollLeft
    }
  }

  const updateSuggestions = useCallback((val: string) => {
    const sugs = getSuggestions(val, tableNames, viewNames, columns)
    setSuggestions(sugs)
    setSugIdx(sugs.length > 0 ? 0 : -1)
  }, [tableNames, viewNames, columns])

  const applySuggestion = useCallback((sug?: string) => {
    if (suggestions.length === 0) return
    const idx = sugIdx >= 0 ? sugIdx : 0
    const chosen = sug ?? suggestions[idx]
    if (!chosen) return
    const parts = sql.split(/([^\w.])/)
    const last = parts[parts.length - 1] || ''
    const newSql = sql.slice(0, sql.length - last.length) + chosen
    setSql(newSql)
    setSuggestions([])
    setSugIdx(-1)
  }, [sql, suggestions, sugIdx, setSql])

  const handleKey = (e: React.KeyboardEvent) => {
    if (e.key === 's' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault()
      setSuggestions([])
      onRun()
      return
    }
    if (suggestions.length > 0) {
      if (e.key === 'ArrowDown') {
        e.preventDefault()
        setSugIdx(i => Math.min(i + 1, suggestions.length - 1))
        return
      }
      if (e.key === 'ArrowUp') {
        e.preventDefault()
        setSugIdx(i => Math.max(i - 1, 0))
        return
      }
      if (e.key === 'Tab' || e.key === 'Enter') {
        e.preventDefault()
        applySuggestion()
        return
      }
      if (e.key === 'Escape') {
        setSuggestions([])
        return
      }
    }
  }

  const handleChange = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const val = e.target.value
    setSql(val)
    updateSuggestions(val)
  }

  const result = results?.[selectedResultIdx] ?? null

  return (
    <div style={{ height: '100%', display: 'flex', flexDirection: 'column', gap: 12 }}>
      <div style={{ display: 'flex', gap: 8, alignItems: 'flex-start' }}>
        <div style={{ flex: 1, position: 'relative' }}>
          {/* Highlight overlay */}
          <div
            ref={highlightRef}
            aria-hidden="true"
            style={{
              position: 'absolute', top: 0, left: 0, right: 0, bottom: 0,
              padding: 10,
              fontFamily: "'JetBrains Mono', monospace", fontSize: 12, lineHeight: 1.5,
              whiteSpace: 'pre-wrap', wordWrap: 'break-word',
              overflow: 'hidden', pointerEvents: 'none',
              color: 'transparent',
              border: '1px solid transparent', borderRadius: 6,
              minHeight: 80,
            }}
          >
            {sql ? highlightSQL(sql) : <span style={{ color: '#52525b' }}>SELECT * FROM ...</span>}
          </div>
          <textarea
            ref={queryRef}
            value={sql}
            onChange={handleChange}
            onKeyDown={handleKey}
            onScroll={syncScroll}
            onBlur={() => { setTimeout(() => setSuggestions([]), 150) }}
            placeholder=""
            spellCheck={false}
            autoCapitalize="off"
            autoCorrect="off"
            style={{
              width: '100%', minHeight: 80, resize: 'vertical', overflow: 'auto',
              fontFamily: "'JetBrains Mono', monospace", fontSize: 12, lineHeight: 1.5,
              padding: 10,
              background: 'transparent', color: 'transparent', caretColor: '#6ee7b7',
              border: '1px solid var(--border)', borderRadius: 6,
              position: 'relative', zIndex: 1,
            }}
          />
            {/* Suggestion popup */}
            {suggestions.length > 0 && (
              <div ref={sugRef} style={{
                position: 'absolute', top: '100%', left: 0, zIndex: 50,
                background: '#18181b', border: '1px solid var(--border)', borderRadius: 6,
                maxHeight: 180, overflowY: 'auto', minWidth: 200,
                boxShadow: '0 4px 12px rgba(0,0,0,.4)',
              }}>
                {suggestions.map((s, i) => (
                  <div
                    key={s}
                    onMouseDown={e => { e.preventDefault(); applySuggestion(s) }}
                    style={{
                      padding: '4px 10px', fontSize: 12,
                      fontFamily: "'JetBrains Mono', monospace",
                      color: i === sugIdx ? 'var(--cyan)' : 'var(--text-dim)',
                      background: i === sugIdx ? 'var(--hover-overlay)' : 'transparent',
                      cursor: 'pointer',
                    }}
                  >
                    {s}
                  </div>
                ))}
              </div>
            )}
          </div>
          <div style={{ display: 'flex', gap: 6, alignItems: 'flex-start' }}>
            <label style={{ ...btnStyle, cursor: 'pointer', padding: '8px 12px', whiteSpace: 'nowrap' }}>
              Load File
              <input
                type="file"
                accept=".sql"
                style={{ display: 'none' }}
                onChange={async (e) => {
                  const file = e.target.files?.[0]
                  if (!file) return
                  setSql(await file.text())
                  e.target.value = ''
                }}
              />
            </label>
            <button
              onClick={onRun}
              disabled={loading}
              style={{ ...btnStyle, background: 'var(--cyan)', color: '#000', fontWeight: 700, padding: '8px 16px' }}
            >
              {loading ? '...' : 'Run'} <span style={{ fontSize: 10, opacity: .7 }}>Ctrl+S</span>
            </button>
          </div>
        </div>

      {error && (
        <div style={{ padding: 12, background: 'rgba(239,68,68,.1)', border: '1px solid rgba(239,68,68,.3)', borderRadius: 6, color: 'var(--error)', fontFamily: "'JetBrains Mono', monospace", fontSize: 12, whiteSpace: 'pre-wrap' }}>
          {error}
        </div>
      )}

      {/* Result tabs */}
      {results && results.length > 1 && (
        <div style={{ display: 'flex', gap: 4, borderBottom: `1px solid var(--border)`, paddingBottom: 4 }}>
          {results.map((r, i) => (
            <button
              key={i}
              onClick={() => setSelectedResultIdx(i)}
              style={{
                padding: '4px 12px', fontSize: 11, fontWeight: 600,
                borderRadius: 4,
                border: 'none',
                background: i === selectedResultIdx ? 'var(--hover-overlay)' : 'transparent',
                color: i === selectedResultIdx ? 'var(--cyan)' : 'var(--muted)',
                cursor: 'pointer',
              }}
            >
              Result {i + 1}
              <span style={{ marginLeft: 6, fontWeight: 400, color: 'var(--text-dim)' }}>
                {r.rows.length}×{r.columns.length}
              </span>
            </button>
          ))}
        </div>
      )}

      {/* Result table */}
      {results && results.length > 0 && result && result.columns.length === 0 && result.rows.length === 0 ? (
        <div style={{ padding: 24, textAlign: 'center', border: `1px solid var(--border)`, borderRadius: 6, color: 'var(--muted)' }}>
          Script executed successfully.
        </div>
      ) : result && (
        <div style={{ flex: 1, overflow: 'auto', border: `1px solid var(--border)`, borderRadius: 6 }}>
          <div style={{ padding: '8px 12px', borderBottom: `1px solid var(--border)`, fontSize: 12, color: 'var(--muted)', display: 'flex', justifyContent: 'space-between', alignItems: 'center' }}>
            <span>{result.rows.length} rows · {result.columns.length} cols</span>
            <div style={{ display: 'flex', gap: 6 }}>
              <button onClick={() => copyResult(result, 'tsv', showToast)} style={smallBtnStyle}>Copy TSV</button>
              <button onClick={() => copyResult(result, 'json', showToast)} style={smallBtnStyle}>Copy JSON</button>
              <span style={{ borderLeft: '1px solid var(--border)', margin: '0 2px' }} />
              <button onClick={() => downloadResult(result, 'csv')} style={smallBtnStyle}>CSV</button>
              <button onClick={() => downloadResult(result, 'json')} style={smallBtnStyle}>JSON</button>
              <button onClick={() => downloadResult(result, 'sql')} style={smallBtnStyle}>SQL</button>
            </div>
          </div>
          <table style={{ borderCollapse: 'collapse', width: 'max-content', minWidth: '100%' }}>
            <thead>
              <tr>
                <th style={thStyle}>#</th>
                {result.columns.map((col, i) => (
                  <th key={i} style={thStyle}>{col}</th>
                ))}
              </tr>
            </thead>
            <tbody>
              {result.rows.map((row, ri) => (
                <tr key={ri} style={{ background: ri % 2 !== 0 ? 'rgba(255,255,255,.02)' : 'transparent' }}>
                  <td style={{ ...tdStyle, color: 'var(--muted)', fontFamily: "'JetBrains Mono', monospace", fontSize: 11 }}>
                    {ri + 1}
                  </td>
                  {row.map((cell, ci) => (
                    <td key={ci} style={{
                      ...tdStyle,
                      fontFamily: "'JetBrains Mono', monospace",
                      fontSize: 12,
                      color: cell === 'null' ? 'var(--muted)' : 'var(--text)',
                      fontStyle: cell === 'null' ? 'italic' : 'normal',
                    }}>
                      {cell}
                    </td>
                  ))}
                </tr>
              ))}
            </tbody>
          </table>
        </div>
      )}
    </div>
  )
}

function downloadResult(result: { columns: string[]; rows: string[][] }, format: string) {
  let content: string
  let mime: string
  let ext: string

  if (format === 'csv') {
    const lines = [result.columns.map(c => csvEscape(c)).join(',')]
    for (const row of result.rows) {
      lines.push(row.map(c => csvEscape(c)).join(','))
    }
    content = lines.join('\n')
    mime = 'text/csv'
    ext = 'csv'
  } else if (format === 'json') {
    const arr = result.rows.map(row => {
      const obj: Record<string, string | null> = {}
      result.columns.forEach((col, i) => { obj[col] = row[i] === 'null' ? null : row[i] })
      return obj
    })
    content = JSON.stringify(arr, null, 2)
    mime = 'application/json'
    ext = 'json'
  } else {
    const lines: string[] = []
    const cols = result.columns.map(c => `"${c}"`).join(', ')
    for (const row of result.rows) {
      const vals = row.map(v => v === 'null' ? 'NULL' : `'${v.replace(/'/g, "''")}'`).join(', ')
      lines.push(`INSERT INTO result (${cols}) VALUES (${vals});`)
    }
    content = lines.join('\n')
    mime = 'text/plain'
    ext = 'sql'
  }

  const blob = new Blob([content], { type: mime })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = `query_result.${ext}`
  document.body.appendChild(a)
  a.click()
  document.body.removeChild(a)
  URL.revokeObjectURL(url)
}

function csvEscape(val: string): string {
  if (val.includes(',') || val.includes('"') || val.includes('\n')) {
    return `"${val.replace(/"/g, '""')}"`
  }
  return val
}

function ExportTab({ schema, name, table }: { schema: string; name: string; table: TableInfo }) {
  const total = table.row_count ?? 0
  const [exporting, setExporting] = useState(false)
  const [pageExport, setPageExport] = useState<'all' | 'page'>('all')
  const display = schema === 'public' ? name : `${schema}.${name}`

  const download = async (format: string) => {
    setExporting(true)
    try {
      let url = `/api/table/${schema}/${name}/export/${format}`
      if (pageExport === 'page') {
        url += `?limit=500`
      }
      const a = document.createElement('a')
      a.href = url
      a.download = ''
      document.body.appendChild(a)
      a.click()
      document.body.removeChild(a)
    } catch (e) {
      console.error('Export failed', e)
    }
    setExporting(false)
  }

  return (
    <div>
      <div style={{ marginBottom: 16 }}>
        <span style={{ fontWeight: 700, fontSize: 14, color: 'var(--cyan)' }}>{display}</span>
        <span style={{ color: 'var(--muted)', marginLeft: 12, fontSize: 12 }}>{total.toLocaleString()} rows</span>
      </div>

      <div style={{ marginBottom: 16, display: 'flex', gap: 8, alignItems: 'center' }}>
        <span style={{ fontSize: 12, color: 'var(--text-dim)' }}>Scope:</span>
        <button
          onClick={() => setPageExport('all')}
          style={{ ...btnStyle, borderBottom: pageExport === 'all' ? '2px solid var(--cyan)' : '2px solid transparent', color: pageExport === 'all' ? 'var(--cyan)' : 'var(--muted)' }}
        >
          All rows ({total.toLocaleString()})
        </button>
        <button
          onClick={() => setPageExport('page')}
          style={{ ...btnStyle, borderBottom: pageExport === 'page' ? '2px solid var(--cyan)' : '2px solid transparent', color: pageExport === 'page' ? 'var(--cyan)' : 'var(--muted)' }}
        >
          First 500 rows
        </button>
      </div>

      <div style={{ display: 'grid', gridTemplateColumns: 'repeat(3, 1fr)', gap: 12, maxWidth: 600 }}>
        <button onClick={() => download('csv')} disabled={exporting} style={exportCardStyle}>
          <div style={{ fontWeight: 700, fontSize: 14, color: 'var(--cyan)', marginBottom: 4 }}>CSV</div>
          <div style={{ fontSize: 11, color: 'var(--muted)' }}>Comma-separated values.</div>
        </button>
        <button onClick={() => download('json')} disabled={exporting} style={exportCardStyle}>
          <div style={{ fontWeight: 700, fontSize: 14, color: 'var(--cyan)', marginBottom: 4 }}>JSON</div>
          <div style={{ fontSize: 11, color: 'var(--muted)' }}>Array of objects with column keys.</div>
        </button>
        <button onClick={() => download('sql')} disabled={exporting} style={exportCardStyle}>
          <div style={{ fontWeight: 700, fontSize: 14, color: 'var(--cyan)', marginBottom: 4 }}>SQL</div>
          <div style={{ fontSize: 11, color: 'var(--muted)' }}>INSERT INTO statements.</div>
        </button>
      </div>

      {exporting && <div style={{ marginTop: 12, color: 'var(--muted)', fontSize: 12 }}>Downloading...</div>}
    </div>
  )
}

async function copyData(data: DataResponse | null, format: 'tsv' | 'json', onDone?: (msg: string) => void) {
  if (!data) return
  if (format === 'tsv') {
    const tsv = [data.columns.join('\t'), ...data.rows.map(r => r.join('\t'))].join('\n')
    await writeToClipboard(tsv)
  } else {
    const arr = data.rows.map(row => {
      const obj: Record<string, string> = {}
      data.columns.forEach((col, i) => { obj[col] = row[i] ?? 'null' })
      return obj
    })
    await writeToClipboard(JSON.stringify(arr, null, 2))
  }
  onDone?.('Copied to clipboard')
}

async function copyResult(result: { columns: string[]; rows: string[][] }, format: 'tsv' | 'json', onDone?: (msg: string) => void) {
  if (format === 'tsv') {
    const tsv = [result.columns.join('\t'), ...result.rows.map(r => r.join('\t'))].join('\n')
    await writeToClipboard(tsv)
  } else {
    const arr = result.rows.map(row => {
      const obj: Record<string, string> = {}
      result.columns.forEach((col, i) => { obj[col] = row[i] ?? 'null' })
      return obj
    })
    await writeToClipboard(JSON.stringify(arr, null, 2))
  }
  onDone?.('Copied to clipboard')
}

const thStyle: React.CSSProperties = {
  padding: '8px 12px', textAlign: 'left', fontWeight: 600, fontSize: 11,
  textTransform: 'uppercase' as const, letterSpacing: '.03em',
  color: 'var(--muted)', borderBottom: '1px solid var(--border)',
  background: 'var(--card)', position: 'sticky' as const, top: 0, zIndex: 1,
}

const tdStyle: React.CSSProperties = {
  padding: '5px 12px', borderBottom: '1px solid var(--border)', whiteSpace: 'nowrap' as const,
}

const smallBtnStyle: React.CSSProperties = {
  fontSize: 11, fontWeight: 500, padding: '3px 8px', borderRadius: 4,
  border: '1px solid var(--border)', background: 'var(--card)', color: 'var(--muted)',
  cursor: 'pointer', transition: 'color .15s',
}

const exportCardStyle: React.CSSProperties = {
  padding: 20, borderRadius: 8, border: '1px solid var(--border)',
  background: 'var(--card)', cursor: 'pointer', textAlign: 'left' as const,
  transition: 'border-color .15s',
}

const btnStyle: React.CSSProperties = {
  fontSize: 12, fontWeight: 600, padding: '6px 12px', borderRadius: 6,
  border: '1px solid var(--border)', background: 'var(--card)', color: 'var(--fg)',
  cursor: 'pointer', display: 'flex', alignItems: 'center', gap: 5,
  transition: 'border-color .15s, color .15s',
}
