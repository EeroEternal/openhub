import type { LucideIcon } from "lucide-react"
import { FolderGit2, Settings } from "lucide-react"

export type NavItem = {
  nameKey: string
  href: string
  icon: LucideIcon
}

export type NavSection = {
  id: string
  titleKey: string
  collapsible?: boolean
  items: NavItem[]
}

export const APP_TITLE = "OpenHub"

export const NAV_SECTIONS: NavSection[] = [
  {
    id: "work",
    titleKey: "nav.projects",
    items: [{ nameKey: "nav.projects", href: "/", icon: FolderGit2 }],
  },
  {
    id: "system",
    titleKey: "nav.settings",
    items: [{ nameKey: "nav.settings", href: "/settings", icon: Settings }],
  },
]
