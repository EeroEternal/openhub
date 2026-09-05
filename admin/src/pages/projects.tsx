/* eslint-disable react-hooks/set-state-in-effect */
import { useEffect, useMemo, useState, type FormEvent } from "react"
import { useNavigate } from "react-router-dom"
import { toast } from "sonner"
import { EntityListToolbar } from "@/components/entity-list/EntityListToolbar"
import { PageContainer } from "@/components/layout/page-container"
import { PageHeader } from "@/components/layout/page-header"
import { PageShell } from "@/components/layout/page-shell"
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
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table"
import { api } from "@/lib/api"
import { t, useI18n } from "@/lib/i18n"

type Project = {
  id: string
  slug: string
  name: string
  created_at: string
}

export default function ProjectsPage() {
  useI18n()
  const navigate = useNavigate()
  const [items, setItems] = useState<Project[]>([])
  const [query, setQuery] = useState("")
  const [open, setOpen] = useState(false)
  const [name, setName] = useState("")
  const [slug, setSlug] = useState("")
  const [pending, setPending] = useState(false)

  async function load() {
    try {
      const res = await api<{ projects: Project[] }>("/api/v1/projects")
      setItems(res.projects ?? [])
    } catch (err) {
      toast.error(err instanceof Error ? err.message : t("common.error"))
    }
  }

  useEffect(() => {
    void load()
  }, [])

  const rows = useMemo(() => {
    const q = query.trim().toLowerCase()
    return items.filter(
      (p) =>
        !q ||
        p.name.toLowerCase().includes(q) ||
        p.slug.toLowerCase().includes(q) ||
        p.id.toLowerCase().includes(q)
    )
  }, [items, query])

  async function onCreate(e: FormEvent) {
    e.preventDefault()
    setPending(true)
    try {
      const res = await api<Project>("/api/v1/projects", {
        method: "POST",
        body: JSON.stringify({ name, slug: slug.trim() || undefined }),
      })
      setOpen(false)
      setName("")
      setSlug("")
      navigate(`/projects/${res.id}`)
    } catch (err) {
      toast.error(err instanceof Error ? err.message : t("common.error"))
    } finally {
      setPending(false)
    }
  }

  return (
    <PageShell>
      <PageContainer>
        <PageHeader
          title={t("projects.title")}
          action={
            <Button onClick={() => setOpen(true)}>{t("projects.create")}</Button>
          }
        />
        <Card className="gap-0 p-4 sm:p-6">
          <EntityListToolbar
            searchValue={query}
            onSearchChange={setQuery}
            searchPlaceholder={t("projects.search")}
            resultCount={`${rows.length} ${t("projects.count")}`}
          />
          <Table className="table-fixed">
            <TableHeader className="sticky top-0 bg-card">
              <TableRow>
                <TableHead>{t("projects.name")}</TableHead>
                <TableHead>{t("projects.slug")}</TableHead>
                <TableHead>ID</TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {rows.length === 0 ? (
                <TableRow>
                  <TableCell colSpan={3} className="text-muted-foreground">
                    {t("projects.empty")}
                  </TableCell>
                </TableRow>
              ) : (
                rows.map((row) => (
                  <TableRow
                    key={row.id}
                    className="cursor-pointer hover:bg-muted/50"
                    onClick={() => navigate(`/projects/${row.id}`)}
                  >
                    <TableCell className="font-medium">{row.name}</TableCell>
                    <TableCell>{row.slug}</TableCell>
                    <TableCell className="truncate font-mono text-meta-sm">
                      {row.id}
                    </TableCell>
                  </TableRow>
                ))
              )}
            </TableBody>
          </Table>
        </Card>
        <Dialog open={open} onOpenChange={setOpen}>
          <DialogContent className="max-h-[85vh] overflow-y-auto sm:max-w-lg">
            <form onSubmit={onCreate}>
              <DialogHeader>
                <DialogTitle>{t("projects.create")}</DialogTitle>
              </DialogHeader>
              <div className="space-y-4">
                <div className="space-y-2">
                  <Label htmlFor="name">{t("projects.name")}</Label>
                  <Input
                    id="name"
                    value={name}
                    onChange={(e) => setName(e.target.value)}
                    required
                  />
                </div>
                <div className="space-y-2">
                  <Label htmlFor="slug">{t("projects.slugOptional")}</Label>
                  <Input
                    id="slug"
                    value={slug}
                    onChange={(e) => setSlug(e.target.value)}
                  />
                </div>
              </div>
              <DialogFooter>
                <Button type="button" variant="outline" onClick={() => setOpen(false)}>
                  {t("projects.cancel")}
                </Button>
                <Button type="submit" disabled={pending}>
                  {t("projects.create")}
                </Button>
              </DialogFooter>
            </form>
          </DialogContent>
        </Dialog>
      </PageContainer>
    </PageShell>
  )
}
