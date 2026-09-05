import { Link, useLocation } from "react-router-dom"
import { FolderGit2, Plus } from "lucide-react"

import {
  Sidebar,
  SidebarContent,
  SidebarGroup,
  SidebarGroupAction,
  SidebarGroupLabel,
  SidebarHeader,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarRail,
} from "@/components/ui/sidebar"
import { APP_TITLE } from "@/lib/nav"
import { useProjects } from "@/lib/projects"
import { t, useI18n } from "@/lib/i18n"
import { cn } from "@/lib/utils"

export function AppSidebar({ ...props }: React.ComponentProps<typeof Sidebar>) {
  useI18n()
  const location = useLocation()
  const { projects, setCreateOpen } = useProjects()

  return (
    <Sidebar collapsible="icon" {...props}>
      <SidebarHeader>
        <SidebarMenu>
          <SidebarMenuItem>
            <SidebarMenuButton size="lg" asChild>
              <Link to="/">
                <div className="flex size-8 items-center justify-center rounded-md bg-primary text-primary-foreground text-sm font-semibold">
                  O
                </div>
                <div className="grid min-w-0 flex-1 text-left text-sm leading-tight">
                  <span className="break-words font-semibold text-base leading-5">{APP_TITLE}</span>
                </div>
              </Link>
            </SidebarMenuButton>
          </SidebarMenuItem>
        </SidebarMenu>
      </SidebarHeader>
      <SidebarContent>
        <SidebarGroup className="relative">
          <SidebarGroupLabel className="text-sidebar-foreground/85">
            {t("nav.projects")}
          </SidebarGroupLabel>
          <SidebarGroupAction
            type="button"
            title={t("projects.create")}
            onClick={() => setCreateOpen(true)}
          >
            <Plus />
            <span className="sr-only">{t("projects.create")}</span>
          </SidebarGroupAction>
          <SidebarMenu>
            {projects.map((project) => {
              const href = `/projects/${project.id}`
              const isActive = location.pathname === href
              return (
                <SidebarMenuItem key={project.id}>
                  <SidebarMenuButton asChild tooltip={project.name} isActive={isActive}>
                    <Link to={href} className={cn("flex items-center gap-3")}>
                      <FolderGit2 className="size-4" />
                      <span>{project.name}</span>
                    </Link>
                  </SidebarMenuButton>
                </SidebarMenuItem>
              )
            })}
          </SidebarMenu>
        </SidebarGroup>
      </SidebarContent>
      <SidebarRail />
    </Sidebar>
  )
}
