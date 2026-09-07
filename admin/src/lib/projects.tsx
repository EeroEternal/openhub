/* eslint-disable react-hooks/set-state-in-effect, react-refresh/only-export-components */
import {
  createContext,
  useContext,
  useEffect,
  useState,
  type FormEvent,
  type ReactNode,
} from "react"
import { useNavigate } from "react-router-dom"
import { toast } from "sonner"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { api } from "@/lib/api"
import { cn } from "@/lib/utils"
import { t } from "@/lib/i18n"

export type Project = {
  id: string
  slug: string
  name: string
  created_at: string
}

type ProjectsContextValue = {
  projects: Project[]
  reload: () => Promise<void>
  createOpen: boolean
  setCreateOpen: (open: boolean) => void
}

const ProjectsContext = createContext<ProjectsContextValue | null>(null)

export function useProjects(): ProjectsContextValue {
  const ctx = useContext(ProjectsContext)
  if (!ctx) throw new Error("useProjects")
  return ctx
}

export function ProjectsProvider({ children }: { children: ReactNode }) {
  const [projects, setProjects] = useState<Project[]>([])
  const [createOpen, setCreateOpen] = useState(false)

  async function reload() {
    const res = await api<{ projects: Project[] }>("/api/v1/projects")
    setProjects(res.projects ?? [])
  }

  useEffect(() => {
    void reload().catch((err) =>
      toast.error(err instanceof Error ? err.message : t("common.error"))
    )
  }, [])

  return (
    <ProjectsContext.Provider
      value={{ projects, reload, createOpen, setCreateOpen }}
    >
      {children}
      <CreateProjectDialog />
    </ProjectsContext.Provider>
  )
}

function CreateProjectDialog() {
  const navigate = useNavigate()
  const { createOpen, setCreateOpen, reload } = useProjects()
  const [owner, setOwner] = useState("")
  const [name, setName] = useState("")
  const [slug, setSlug] = useState("")
  const [slugEdited, setSlugEdited] = useState(false)
  const [description, setDescription] = useState("")
  const [visibility, setVisibility] = useState<"private" | "public">("private")
  const [initReadme, setInitReadme] = useState(true)
  const [pending, setPending] = useState(false)

  const [slugStatus, setSlugStatus] = useState<"idle" | "checking" | "available" | "unavailable">("idle")

  useEffect(() => {
    if (createOpen) {
      void api<{ username?: string; email: string }>("/api/v1/me")
        .then((me) => setOwner(me.username || me.email.split("@")[0]))
        .catch(() => {})
    }
  }, [createOpen])

  function toSlug(text: string): string {
    return text
      .toLowerCase()
      .trim()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-+|-+$/g, "")
      .slice(0, 40)
  }

  function handleNameChange(val: string) {
    setName(val)
    if (!slugEdited) {
      setSlug(toSlug(val))
      setSlugStatus("idle")
    }
  }

  function handleSlugChange(val: string) {
    setSlugEdited(true)
    setSlug(toSlug(val))
    setSlugStatus("idle")
  }

  async function onCreate(e: FormEvent) {
    e.preventDefault()
    if (slugStatus === "unavailable") {
      toast.error(t("projects.slugUnavailable"))
      return
    }
    setPending(true)
    try {
      const res = await api<Project>("/api/v1/projects", {
        method: "POST",
        body: JSON.stringify({
          name: name.trim(),
          slug: slug.trim() || undefined,
          description: description.trim() || undefined,
          init_readme: initReadme,
        }),
      })
      setCreateOpen(false)
      setName("")
      setSlug("")
      setSlugEdited(false)
      setSlugStatus("idle")
      setDescription("")
      await reload()
      navigate(`/projects/${res.id}`)
    } catch (err) {
      toast.error(err instanceof Error ? err.message : t("common.error"))
    } finally {
      setPending(false)
    }
  }

  async function checkSlugManually(s: string) {
    const raw = s.trim()
    if (!raw) return
    setSlugStatus("checking")
    try {
      const res = await api<{ available: boolean; reason?: string }>(
        `/api/v1/projects/check-slug?slug=${encodeURIComponent(raw)}`
      )
      if (res.available) {
        setSlugStatus("available")
        toast.success(t("projects.slugAvailable"))
      } else {
        setSlugStatus("unavailable")
        toast.error(t("projects.slugUnavailable"))
      }
    } catch (err) {
      setSlugStatus("idle")
      toast.error(err instanceof Error ? err.message : t("common.error"))
    }
  }

  return (
    <Dialog open={createOpen} onOpenChange={setCreateOpen}>
      <DialogContent className="w-[92vw] sm:w-[480px] max-w-[480px] max-h-[85vh] overflow-y-auto">
        <form onSubmit={(e) => void onCreate(e)}>
          <DialogHeader>
            <DialogTitle>{t("projects.create")}</DialogTitle>
          </DialogHeader>
          <div className="space-y-4 py-2">
            {/* Project Name */}
            <div className="space-y-1.5">
              <Label htmlFor="project-name">{t("projects.name")}</Label>
              <Input
                id="project-name"
                value={name}
                onChange={(e) => handleNameChange(e.target.value)}
                placeholder={t("projects.namePlaceholder")}
                required
                autoFocus
              />
            </div>

            {/* Owner + Slug */}
            <div className="space-y-1.5">
              <div className="flex items-center justify-between">
                <Label htmlFor="project-slug">{t("projects.slug")}</Label>
                {slugStatus === "checking" && (
                  <span className="text-meta-sm text-muted-foreground animate-pulse">
                    {t("projects.slugChecking")}
                  </span>
                )}
                {slugStatus === "available" && (
                  <span className="text-meta-sm text-emerald-600 font-medium flex items-center gap-1">
                    ✓ {t("projects.slugAvailable")}
                  </span>
                )}
                {slugStatus === "unavailable" && (
                  <span className="text-meta-sm text-destructive font-medium flex items-center gap-1">
                    ✗ {t("projects.slugUnavailable")}
                  </span>
                )}
              </div>
              <div className="flex items-center gap-2">
                <div className="rounded-md border bg-muted/50 px-3 py-2 text-sm text-muted-foreground shrink-0 select-none">
                  {owner || "..."} /
                </div>
                <Input
                  id="project-slug"
                  value={slug}
                  onChange={(e) => handleSlugChange(e.target.value)}
                  placeholder="repository-slug"
                  className={cn(
                    "flex-1",
                    slugStatus === "unavailable" && "border-destructive focus-visible:ring-destructive",
                    slugStatus === "available" && "border-emerald-500 focus-visible:ring-emerald-500"
                  )}
                  required
                />
                <Button
                  type="button"
                  variant="outline"
                  className="shrink-0 h-10 px-3 text-sm"
                  onClick={() => void checkSlugManually(slug)}
                  disabled={!slug.trim() || slugStatus === "checking"}
                >
                  {t("projects.checkSlugBtn")}
                </Button>
              </div>
            </div>

            {/* Description (Optional) */}
            <div className="space-y-1.5">
              <Label htmlFor="project-desc">{t("projects.description")}</Label>
              <Input
                id="project-desc"
                value={description}
                onChange={(e) => setDescription(e.target.value)}
                placeholder={t("projects.descriptionPlaceholder")}
              />
            </div>

            {/* Visibility Mode */}
            <div className="space-y-1.5">
              <Label>{t("projects.visibility")}</Label>
              <div className="grid grid-cols-2 gap-2">
                <button
                  type="button"
                  onClick={() => setVisibility("private")}
                  className={cn(
                    "px-3 py-2 rounded-md border text-sm transition-all cursor-pointer",
                    visibility === "private"
                      ? "border-primary bg-primary/5 text-foreground font-medium ring-1 ring-primary/20"
                      : "border-muted bg-card text-muted-foreground hover:bg-muted/30"
                  )}
                >
                  {t("projects.private")}
                </button>
                <button
                  type="button"
                  onClick={() => setVisibility("public")}
                  className={cn(
                    "px-3 py-2 rounded-md border text-sm transition-all cursor-pointer",
                    visibility === "public"
                      ? "border-primary bg-primary/5 text-foreground font-medium ring-1 ring-primary/20"
                      : "border-muted bg-card text-muted-foreground hover:bg-muted/30"
                  )}
                >
                  {t("projects.public")}
                </button>
              </div>
            </div>

            {/* Init with README checkbox */}
            <label className="flex items-center gap-2.5 text-sm text-foreground cursor-pointer select-none pt-1">
              <input
                id="init-readme"
                type="checkbox"
                checked={initReadme}
                onChange={(e) => setInitReadme(e.target.checked)}
                className="h-4 w-4 rounded border-muted text-primary focus:ring-primary cursor-pointer"
              />
              <span>{t("projects.initReadme")}</span>
            </label>
          </div>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => setCreateOpen(false)}>
              {t("projects.cancel")}
            </Button>
            <Button
              type="submit"
              disabled={
                pending ||
                !name.trim() ||
                !slug.trim() ||
                slugStatus === "checking" ||
                slugStatus === "unavailable"
              }
            >
              {pending ? "..." : t("projects.create")}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
