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
  const [name, setName] = useState("")
  const [slug, setSlug] = useState("")
  const [pending, setPending] = useState(false)

  async function onCreate(e: FormEvent) {
    e.preventDefault()
    setPending(true)
    try {
      const res = await api<Project>("/api/v1/projects", {
        method: "POST",
        body: JSON.stringify({ name, slug: slug.trim() || undefined }),
      })
      setCreateOpen(false)
      setName("")
      setSlug("")
      await reload()
      navigate(`/projects/${res.id}`)
    } catch (err) {
      toast.error(err instanceof Error ? err.message : t("common.error"))
    } finally {
      setPending(false)
    }
  }

  return (
    <Dialog open={createOpen} onOpenChange={setCreateOpen}>
      <DialogContent className="max-h-[85vh] overflow-y-auto sm:max-w-lg">
        <form onSubmit={(e) => void onCreate(e)}>
          <DialogHeader>
            <DialogTitle>{t("projects.create")}</DialogTitle>
          </DialogHeader>
          <div className="space-y-4">
            <div className="space-y-2">
              <Label htmlFor="project-name">{t("projects.name")}</Label>
              <Input
                id="project-name"
                value={name}
                onChange={(e) => setName(e.target.value)}
                required
              />
            </div>
            <div className="space-y-2">
              <Label htmlFor="project-slug">{t("projects.slugOptional")}</Label>
              <Input
                id="project-slug"
                value={slug}
                onChange={(e) => setSlug(e.target.value)}
              />
            </div>
          </div>
          <DialogFooter>
            <Button type="button" variant="outline" onClick={() => setCreateOpen(false)}>
              {t("projects.cancel")}
            </Button>
            <Button type="submit" disabled={pending}>
              {t("projects.create")}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
