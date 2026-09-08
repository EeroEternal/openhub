import { useEffect, useState, type FormEvent } from "react"
import { useLocation, useNavigate } from "react-router-dom"
import { toast } from "sonner"
import { SidebarTrigger } from "@/components/ui/sidebar"
import { Separator } from "@/components/ui/separator"
import { Avatar, AvatarFallback } from "@/components/ui/avatar"
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
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { api, setToken } from "@/lib/api"
import { t, useI18n } from "@/lib/i18n"

export function SiteHeader() {
  useI18n()
  const navigate = useNavigate()
  const location = useLocation()
  const [email, setEmail] = useState("")
  const [passwordOpen, setPasswordOpen] = useState(false)
  const [currentPassword, setCurrentPassword] = useState("")
  const [newPassword, setNewPassword] = useState("")
  const [pending, setPending] = useState(false)

  useEffect(() => {
    void api<{ email: string }>("/api/v1/me")
      .then((me) => setEmail(me.email))
      .catch(() => setEmail(""))
  }, [location.pathname])

  const initial = (email.split("@")[0] ?? "?").charAt(0).toUpperCase() || "?"

  async function onChangePassword(e: FormEvent) {
    e.preventDefault()
    setPending(true)
    try {
      await api("/api/v1/me/password", {
        method: "POST",
        body: JSON.stringify({
          current_password: currentPassword,
          new_password: newPassword,
        }),
      })
      setPasswordOpen(false)
      setCurrentPassword("")
      setNewPassword("")
      toast.success(t("settings.passwordChanged"))
    } catch (err) {
      toast.error(err instanceof Error ? err.message : t("common.error"))
    } finally {
      setPending(false)
    }
  }

  return (
    <header className="relative z-50 flex h-14 shrink-0 items-center border-b bg-background/95 px-4 backdrop-blur supports-[backdrop-filter]:bg-background/60">
      <SidebarTrigger className="-ml-1" />
      <Separator orientation="vertical" className="mx-3 hidden h-4 sm:block" />
      <div className="ml-auto flex h-10 items-center">
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <button
              type="button"
              className="rounded-full focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
              aria-label={t("settings.account")}
            >
              <Avatar className="size-8">
                <AvatarFallback className="bg-muted text-sm font-medium text-foreground">
                  {initial}
                </AvatarFallback>
              </Avatar>
            </button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end" className="min-w-48">
            <DropdownMenuLabel className="truncate font-normal">{email}</DropdownMenuLabel>
            <DropdownMenuSeparator />
            <DropdownMenuItem onClick={() => navigate("/settings")}>
              {t("nav.settings")}
            </DropdownMenuItem>
            <DropdownMenuItem onClick={() => setPasswordOpen(true)}>
              {t("settings.password")}
            </DropdownMenuItem>
            <DropdownMenuItem onClick={() => navigate("/help")}>
              {t("nav.help")}
            </DropdownMenuItem>
            <DropdownMenuItem
              onClick={() => {
                setToken(null)
                navigate("/login", { replace: true })
              }}
            >
              {t("settings.logout")}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
      <Dialog open={passwordOpen} onOpenChange={setPasswordOpen}>
        <DialogContent className="max-h-[85vh] overflow-y-auto sm:max-w-lg">
          <form onSubmit={(e) => void onChangePassword(e)}>
            <DialogHeader>
              <DialogTitle>{t("settings.password")}</DialogTitle>
            </DialogHeader>
            <div className="space-y-4">
              <div className="space-y-2">
                <Label htmlFor="current-password">{t("settings.currentPassword")}</Label>
                <Input
                  id="current-password"
                  type="password"
                  autoComplete="off"
                  value={currentPassword}
                  onChange={(e) => setCurrentPassword(e.target.value)}
                  required
                />
              </div>
              <div className="space-y-2">
                <Label htmlFor="new-password">{t("settings.newPassword")}</Label>
                <Input
                  id="new-password"
                  type="password"
                  autoComplete="off"
                  minLength={8}
                  value={newPassword}
                  onChange={(e) => setNewPassword(e.target.value)}
                  required
                />
              </div>
            </div>
            <DialogFooter>
              <Button type="button" variant="outline" onClick={() => setPasswordOpen(false)}>
                {t("projects.cancel")}
              </Button>
              <Button type="submit" disabled={pending}>
                {t("settings.save")}
              </Button>
            </DialogFooter>
          </form>
        </DialogContent>
      </Dialog>
    </header>
  )
}
