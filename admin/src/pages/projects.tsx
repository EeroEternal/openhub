import { Navigate } from "react-router-dom"
import { PageContainer } from "@/components/layout/page-container"
import { PageHeader } from "@/components/layout/page-header"
import { PageShell } from "@/components/layout/page-shell"
import { Button } from "@/components/ui/button"
import { useProjects } from "@/lib/projects"
import { t, useI18n } from "@/lib/i18n"

export default function ProjectsPage() {
  useI18n()
  const { projects, setCreateOpen } = useProjects()

  if (projects.length > 0) {
    return <Navigate to={`/projects/${projects[0].id}`} replace />
  }

  return (
    <PageShell>
      <PageContainer>
        <PageHeader
          title={t("projects.title")}
          action={
            <Button onClick={() => setCreateOpen(true)}>{t("projects.create")}</Button>
          }
        />
        <p className="text-body-md text-muted-foreground">{t("projects.empty")}</p>
      </PageContainer>
    </PageShell>
  )
}
