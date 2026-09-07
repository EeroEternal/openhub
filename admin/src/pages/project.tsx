/* eslint-disable react-hooks/set-state-in-effect */
import { useEffect, useMemo, useState, type FormEvent } from "react"
import { useParams, useSearchParams } from "react-router-dom"
import { toast } from "sonner"
import { PageContainer } from "@/components/layout/page-container"
import { PageHeader } from "@/components/layout/page-header"
import { PageShell } from "@/components/layout/page-shell"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import hljs from "highlight.js"
import "highlight.js/styles/github.css"
import ReactMarkdown from "react-markdown"
import remarkGfm from "remark-gfm"
import {
  ChevronRight,
  ChevronDown,
  Folder,
  FolderOpen,
  FileText,
  FileCode,
  FileSpreadsheet,
  File,
  Terminal,
  Copy,
  Check,
  Bot,
  MessageSquare,
  Wrench,
  Sparkles,
  ChevronUp,
} from "lucide-react"
import { Card } from "@/components/ui/card"
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Select } from "@/components/ui/select"
import { Textarea } from "@/components/ui/textarea"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { api, getToken } from "@/lib/api"
import { t, useI18n } from "@/lib/i18n"
import { cn } from "@/lib/utils"

type Tab = "code" | "commits" | "sessions" | "settings"
type Project = { id: string; name: string; slug: string; owner_username?: string; full_name?: string }
type TreeEntry = { path: string; kind: string; size?: number }
type CommitRow = { sha: string; date: string; message: string }
type SessionEvent = {
  id: string
  event_type: string
  payload: Record<string, unknown>
  created_at: string
}

const TABS: Tab[] = ["code", "commits", "sessions", "settings"]

function isTab(value: string | null): value is Tab {
  return TABS.includes(value as Tab)
}

async function copyText(value: string) {
  await navigator.clipboard.writeText(value)
  toast.success(t("common.copied"))
}

function CopyableCode({ text, className }: { text: string; className?: string }) {
  const [copied, setCopied] = useState(false)

  async function handleCopy() {
    await copyText(text)
    setCopied(true)
    setTimeout(() => setCopied(false), 2000)
  }

  return (
    <div className={cn("group relative flex items-center justify-between rounded-md border bg-muted/40 px-3 py-2 font-mono text-xs text-foreground transition-colors hover:border-border/80 hover:bg-muted/60", className)}>
      <span className="truncate select-all pr-8">{text}</span>
      <button
        type="button"
        onClick={handleCopy}
        className="absolute right-2 flex h-6 w-6 items-center justify-center rounded text-muted-foreground opacity-0 transition-opacity hover:bg-muted hover:text-foreground group-hover:opacity-100"
        title={t("project.copy")}
      >
        {copied ? <Check className="h-3.5 w-3.5 text-success" /> : <Copy className="h-3.5 w-3.5" />}
      </button>
    </div>
  )
}

function parseLog(log: string): CommitRow[] {
  return log
    .split("\n")
    .map((line) => line.trim())
    .filter(Boolean)
    .map((line) => {
      const parts = line.split(/\s+/)
      const sha = parts[0] ?? ""
      const date = parts[1] ?? ""
      const message = parts.slice(2).join(" ")
      return { sha, date, message }
    })
    .filter((row) => row.sha)
}

function getLanguage(path: string): string {
  const ext = path.split(".").pop()?.toLowerCase() ?? ""
  const map: Record<string, string> = {
    rs: "rust",
    ts: "typescript",
    tsx: "typescript",
    js: "javascript",
    jsx: "javascript",
    json: "json",
    py: "python",
    sh: "bash",
    bash: "bash",
    zsh: "bash",
    yaml: "yaml",
    yml: "yaml",
    toml: "toml",
    sql: "sql",
    html: "html",
    css: "css",
    md: "markdown",
    markdown: "markdown",
  }
  return map[ext] || "plaintext"
}

interface TreeNode {
  name: string
  path: string
  kind: "dir" | "file"
  children: TreeNode[]
}

function buildFileTree(entries: TreeEntry[]): TreeNode[] {
  type InternalNode = {
    name: string
    path: string
    kind: "dir" | "file"
    children: Map<string, InternalNode>
  }

  const root: InternalNode = {
    name: "",
    path: "",
    kind: "dir",
    children: new Map(),
  }

  for (const entry of entries) {
    const parts = entry.path.split("/").filter(Boolean)
    let curr = root
    for (let i = 0; i < parts.length; i++) {
      const part = parts[i]
      const isLast = i === parts.length - 1
      const subPath = parts.slice(0, i + 1).join("/")
      if (!curr.children.has(part)) {
        curr.children.set(part, {
          name: part,
          path: subPath,
          kind: isLast ? (entry.kind === "dir" ? "dir" : "file") : "dir",
          children: new Map(),
        })
      }
      curr = curr.children.get(part)!
    }
  }

  function toTreeNodes(node: InternalNode): TreeNode[] {
    const list: TreeNode[] = []
    for (const child of node.children.values()) {
      list.push({
        name: child.name,
        path: child.path,
        kind: child.kind,
        children: toTreeNodes(child),
      })
    }
    // Sort: directories first, then files alphabetically
    return list.sort((a, b) => {
      if (a.kind === b.kind) return a.name.localeCompare(b.name)
      return a.kind === "dir" ? -1 : 1
    })
  }

  return toTreeNodes(root)
}

function getFileIcon(path: string) {
  const ext = path.split(".").pop()?.toLowerCase() ?? ""
  if (["md", "markdown", "txt", "doc"].includes(ext)) {
    return <FileText className="size-4 shrink-0 text-muted-foreground" />
  }
  if (["rs", "ts", "tsx", "js", "jsx", "py", "sh", "sql", "html", "css"].includes(ext)) {
    return <FileCode className="size-4 shrink-0 text-muted-foreground" />
  }
  if (["json", "yaml", "yml", "toml", "xml", "csv"].includes(ext)) {
    return <FileSpreadsheet className="size-4 shrink-0 text-muted-foreground" />
  }
  return <File className="size-4 shrink-0 text-muted-foreground" />
}

interface TreeNodeItemProps {
  node: TreeNode
  depth?: number
  selected: string | null
  expandedDirs: Set<string>
  onToggleDir: (path: string) => void
  onSelectFile: (path: string) => void
}

function TreeNodeItem({
  node,
  depth = 0,
  selected,
  expandedDirs,
  onToggleDir,
  onSelectFile,
}: TreeNodeItemProps) {
  if (node.kind === "dir") {
    const isExpanded = expandedDirs.has(node.path)
    return (
      <li>
        <button
          type="button"
          className="flex w-full items-center gap-1.5 rounded-md py-1.5 px-2 text-left text-sm text-foreground/80 hover:bg-muted/60 transition-colors"
          style={{ paddingLeft: 8 + depth * 14 }}
          onClick={() => onToggleDir(node.path)}
        >
          {isExpanded ? (
            <ChevronDown className="size-3.5 shrink-0 text-muted-foreground" />
          ) : (
            <ChevronRight className="size-3.5 shrink-0 text-muted-foreground" />
          )}
          {isExpanded ? (
            <FolderOpen className="size-4 shrink-0 text-amber-500/90" />
          ) : (
            <Folder className="size-4 shrink-0 text-amber-500/90" />
          )}
          <span className="truncate font-medium text-xs text-foreground/90">{node.name}</span>
        </button>
        {isExpanded && node.children.length > 0 && (
          <ul className="space-y-0.5">
            {node.children.map((child) => (
              <TreeNodeItem
                key={child.path}
                node={child}
                depth={depth + 1}
                selected={selected}
                expandedDirs={expandedDirs}
                onToggleDir={onToggleDir}
                onSelectFile={onSelectFile}
              />
            ))}
          </ul>
        )}
      </li>
    )
  }

  const isSelected = selected === node.path
  return (
    <li>
      <button
        type="button"
        className={cn(
          "flex w-full items-center gap-2 rounded-md py-1.5 px-2 text-left text-sm transition-colors",
          isSelected
            ? "bg-primary/10 font-medium text-primary"
            : "text-foreground/80 hover:bg-muted/60"
        )}
        style={{ paddingLeft: 8 + depth * 14 + 18 }}
        onClick={() => onSelectFile(node.path)}
      >
        {getFileIcon(node.path)}
        <span className="truncate text-xs">{node.name}</span>
      </button>
    </li>
  )
}

interface ParsedToolItem {
  id: string
  name: string
  args?: string
  result?: string
  isError?: boolean
}

interface SessionTurn {
  id: string
  userPrompt?: {
    id: string
    text: string
    createdAt: string
    commitSha?: string
  }
  agentResponses: Array<{
    id: string
    text?: string
    thinking?: string
    createdAt: string
    commitSha?: string
  }>
  tools: ParsedToolItem[]
  timestamp: string
  commitSha?: string
}

function SessionTurnCard({
  turn,
  index,
  total,
  defaultExpanded,
}: {
  turn: SessionTurn
  index: number
  total: number
  defaultExpanded: boolean
}) {
  const [turnOpen, setTurnOpen] = useState(defaultExpanded)
  const [toolsOpen, setToolsOpen] = useState(false)
  const [expandedTexts, setExpandedTexts] = useState<Record<string, boolean>>({})

  // Update open state if parent forces expand/collapse
  useEffect(() => {
    setTurnOpen(defaultExpanded)
  }, [defaultExpanded])

  const toggleTextExpand = (id: string) => {
    setExpandedTexts((prev) => ({ ...prev, [id]: !prev[id] }))
  }

  const userText = turn.userPrompt?.text || ""
  const firstLinePrompt = userText.trim().split("\n")[0] || ""
  const displayTitle = firstLinePrompt.length > 80 ? `${firstLinePrompt.slice(0, 80)}...` : firstLinePrompt

  return (
    <Card className="p-0 overflow-hidden border border-border shadow-xs transition-shadow">
      {/* Turn Header / Accordion trigger */}
      <div
        className={cn(
          "flex cursor-pointer items-center justify-between gap-3 p-4 select-none transition-colors",
          turnOpen ? "bg-muted/40 border-b border-border" : "hover:bg-muted/30"
        )}
        onClick={() => setTurnOpen(!turnOpen)}
      >
        <div className="flex min-w-0 flex-1 items-center gap-2.5">
          <button
            type="button"
            className="flex size-6 shrink-0 items-center justify-center rounded-md border border-border/80 bg-background text-muted-foreground transition-colors hover:text-foreground"
            onClick={(e) => {
              e.stopPropagation()
              setTurnOpen(!turnOpen)
            }}
          >
            {turnOpen ? <ChevronDown className="size-3.5" /> : <ChevronRight className="size-3.5" />}
          </button>

          <span className="inline-flex shrink-0 items-center gap-1 rounded-md bg-primary/10 px-2 py-0.5 text-xs font-semibold text-primary border border-primary/20">
            <MessageSquare className="size-3" />
            {t("project.turn", { number: total - index })}
          </span>

          <span className="truncate font-medium text-xs text-foreground/90">
            {displayTitle || t("project.userPrompt")}
          </span>
        </div>

        <div className="flex shrink-0 items-center gap-2 text-xs text-muted-foreground">
          {turn.tools.length > 0 && (
            <span className="hidden sm:inline-flex items-center gap-1 rounded-md bg-muted px-2 py-0.5 text-[11px] font-mono text-muted-foreground border border-border/60">
              <Wrench className="size-3 text-primary/80" />
              {turn.tools.length}
            </span>
          )}
          {turn.commitSha && (
            <span className="inline-flex items-center font-mono bg-muted/60 px-1.5 py-0.5 rounded text-[10px] text-foreground/80">
              {turn.commitSha.slice(0, 7)}
            </span>
          )}
          <span className="text-[11px] font-mono">{new Date(turn.timestamp).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</span>
        </div>
      </div>

      {/* Turn Body */}
      {turnOpen && (
        <div className="p-4 space-y-4 bg-background/50">
          {/* User Prompt */}
          {turn.userPrompt && (
            <div className="rounded-md border border-primary/20 bg-primary/[0.03] p-3.5 space-y-2">
              <div className="flex items-center justify-between text-xs text-muted-foreground pb-1 border-b border-primary/10">
                <span className="font-semibold text-primary inline-flex items-center gap-1">
                  <MessageSquare className="size-3.5" />
                  {t("project.userPrompt")}
                </span>
                <span className="text-[11px]">{new Date(turn.userPrompt.createdAt).toLocaleString()}</span>
              </div>
              <div className="text-xs leading-relaxed text-foreground whitespace-pre-wrap font-sans">
                {userText}
              </div>
            </div>
          )}

          {/* Tools Pipeline summary & details */}
          {turn.tools.length > 0 && (
            <div className="rounded-md border border-border/80 bg-muted/20 text-xs overflow-hidden">
              <div
                className="flex cursor-pointer items-center justify-between px-3.5 py-2.5 hover:bg-muted/40 transition-colors select-none"
                onClick={() => setToolsOpen(!toolsOpen)}
              >
                <div className="flex items-center gap-2">
                  <Wrench className="size-3.5 text-primary" />
                  <span className="font-medium text-foreground">
                    {t("project.toolExecSummary", { count: turn.tools.length })}
                  </span>
                  <span className="text-[11px] text-muted-foreground hidden sm:inline">
                    ({turn.tools.map((tl) => tl.name).filter((v, i, a) => a.indexOf(v) === i).join(", ")})
                  </span>
                </div>
                <div className="flex items-center gap-1 text-[11px] text-primary hover:underline">
                  <span>{toolsOpen ? t("project.collapse") : t("project.viewToolDetails")}</span>
                  {toolsOpen ? <ChevronUp className="size-3" /> : <ChevronDown className="size-3" />}
                </div>
              </div>

              {toolsOpen && (
                <div className="border-t border-border/60 p-3 space-y-3 bg-background/40 max-h-[30rem] overflow-y-auto">
                  {turn.tools.map((tl, tIdx) => (
                    <div key={tl.id || tIdx} className="rounded border border-border/70 bg-card p-2.5 space-y-2">
                      <div className="flex items-center justify-between text-[11px]">
                        <span className="inline-flex items-center gap-1.5 font-semibold text-foreground">
                          <Wrench className="size-3 text-primary" />
                          {tl.name}
                        </span>
                        {tl.isError && (
                          <Badge variant="warning" className="text-[10px] py-0 px-1">
                            Error
                          </Badge>
                        )}
                      </div>

                      {tl.args && (
                        <div className="space-y-1">
                          <span className="text-[10px] uppercase font-mono text-muted-foreground">Arguments:</span>
                          <pre className="m-0 max-h-36 overflow-x-auto rounded bg-muted/60 p-2 font-mono text-[11px] text-muted-foreground leading-snug">
                            {tl.args}
                          </pre>
                        </div>
                      )}

                      {tl.result && (
                        <div className="space-y-1">
                          <span className="text-[10px] uppercase font-mono text-muted-foreground">Output:</span>
                          <pre className="m-0 max-h-48 overflow-x-auto rounded bg-muted/60 p-2 font-mono text-[11px] text-muted-foreground leading-snug">
                            {tl.result}
                          </pre>
                        </div>
                      )}
                    </div>
                  ))}
                </div>
              )}
            </div>
          )}

          {/* Agent Responses & Thinking */}
          {turn.agentResponses.map((resp, rIdx) => {
            const isTextExpanded = expandedTexts[resp.id]
            const textContent = resp.text || ""
            const isLong = textContent.length > 600 || textContent.split("\n").length > 10

            return (
              <div key={resp.id || rIdx} className="rounded-md border border-border/70 bg-card p-3.5 space-y-3">
                <div className="flex items-center justify-between text-xs text-muted-foreground pb-1 border-b border-border/50">
                  <span className="font-semibold text-foreground/90 inline-flex items-center gap-1.5">
                    <Bot className="size-3.5 text-primary" />
                    {t("project.assistantResponse")}
                  </span>
                  <div className="flex items-center gap-2 text-[11px]">
                    {resp.commitSha && (
                      <span className="font-mono text-[10px] bg-muted px-1.5 py-0.5 rounded">
                        commit: {resp.commitSha.slice(0, 7)}
                      </span>
                    )}
                    <span>{new Date(resp.createdAt).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}</span>
                  </div>
                </div>

                {/* Thinking Fold */}
                {resp.thinking && (
                  <details className="group rounded-md border border-border/60 bg-muted/20 text-xs">
                    <summary className="flex cursor-pointer items-center justify-between px-3 py-1.5 font-medium text-muted-foreground hover:text-foreground select-none">
                      <span className="flex items-center gap-1.5">
                        <Sparkles className="size-3.5 text-amber-500" />
                        {t("project.thinking")}
                      </span>
                      <span className="text-[10px] text-muted-foreground/70 group-open:hidden">
                        {t("project.expandFull")}
                      </span>
                    </summary>
                    <div className="border-t border-border/50 p-3 font-mono text-[11px] leading-relaxed text-muted-foreground whitespace-pre-wrap max-h-60 overflow-y-auto">
                      {resp.thinking}
                    </div>
                  </details>
                )}

                {/* Response Text with Max-height clamp */}
                {textContent && (
                  <div className="space-y-1">
                    <div
                      className={cn(
                        "text-xs leading-relaxed text-foreground whitespace-pre-wrap font-sans transition-all",
                        !isTextExpanded && isLong && "max-h-56 overflow-hidden relative"
                      )}
                    >
                      {textContent}
                      {!isTextExpanded && isLong && (
                        <div className="absolute inset-x-0 bottom-0 h-16 bg-gradient-to-t from-card to-transparent pointer-events-none" />
                      )}
                    </div>
                    {isLong && (
                      <button
                        type="button"
                        className="text-xs text-primary font-medium hover:underline pt-1 inline-flex items-center gap-1"
                        onClick={() => toggleTextExpand(resp.id)}
                      >
                        {isTextExpanded ? t("project.collapse") : t("project.expandFull")}
                      </button>
                    )}
                  </div>
                )}
              </div>
            )
          })}
        </div>
      )}
    </Card>
  )
}

export default function ProjectPage() {
  useI18n()
  const { id } = useParams()
  const [params, setParams] = useSearchParams()
  const rawTab = params.get("tab")
  const tab: Tab = isTab(rawTab) ? rawTab : "code"
  const [project, setProject] = useState<Project | null>(null)
  const [tree, setTree] = useState<TreeEntry[]>([])
  const [log, setLog] = useState("")
  const [branches, setBranches] = useState<string[]>([])
  const [current, setCurrent] = useState("")
  const [status, setStatus] = useState("")
  const [selected, setSelected] = useState<string | null>(null)
  const [content, setContent] = useState("")
  const [mode, setMode] = useState<"preview" | "raw" | "view" | "edit">("preview")
  const [expandedDirs, setExpandedDirs] = useState<Set<string>>(new Set())
  const [commitMsg, setCommitMsg] = useState("")
  const [fileOpen, setFileOpen] = useState(false)
  const [commitOpen, setCommitOpen] = useState(false)
  const [branchOpen, setBranchOpen] = useState(false)
  const [commitShow, setCommitShow] = useState("")
  const [showOpen, setShowOpen] = useState(false)
  const [newPath, setNewPath] = useState("README.md")
  const [newBranch, setNewBranch] = useState("")
  const [pending, setPending] = useState(false)
  const [sessionEvents, setSessionEvents] = useState<SessionEvent[]>([])
  const [loadingSessions, setLoadingSessions] = useState(false)

  function toggleDir(dirPath: string) {
    setExpandedDirs((prev) => {
      const next = new Set(prev)
      if (next.has(dirPath)) {
        next.delete(dirPath)
      } else {
        next.add(dirPath)
      }
      return next
    })
  }

  function expandAncestors(filePath: string) {
    const parts = filePath.split("/").filter(Boolean)
    if (parts.length <= 1) return
    setExpandedDirs((prev) => {
      const next = new Set(prev)
      for (let i = 1; i < parts.length; i++) {
        next.add(parts.slice(0, i).join("/"))
      }
      return next
    })
  }

  function setTab(next: Tab) {
    setParams(next === "code" ? {} : { tab: next }, { replace: true })
  }

  async function refresh(selectPath?: string) {
    if (!id) return
    const p = await api<Project>(`/api/v1/projects/${id}`)
    setProject(p)
    const tr = await api<{ tree: TreeEntry[] }>(`/api/v1/repos/${id}/tree`)
    const entries = tr.tree ?? []
    setTree(entries)
    const files = entries.filter((e) => e.kind === "file")
    const lg = await api<{ log: string }>(`/api/v1/repos/${id}/log?limit=50`).catch(() => ({
      log: "",
    }))
    setLog(lg.log ?? "")
    const br = await api<{ branches: string[]; current?: string }>(
      `/api/v1/repos/${id}/branches`
    ).catch(() => ({ branches: [] as string[], current: "" }))
    setBranches(br.branches ?? [])
    setCurrent(br.current ?? "")
    const st = await api<{ status: string }>(`/api/v1/repos/${id}/status`).catch(() => ({
      status: "",
    }))
    setStatus(st.status ?? "")
    if (selectPath) {
      setSelected(selectPath)
      return
    }
    setSelected((cur) => {
      if (cur && files.some((f) => f.path === cur)) return cur
      const readme = files.find((f) => f.path.toLowerCase() === "readme.md")
      return readme?.path ?? files[0]?.path ?? null
    })
  }

  useEffect(() => {
    if (!id) return
    void refresh().catch((err) =>
      toast.error(err instanceof Error ? err.message : t("common.error"))
    )
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [id])

  useEffect(() => {
    if (!id || !selected) {
      setContent("")
      return
    }
    expandAncestors(selected)
    const isMd = selected.toLowerCase().endsWith(".md") || selected.toLowerCase().endsWith(".markdown")
    setMode(isMd ? "preview" : "view")
    const entry = tree.find((e) => e.path === selected)
    if (entry && entry.kind !== "file") return
    void api<{ content: string }>(`/api/v1/repos/${id}/files/${selected}`)
      .then((file) => setContent(file.content ?? ""))
      .catch((err) => toast.error(err instanceof Error ? err.message : t("common.error")))
  }, [id, selected, tree])

  useEffect(() => {
    if (!id || tab !== "sessions") return
    setLoadingSessions(true)
    api<{ events: SessionEvent[] }>(`/api/v1/projects/${id}/events?limit=200`)
      .then((res) => {
        setSessionEvents(res.events ?? [])
      })
      .catch((err) => {
        toast.error(err instanceof Error ? err.message : t("common.error"))
      })
      .finally(() => setLoadingSessions(false))
  }, [id, tab])

  const [expandAllTurns, setExpandAllTurns] = useState(false)

  const sessionTurns = useMemo(() => {
    if (!sessionEvents || sessionEvents.length === 0) return []

    // Group raw events into user-initiated turns
    const turns: SessionTurn[] = []
    let currentTurn: SessionTurn | null = null

    // Track tool calls by ID so toolResult can be paired
    const toolCallMap = new Map<string, ParsedToolItem>()

    for (const ev of sessionEvents) {
      const payload = ev.payload || {}
      const role = (payload.role as string) || (payload.message as { role?: string })?.role || ""
      const isUser = role === "user" || ev.event_type.includes("prompt") || (ev.event_type === "pi.message" && role === "user")
      const commitSha = (payload.commit_sha as string) || undefined

      let text = ""
      let thinking = ""
      const newTools: ParsedToolItem[] = []

      // Parse payload
      if (payload.type === "message" || payload.message) {
        const msg = (payload.message || payload) as {
          role?: string
          content?: Array<{
            id?: string
            type: string
            text?: string
            thinking?: string
            name?: string
            arguments?: unknown
          }>
        }
        if (Array.isArray(msg.content)) {
          for (const c of msg.content) {
            if (c.type === "text" && c.text) text += (text ? "\n" : "") + c.text
            if (c.type === "thinking" && c.thinking) thinking += c.thinking
            if (c.type === "toolCall") {
              const toolItem: ParsedToolItem = {
                id: c.id || `${ev.id}-tool-${newTools.length}`,
                name: c.name || "tool",
                args: typeof c.arguments === "object" ? JSON.stringify(c.arguments, null, 2) : String(c.arguments || ""),
              }
              newTools.push(toolItem)
              if (c.id) {
                toolCallMap.set(c.id, toolItem)
              }
            }
          }
        }
      } else if (payload.type === "toolResult" || role === "toolResult") {
        const msg = (payload.message || payload) as {
          toolCallId?: string
          toolName?: string
          content?: Array<{ text?: string }>
          isError?: boolean
        }
        const callId = msg.toolCallId || (payload.toolCallId as string) || ""
        const content = msg.content || (payload.content as Array<{ text?: string }>) || []
        const resultText = Array.isArray(content) ? content.map((c) => c.text || "").join("\n") : ""
        const toolName = msg.toolName || (payload.toolName as string) || "tool"
        const isError = Boolean(msg.isError || payload.isError)

        if (callId && toolCallMap.has(callId)) {
          const item = toolCallMap.get(callId)!
          item.result = resultText
          item.isError = isError
        } else {
          // Unpaired tool result
          const item: ParsedToolItem = {
            id: callId || ev.id,
            name: toolName,
            result: resultText,
            isError,
          }
          newTools.push(item)
        }
      }

      if (isUser) {
        if (currentTurn) {
          turns.push(currentTurn)
        }
        currentTurn = {
          id: ev.id,
          userPrompt: {
            id: ev.id,
            text,
            createdAt: ev.created_at,
            commitSha,
          },
          agentResponses: [],
          tools: [],
          timestamp: ev.created_at,
          commitSha,
        }
      } else {
        if (!currentTurn) {
          currentTurn = {
            id: `turn-init-${ev.id}`,
            agentResponses: [],
            tools: [],
            timestamp: ev.created_at,
            commitSha,
          }
        }

        if (text || thinking) {
          currentTurn.agentResponses.push({
            id: ev.id,
            text: text || undefined,
            thinking: thinking || undefined,
            createdAt: ev.created_at,
            commitSha,
          })
        }

        if (newTools.length > 0) {
          currentTurn.tools.push(...newTools)
        }

        if (commitSha && !currentTurn.commitSha) {
          currentTurn.commitSha = commitSha
        }
      }
    }

    if (currentTurn) {
      turns.push(currentTurn)
    }

    return turns
  }, [sessionEvents])

  const commits = useMemo(() => parseLog(log), [log])
  const treeNodes = useMemo(() => buildFileTree(tree), [tree])
  const dirty = status
    .split("\n")
    .some((line) => line.trim() && !line.startsWith("##"))

  async function saveFile() {
    if (!id || !selected) return
    setPending(true)
    try {
      await api(`/api/v1/repos/${id}/files/${selected}`, {
        method: "PUT",
        body: JSON.stringify({ content }),
      })
      toast.success(t("project.saved"))
      await refresh(selected)
    } catch (err) {
      toast.error(err instanceof Error ? err.message : t("common.error"))
    } finally {
      setPending(false)
    }
  }

  async function createFile(e: FormEvent) {
    e.preventDefault()
    if (!id) return
    const path = newPath.trim().replace(/^\/+/, "")
    if (!path) return
    setPending(true)
    try {
      await api(`/api/v1/repos/${id}/files/${path}`, {
        method: "PUT",
        body: JSON.stringify({ content: "" }),
      })
      setFileOpen(false)
      await refresh(path)
    } catch (err) {
      toast.error(err instanceof Error ? err.message : t("common.error"))
    } finally {
      setPending(false)
    }
  }

  async function onCommit(e: FormEvent) {
    e.preventDefault()
    if (!id || !commitMsg.trim()) return
    setPending(true)
    try {
      if (selected) {
        await api(`/api/v1/repos/${id}/files/${selected}`, {
          method: "PUT",
          body: JSON.stringify({ content }),
        })
      }
      const res = await api<{ sha?: string }>(`/api/v1/repos/${id}/commit`, {
        method: "POST",
        body: JSON.stringify({ message: commitMsg.trim() }),
      })
      setCommitMsg("")
      setCommitOpen(false)
      toast.success(res.sha ? res.sha.slice(0, 8) : t("project.committed"))
      await refresh()
    } catch (err) {
      toast.error(err instanceof Error ? err.message : t("common.error"))
    } finally {
      setPending(false)
    }
  }

  async function onCreateBranch(e: FormEvent) {
    e.preventDefault()
    if (!id || !newBranch.trim()) return
    setPending(true)
    try {
      await api(`/api/v1/repos/${id}/branches`, {
        method: "POST",
        body: JSON.stringify({ name: newBranch.trim(), checkout: true }),
      })
      setNewBranch("")
      setBranchOpen(false)
      await refresh()
    } catch (err) {
      toast.error(err instanceof Error ? err.message : t("common.error"))
    } finally {
      setPending(false)
    }
  }

  async function onCheckout(name: string) {
    if (!id || !name || name === current) return
    setPending(true)
    try {
      await api(`/api/v1/repos/${id}/checkout`, {
        method: "POST",
        body: JSON.stringify({ name }),
      })
      await refresh()
    } catch (err) {
      toast.error(err instanceof Error ? err.message : t("common.error"))
    } finally {
      setPending(false)
    }
  }

  async function openCommit(sha: string) {
    if (!id) return
    try {
      const res = await api<{ show: string }>(`/api/v1/repos/${id}/show/${sha}`)
      setCommitShow(res.show ?? "")
      setShowOpen(true)
    } catch (err) {
      toast.error(err instanceof Error ? err.message : t("common.error"))
    }
  }

  const repoPath = project?.full_name || (project?.owner_username && project?.slug ? `${project.owner_username}/${project.slug}` : (project?.slug || id || ""))
  const cloneUrl = repoPath ? `${window.location.origin}/git/${repoPath}` : ""
  const token = getToken() ?? ""

  return (
    <PageShell>
      <PageContainer>
        <PageHeader
          title={
            <>
              <span>{project?.name ?? t("project.missing")}</span>
              <Badge variant="secondary" className="text-xs font-normal">
                {t("project.private")}
              </Badge>
            </>
          }
          action={
            dirty ? <Badge variant="warning">{t("project.changes")}</Badge> : null
          }
        />
        <nav className="mb-6 flex h-10 items-end gap-6 border-b border-border">
          {TABS.map((item) => (
            <button
              key={item}
              type="button"
              className={cn(
                "-mb-px border-b-2 pb-2 text-sm",
                tab === item
                  ? "border-primary font-medium text-primary"
                  : "border-transparent text-muted-foreground"
              )}
              onClick={() => setTab(item)}
            >
              {t(
                item === "code"
                  ? "project.tabCode"
                  : item === "commits"
                    ? "project.tabCommits"
                    : item === "sessions"
                      ? "project.tabSessions"
                      : "project.tabSettings"
              )}
            </button>
          ))}
        </nav>

        {tab === "code" ? (
          <>
            <div className="mb-4 flex flex-wrap items-center justify-between gap-3">
              <div className="flex items-center gap-2">
                <Select
                  value={current || undefined}
                  onChange={(v) => void onCheckout(v)}
                  className="w-48"
                  triggerClassName="h-9"
                  placeholder={t("project.noBranches")}
                  options={branches.map((name) => ({ value: name, label: name }))}
                />
                <Button variant="outline" size="sm" className="h-9" type="button" onClick={() => setBranchOpen(true)}>
                  {t("project.newBranch")}
                </Button>
              </div>
              <div className="flex items-center gap-2">
                <Button variant="outline" size="sm" className="h-9" type="button" onClick={() => setFileOpen(true)}>
                  {t("project.newFile")}
                </Button>
                <Button size="sm" className="h-9" type="button" onClick={() => setCommitOpen(true)}>
                  {t("project.commit")}
                </Button>
              </div>
            </div>

            <div className="grid grid-cols-1 gap-6 lg:grid-cols-12 h-[calc(100vh-14rem)] min-h-[36rem]">
              <Card className="flex flex-col p-3 lg:col-span-3 h-full overflow-hidden">
                {tree.length === 0 ? (
                  <p className="p-2 text-sm text-muted-foreground">{t("project.noFiles")}</p>
                ) : (
                  <ul className="flex-1 overflow-y-auto space-y-0.5">
                    {treeNodes.map((node) => (
                      <TreeNodeItem
                        key={node.path}
                        node={node}
                        selected={selected}
                        expandedDirs={expandedDirs}
                        onToggleDir={toggleDir}
                        onSelectFile={(path) => {
                          setSelected(path)
                          const isMd = path.toLowerCase().endsWith(".md") || path.toLowerCase().endsWith(".markdown")
                          setMode(isMd ? "preview" : "view")
                        }}
                      />
                    ))}
                  </ul>
                )}
              </Card>

              <Card className="flex flex-col p-4 lg:col-span-9 h-full overflow-hidden">
                {selected ? (
                  <div className="flex h-full min-h-0 flex-col gap-3">
                    <div className="flex flex-wrap items-center justify-between gap-2 border-b border-border pb-3">
                      <span className="font-mono text-xs text-muted-foreground">{selected}</span>
                      <div className="flex items-center gap-2">
                        {selected.toLowerCase().endsWith(".md") || selected.toLowerCase().endsWith(".markdown") ? (
                          <div className="flex items-center rounded-lg border border-border bg-muted/40 p-0.5 text-xs">
                            <button
                              type="button"
                              className={cn(
                                "rounded-md px-2.5 py-1 transition-all",
                                mode === "preview" ? "bg-background font-medium text-foreground shadow-sm" : "text-muted-foreground hover:text-foreground"
                              )}
                              onClick={() => setMode("preview")}
                            >
                              {t("project.preview")}
                            </button>
                            <button
                              type="button"
                              className={cn(
                                "rounded-md px-2.5 py-1 transition-all",
                                mode === "raw" ? "bg-background font-medium text-foreground shadow-sm" : "text-muted-foreground hover:text-foreground"
                              )}
                              onClick={() => setMode("raw")}
                            >
                              {t("project.raw")}
                            </button>
                          </div>
                        ) : (
                          <div className="flex items-center rounded-lg border border-border bg-muted/40 p-0.5 text-xs">
                            <button
                              type="button"
                              className={cn(
                                "rounded-md px-2.5 py-1 transition-all",
                                mode === "view" ? "bg-background font-medium text-foreground shadow-sm" : "text-muted-foreground hover:text-foreground"
                              )}
                              onClick={() => setMode("view")}
                            >
                              {t("project.view")}
                            </button>
                            <button
                              type="button"
                              className={cn(
                                "rounded-md px-2.5 py-1 transition-all",
                                mode === "edit" ? "bg-background font-medium text-foreground shadow-sm" : "text-muted-foreground hover:text-foreground"
                              )}
                              onClick={() => setMode("edit")}
                            >
                              {t("project.edit")}
                            </button>
                          </div>
                        )}
                      </div>
                    </div>

                    <div className="flex-1 overflow-y-auto">
                      {mode === "preview" && (
                        <div className="prose prose-sm dark:prose-invert max-w-none py-2 px-1">
                          <ReactMarkdown
                            remarkPlugins={[remarkGfm]}
                            components={{
                              code({ className, children, ...props }) {
                                const match = /language-(\w+)/.exec(className || "")
                                const language = match ? match[1] : ""
                                const codeStr = String(children).replace(/\n$/, "")
                                if (language && hljs.getLanguage(language)) {
                                  try {
                                    const highlighted = hljs.highlight(codeStr, { language, ignoreIllegals: true }).value
                                    return (
                                      <code
                                        className={className}
                                        {...props}
                                        dangerouslySetInnerHTML={{ __html: highlighted }}
                                      />
                                    )
                                  } catch {
                                    // fall through
                                  }
                                }
                                return (
                                  <code className={className} {...props}>
                                    {children}
                                  </code>
                                )
                              },
                            }}
                          >
                            {content}
                          </ReactMarkdown>
                        </div>
                      )}

                      {mode === "view" && (
                        <pre className="m-0 h-full overflow-x-auto rounded-md bg-muted/30 p-4 font-mono text-xs leading-relaxed hljs">
                          <code
                            dangerouslySetInnerHTML={{
                              __html: (() => {
                                const lang = getLanguage(selected)
                                if (hljs.getLanguage(lang)) {
                                  try {
                                    return hljs.highlight(content, { language: lang, ignoreIllegals: true }).value
                                  } catch {
                                    // fallback
                                  }
                                }
                                return hljs.highlightAuto(content).value
                              })(),
                            }}
                          />
                        </pre>
                      )}

                      {(mode === "raw" || mode === "edit") && (
                        <div className="flex h-full flex-col gap-3">
                          <Textarea
                            className="min-h-[20rem] flex-1 font-mono text-xs leading-relaxed"
                            value={content}
                            onChange={(e) => setContent(e.target.value)}
                          />
                          <div className="flex justify-end">
                            <Button
                              type="button"
                              size="sm"
                              disabled={pending}
                              onClick={() => void saveFile()}
                            >
                              {t("project.save")}
                            </Button>
                          </div>
                        </div>
                      )}
                    </div>
                  </div>
                ) : (
                  <div className="flex h-full min-h-[20rem] flex-col justify-center gap-4 p-4 sm:p-6">
                    <div className="flex items-center gap-2 text-foreground font-medium text-sm">
                      <Terminal className="size-4 text-primary" />
                      <span>{t("project.quickstart")}</span>
                    </div>

                    <div className="space-y-3 max-w-xl">
                      <div className="space-y-1">
                        <span className="text-xs text-muted-foreground">{t("project.cliInstall")}</span>
                        <CopyableCode text="curl -fsSL https://openhub.run/install.sh | sh" />
                      </div>

                      <div className="space-y-1">
                        <span className="text-xs text-muted-foreground">{t("project.cliLogin")}</span>
                        <CopyableCode text={`oh login --token ${token} --url ${window.location.origin}`} />
                      </div>

                      <div className="space-y-1">
                        <span className="text-xs text-muted-foreground">{t("project.cliClone")}</span>
                        <CopyableCode text={`oh clone ${repoPath}`} />
                      </div>
                    </div>
                  </div>
                )}
              </Card>
            </div>
          </>
        ) : null}

        {tab === "commits" ? (
          <Card className="gap-0 p-4 sm:p-6">
            <Table className="table-fixed">
              <TableHeader className="sticky top-0 bg-card">
                <TableRow>
                  <TableHead className="w-28">{t("project.commitSha")}</TableHead>
                  <TableHead className="w-32">{t("project.commitDate")}</TableHead>
                  <TableHead>{t("project.commitMessage")}</TableHead>
                </TableRow>
              </TableHeader>
              <TableBody>
                {commits.length === 0 ? (
                  <TableRow>
                    <TableCell colSpan={3} className="text-muted-foreground">
                      {t("project.noCommits")}
                    </TableCell>
                  </TableRow>
                ) : (
                  commits.map((row) => (
                    <TableRow
                      key={row.sha}
                      className="cursor-pointer hover:bg-muted/50"
                      onClick={() => void openCommit(row.sha)}
                    >
                      <TableCell className="font-mono text-meta-sm">{row.sha}</TableCell>
                      <TableCell>{row.date}</TableCell>
                      <TableCell>{row.message}</TableCell>
                    </TableRow>
                  ))
                )}
              </TableBody>
            </Table>
          </Card>
        ) : null}

        {tab === "sessions" ? (
          <div className="space-y-4">
            <div className="flex items-center justify-between text-xs text-muted-foreground">
              <div className="flex items-center gap-2">
                <span className="font-medium text-foreground">
                  {sessionTurns.length} {t("project.sessionTurns")}
                </span>
                <span>•</span>
                <span>
                  {sessionEvents.length} {t("project.sessionEvents")}
                </span>
              </div>
              <div className="flex items-center gap-2">
                {sessionTurns.length > 0 && (
                  <Button
                    variant="ghost"
                    size="sm"
                    className="h-8 text-xs text-muted-foreground hover:text-foreground"
                    onClick={() => setExpandAllTurns(!expandAllTurns)}
                  >
                    {expandAllTurns ? t("project.collapseAll") : t("project.expandAll")}
                  </Button>
                )}
                <Button
                  variant="outline"
                  size="sm"
                  className="h-8 text-xs gap-1.5"
                  disabled={loadingSessions}
                  onClick={() => {
                    if (!id) return
                    setLoadingSessions(true)
                    api<{ events: SessionEvent[] }>(`/api/v1/projects/${id}/events?limit=200`)
                      .then((res) => setSessionEvents(res.events ?? []))
                      .catch((err) => toast.error(err instanceof Error ? err.message : t("common.error")))
                      .finally(() => setLoadingSessions(false))
                  }}
                >
                  <Sparkles className="size-3.5 text-primary" />
                  {loadingSessions ? "..." : t("common.refresh", "刷新")}
                </Button>
              </div>
            </div>

            {sessionTurns.length === 0 ? (
              <Card className="flex flex-col items-center justify-center p-12 text-center">
                <Bot className="size-10 text-muted-foreground/50 mb-3" />
                <p className="text-sm font-medium text-foreground">{t("project.noSessions")}</p>
                <p className="text-xs text-muted-foreground mt-1 max-w-sm">
                  Run <code className="font-mono bg-muted px-1.5 py-0.5 rounded">oh sync</code> locally to upload agent prompt, thinking & execution trails.
                </p>
              </Card>
            ) : (
              <div className="space-y-3">
                {sessionTurns.map((turn, idx) => (
                  <SessionTurnCard
                    key={turn.id || idx}
                    turn={turn}
                    index={idx}
                    total={sessionTurns.length}
                    defaultExpanded={expandAllTurns || idx === 0}
                  />
                ))}
              </div>
            )}
          </div>
        ) : null}

        {tab === "settings" ? (
          <div className="max-w-2xl divide-y divide-border">
            <section className="py-4 first:pt-0">
              <h2 className="mb-3 text-sm font-medium">{t("project.access")}</h2>
              <div className="flex h-10 items-center justify-between gap-4">
                <span className="text-sm">{t("project.visibility")}</span>
                <Badge variant="secondary">{t("project.private")}</Badge>
              </div>
              <p className="text-meta-sm text-muted-foreground">{t("project.ownerOnly")}</p>
            </section>
            <section className="py-4">
              <h2 className="mb-3 text-sm font-medium">{t("project.git")}</h2>
              <div className="space-y-3">
                <div className="space-y-1">
                  <span className="text-xs text-muted-foreground">{t("project.cloneUrl")}</span>
                  <CopyableCode text={cloneUrl} />
                </div>
                <div className="flex items-start gap-2 pt-1">
                  <span className="w-28 shrink-0 text-sm">{t("project.pushAuth")}</span>
                  <p className="text-meta-sm text-muted-foreground">{t("project.pushAuthValue")}</p>
                </div>
              </div>
            </section>
            <section className="py-4">
              <div className="mb-3 flex items-center gap-2">
                <Terminal className="size-4 text-primary" />
                <h2 className="text-sm font-medium">{t("project.cli")}</h2>
              </div>
              <div className="space-y-3 max-w-xl">
                <div className="space-y-1">
                  <span className="text-xs text-muted-foreground">{t("project.cliInstall")}</span>
                  <CopyableCode text="curl -fsSL https://openhub.run/install.sh | sh" />
                </div>

                <div className="space-y-1">
                  <span className="text-xs text-muted-foreground">{t("project.cliLogin")}</span>
                  <CopyableCode text={`oh login --token ${token} --url ${window.location.origin}`} />
                </div>

                <div className="space-y-1">
                  <span className="text-xs text-muted-foreground">{t("project.cliClone")}</span>
                  <CopyableCode text={`oh clone ${repoPath}`} />
                </div>
              </div>
            </section>
          </div>
        ) : null}

        <Dialog open={fileOpen} onOpenChange={setFileOpen}>
          <DialogContent className="max-h-[85vh] overflow-y-auto sm:max-w-lg">
            <form onSubmit={(e) => void createFile(e)}>
              <DialogHeader>
                <DialogTitle>{t("project.newFile")}</DialogTitle>
              </DialogHeader>
              <div className="space-y-2">
                <Label htmlFor="path">{t("project.path")}</Label>
                <Input
                  id="path"
                  value={newPath}
                  onChange={(e) => setNewPath(e.target.value)}
                  autoComplete="off"
                  required
                />
              </div>
              <DialogFooter>
                <Button type="button" variant="outline" onClick={() => setFileOpen(false)}>
                  {t("projects.cancel")}
                </Button>
                <Button type="submit" disabled={pending}>
                  {t("project.newFile")}
                </Button>
              </DialogFooter>
            </form>
          </DialogContent>
        </Dialog>

        <Dialog open={commitOpen} onOpenChange={setCommitOpen}>
          <DialogContent className="max-h-[85vh] overflow-y-auto sm:max-w-lg">
            <form onSubmit={(e) => void onCommit(e)}>
              <DialogHeader>
                <DialogTitle>{t("project.commit")}</DialogTitle>
              </DialogHeader>
              <div className="space-y-4">
                <div className="space-y-2">
                  <Label htmlFor="commit-message">{t("project.commitMessage")}</Label>
                  <Input
                    id="commit-message"
                    value={commitMsg}
                    onChange={(e) => setCommitMsg(e.target.value)}
                    autoComplete="off"
                    required
                  />
                </div>
                {status ? (
                  <pre className="max-h-40 overflow-y-auto whitespace-pre-wrap break-words font-mono text-meta-sm text-muted-foreground">
                    {status}
                  </pre>
                ) : null}
              </div>
              <DialogFooter>
                <Button type="button" variant="outline" onClick={() => setCommitOpen(false)}>
                  {t("projects.cancel")}
                </Button>
                <Button type="submit" disabled={pending}>
                  {t("project.commit")}
                </Button>
              </DialogFooter>
            </form>
          </DialogContent>
        </Dialog>

        <Dialog open={branchOpen} onOpenChange={setBranchOpen}>
          <DialogContent className="max-h-[85vh] overflow-y-auto sm:max-w-lg">
            <form onSubmit={(e) => void onCreateBranch(e)}>
              <DialogHeader>
                <DialogTitle>{t("project.newBranch")}</DialogTitle>
              </DialogHeader>
              <div className="space-y-2">
                <Label htmlFor="branch-name">{t("project.branchName")}</Label>
                <Input
                  id="branch-name"
                  value={newBranch}
                  onChange={(e) => setNewBranch(e.target.value)}
                  autoComplete="off"
                  required
                />
              </div>
              <DialogFooter>
                <Button type="button" variant="outline" onClick={() => setBranchOpen(false)}>
                  {t("projects.cancel")}
                </Button>
                <Button type="submit" disabled={pending}>
                  {t("project.newBranch")}
                </Button>
              </DialogFooter>
            </form>
          </DialogContent>
        </Dialog>

        <Dialog open={showOpen} onOpenChange={setShowOpen}>
          <DialogContent className="max-h-[85vh] overflow-y-auto sm:max-w-3xl">
            <DialogHeader>
              <DialogTitle>{t("project.tabCommits")}</DialogTitle>
            </DialogHeader>
            <pre className="max-h-[60vh] overflow-y-auto whitespace-pre-wrap break-words font-mono text-meta-sm">
              {commitShow}
            </pre>
          </DialogContent>
        </Dialog>
      </PageContainer>
    </PageShell>
  )
}
