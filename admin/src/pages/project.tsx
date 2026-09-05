import { useEffect, useState } from "react"
import { useParams } from "react-router-dom"
import { toast } from "sonner"
import { PageContainer } from "@/components/layout/page-container"
import { PageHeader } from "@/components/layout/page-header"
import { PageShell } from "@/components/layout/page-shell"
import { TwoPanelLayout } from "@/components/layout/two-panel-layout"
import { Card } from "@/components/ui/card"
import { api } from "@/lib/api"
import { t, useI18n } from "@/lib/i18n"
import { cn } from "@/lib/utils"

type Project = { id: string; name: string; slug: string }
type TreeEntry = { path: string; kind: string; size?: number }

export default function ProjectPage() {
  useI18n()
  const { id } = useParams()
  const [project, setProject] = useState<Project | null>(null)
  const [tree, setTree] = useState<TreeEntry[]>([])
  const [selected, setSelected] = useState<string | null>(null)
  const [content, setContent] = useState<string>("")

  useEffect(() => {
    if (!id) return
    void (async () => {
      try {
        const p = await api<Project>(`/api/v1/projects/${id}`)
        setProject(p)
        const tr = await api<{ tree: TreeEntry[] }>(`/api/v1/repos/${id}/tree`)
        setTree((tr.tree ?? []).filter((e) => e.kind === "file"))
      } catch (err) {
        toast.error(err instanceof Error ? err.message : t("common.error"))
      }
    })()
  }, [id])

  useEffect(() => {
    if (!id || !selected) return
    void (async () => {
      try {
        const file = await api<{ content: string }>(
          `/api/v1/repos/${id}/files/${selected}`
        )
        setContent(file.content ?? "")
      } catch (err) {
        toast.error(err instanceof Error ? err.message : t("common.error"))
      }
    })()
  }, [id, selected])

  return (
    <PageShell>
      <PageContainer>
        <PageHeader title={project?.name ?? t("project.missing")} />
        <TwoPanelLayout
          className="min-h-[24rem]"
          left={
            <Card className="min-h-0 flex-1 overflow-y-auto p-4 xl:w-80 xl:shrink-0">
              <p className="mb-3 text-sm font-medium">{t("project.files")}</p>
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
            </Card>
          }
          right={
            <Card className="min-h-0 flex-1 overflow-y-auto p-4">
              {selected ? (
                <pre className="whitespace-pre-wrap break-words text-body-md text-foreground">
                  {content}
                </pre>
              ) : (
                <p className="text-body-md text-muted-foreground">{t("project.emptyFile")}</p>
              )}
            </Card>
          }
        />
      </PageContainer>
    </PageShell>
  )
}
