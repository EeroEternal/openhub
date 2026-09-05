/* eslint-disable react-hooks/set-state-in-effect */
import { useEffect, useState, type FormEvent } from "react"
import { useParams, useSearchParams } from "react-router-dom"
import { toast } from "sonner"
import { PageContainer } from "@/components/layout/page-container"
import { PageHeader } from "@/components/layout/page-header"
import { PageShell } from "@/components/layout/page-shell"
import { TwoPanelLayout } from "@/components/layout/two-panel-layout"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
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
import { api, getToken } from "@/lib/api"
import { t, useI18n } from "@/lib/i18n"
import { cn } from "@/lib/utils"

type Tab = "code" | "commits" | "sessions" | "settings"
type Project = { id: string; name: string; slug: string }
type TreeEntry = { path: string; kind: string; size?: number }
type SessionEvent = {
  id: string
  event_type: string
  payload: unknown
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

function payloadText(payload: unknown): string {
  if (typeof payload === "string") return payload
  if (payload && typeof payload === "object" && "text" in payload) {
    const text = (payload as { text?: unknown }).text
    if (typeof text === "string") return text
  }
  return JSON.stringify(payload, null, 2)
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
  const [events, setEvents] = useState<SessionEvent[]>([])
  const [selected, setSelected] = useState<string | null>(null)
  const [content, setContent] = useState("")
  const [commitMsg, setCommitMsg] = useState("")
  const [fileOpen, setFileOpen] = useState(false)
  const [commitOpen, setCommitOpen] = useState(false)
  const [sessionOpen, setSessionOpen] = useState(false)
  const [newPath, setNewPath] = useState("README.md")
  const [sessionType, setSessionType] = useState("user")
  const [sessionBody, setSessionBody] = useState("")
  const [pending, setPending] = useState(false)

  function setTab(next: Tab) {
    setParams(next === "code" ? {} : { tab: next }, { replace: true })
  }

  async function refresh(selectPath?: string) {
    if (!id) return
    const p = await api<Project>(`/api/v1/projects/${id}`)
    setProject(p)
    const tr = await api<{ tree: TreeEntry[] }>(`/api/v1/repos/${id}/tree`)
    const files = (tr.tree ?? []).filter((e) => e.kind === "file")
    setTree(files)
    const lg = await api<{ log: string }>(`/api/v1/repos/${id}/log?limit=20`).catch(() => ({
      log: "",
    }))
    setLog(lg.log ?? "")
    const ev = await api<{ events: SessionEvent[] }>(
      `/api/v1/projects/${id}/events?limit=200`
    ).catch(() => ({ events: [] }))
    setEvents(ev.events ?? [])
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
    void api<{ content: string }>(`/api/v1/repos/${id}/files/${selected}`)
      .then((file) => setContent(file.content ?? ""))
      .catch((err) => toast.error(err instanceof Error ? err.message : t("common.error")))
  }, [id, selected])

  async function saveFile() {
    if (!id || !selected) return
    setPending(true)
    try {
      await api(`/api/v1/repos/${id}/files/${selected}`, {
        method: "PUT",
        body: JSON.stringify({ content }),
      })
      toast.success(t("project.saved"))
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

  async function createSession(e: FormEvent) {
    e.preventDefault()
    if (!id || !sessionBody.trim()) return
    setPending(true)
    try {
      await api(`/api/v1/projects/${id}/events`, {
        method: "POST",
        body: JSON.stringify({
          events: [
            {
              id: crypto.randomUUID(),
              event_type: sessionType,
              payload: { text: sessionBody.trim() },
              created_at: new Date().toISOString(),
            },
          ],
        }),
      })
      setSessionBody("")
      setSessionOpen(false)
      await refresh()
    } catch (err) {
      toast.error(err instanceof Error ? err.message : t("common.error"))
    } finally {
      setPending(false)
    }
  }

  const cloneUrl = id ? `${window.location.origin}/git/${id}` : ""
  const eventsUrl = id ? `${window.location.origin}/api/v1/projects/${id}/events` : ""
  const token = getToken() ?? ""

  return (
    <PageShell>
      <PageContainer>
        <PageHeader
          title={project?.name ?? t("project.missing")}
          action={
            <div className="flex h-10 items-center gap-2">
              <Badge variant="secondary">{t("project.private")}</Badge>
              <Button variant="outline" type="button" onClick={() => void copyText(cloneUrl)}>
                {t("project.clone")}
              </Button>
            </div>
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
            <TwoPanelLayout
              className="min-h-[24rem]"
              left={
                <Card className="min-h-0 flex-1 overflow-y-auto p-4 xl:w-80 xl:shrink-0">
                  <div className="mb-3 flex items-center justify-between gap-2">
                    <p className="text-sm font-medium">{t("project.files")}</p>
                    <div className="flex shrink-0 gap-2">
                      <Button variant="outline" type="button" onClick={() => setFileOpen(true)}>
                        {t("project.newFile")}
                      </Button>
                      <Button type="button" onClick={() => setCommitOpen(true)}>
                        {t("project.commit")}
                      </Button>
                    </div>
                  </div>
                  {tree.length === 0 ? (
                    <p className="text-body-md text-muted-foreground">{t("project.noFiles")}</p>
                  ) : (
                    <ul>
                      {tree.map((entry) => (
                        <li key={entry.path}>
                          <button
                            type="button"
                            className={cn(
                              "w-full truncate rounded-sm px-2 py-1.5 text-left text-sm",
                              selected === entry.path
                                ? "bg-primary/10 font-medium"
                                : "hover:bg-muted/50"
                            )}
                            onClick={() => setSelected(entry.path)}
                          >
                            {entry.path}
                          </button>
                        </li>
                      ))}
                    </ul>
                  )}
                </Card>
              }
              right={
                <Card className="min-h-0 flex-1 overflow-y-auto p-4">
                  {selected ? (
                    <div className="flex h-full min-h-0 flex-col gap-3">
                      <Textarea
                        className="min-h-[16rem] flex-1 font-mono"
                        value={content}
                        onChange={(e) => setContent(e.target.value)}
                      />
                      <div>
                        <Button
                          type="button"
                          variant="outline"
                          disabled={pending}
                          onClick={() => void saveFile()}
                        >
                          {t("project.save")}
                        </Button>
                      </div>
                    </div>
                  ) : (
                    <p className="text-body-md text-muted-foreground">{t("project.emptyFile")}</p>
                  )}
                </Card>
              }
            />
          </>
        ) : null}

        {tab === "commits" ? (
          <Card className="overflow-y-auto p-4">
            {log.trim() ? (
              <pre className="whitespace-pre-wrap break-words font-mono text-meta-sm text-foreground">
                {log}
              </pre>
            ) : (
              <p className="text-body-md text-muted-foreground">{t("project.noCommits")}</p>
            )}
          </Card>
        ) : null}

        {tab === "sessions" ? (
          <>
          <div className="mb-4 flex h-10 items-center justify-end">
            <Button onClick={() => setSessionOpen(true)}>{t("project.newSession")}</Button>
          </div>
          {events.length === 0 ? (
            <Card className="p-4">
              <p className="text-body-md text-muted-foreground">{t("project.noSessions")}</p>
            </Card>
          ) : (
            <div className="space-y-3">
              {events.map((ev) => (
                <Card key={ev.id} className="p-4">
                  <div className="mb-2 flex items-center justify-between gap-3">
                    <Badge variant="outline">{ev.event_type}</Badge>
                    <span className="font-mono text-meta-sm text-muted-foreground">
                      {ev.created_at}
                    </span>
                  </div>
                  <pre className="whitespace-pre-wrap break-words text-body-md text-foreground">
                    {payloadText(ev.payload)}
                  </pre>
                </Card>
              ))}
            </div>
          )}
          </>
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
                <div className="flex items-center gap-2">
                  <span className="w-28 shrink-0 text-sm">{t("project.cloneUrl")}</span>
                  <Input readOnly value={cloneUrl} className="h-10 min-w-0 flex-1 font-mono" />
                  <Button type="button" variant="outline" onClick={() => void copyText(cloneUrl)}>
                    {t("project.copy")}
                  </Button>
                </div>
                <div className="flex items-start gap-2">
                  <span className="w-28 shrink-0 pt-1 text-sm">{t("project.pushAuth")}</span>
                  <p className="text-meta-sm text-muted-foreground">{t("project.pushAuthValue")}</p>
                </div>
              </div>
            </section>
            <section className="py-4">
              <h2 className="mb-3 text-sm font-medium">{t("project.agent")}</h2>
              <div className="space-y-3">
                <div className="flex items-center gap-2">
                  <span className="w-28 shrink-0 text-sm">{t("project.eventsUrl")}</span>
                  <Input readOnly value={eventsUrl} className="h-10 min-w-0 flex-1 font-mono" />
                  <Button type="button" variant="outline" onClick={() => void copyText(eventsUrl)}>
                    {t("project.copy")}
                  </Button>
                </div>
                <p className="text-meta-sm text-muted-foreground">{t("project.agentBind")}</p>
                <div className="flex h-10 items-center gap-2">
                  <span className="w-28 shrink-0 text-sm">{t("project.token")}</span>
                  <Button type="button" variant="outline" onClick={() => void copyText(token)}>
                    {t("project.copyToken")}
                  </Button>
                </div>
              </div>
            </section>
          </div>
        ) : null}

        <Dialog open={commitOpen} onOpenChange={setCommitOpen}>
          <DialogContent className="max-h-[85vh] overflow-y-auto sm:max-w-lg">
            <form onSubmit={(e) => void onCommit(e)}>
              <DialogHeader>
                <DialogTitle>{t("project.commit")}</DialogTitle>
              </DialogHeader>
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

        <Dialog open={sessionOpen} onOpenChange={setSessionOpen}>
          <DialogContent className="max-h-[85vh] overflow-y-auto sm:max-w-lg">
            <form onSubmit={(e) => void createSession(e)}>
              <DialogHeader>
                <DialogTitle>{t("project.newSession")}</DialogTitle>
              </DialogHeader>
              <div className="space-y-4">
                <div className="space-y-2">
                  <Label>{t("project.sessionType")}</Label>
                  <Select
                    value={sessionType}
                    onChange={setSessionType}
                    options={[
                      { value: "user", label: t("project.sessionUser") },
                      { value: "assistant", label: t("project.sessionAssistant") },
                      { value: "note", label: t("project.sessionNote") },
                    ]}
                  />
                </div>
                <div className="space-y-2">
                  <Label htmlFor="session-body">{t("project.sessionBody")}</Label>
                  <Textarea
                    id="session-body"
                    className="min-h-[8rem]"
                    value={sessionBody}
                    onChange={(e) => setSessionBody(e.target.value)}
                    required
                  />
                </div>
              </div>
              <DialogFooter>
                <Button type="button" variant="outline" onClick={() => setSessionOpen(false)}>
                  {t("projects.cancel")}
                </Button>
                <Button type="submit" disabled={pending}>
                  {t("project.newSession")}
                </Button>
              </DialogFooter>
            </form>
          </DialogContent>
        </Dialog>
      </PageContainer>
    </PageShell>
  )
}
