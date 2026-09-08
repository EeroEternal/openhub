import { useState, type FormEvent } from "react"
import { Navigate, useNavigate } from "react-router-dom"
import { Eye, EyeOff } from "lucide-react"
import { toast } from "sonner"
import { Button } from "@/components/ui/button"
import { AuthCardLayout } from "@/components/layout/auth-card-layout"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { api, getToken, setToken } from "@/lib/api"
import { t, useI18n } from "@/lib/i18n"

export default function LoginPage() {
  useI18n()
  const navigate = useNavigate()
  const [email, setEmail] = useState("")
  const [password, setPassword] = useState("")
  const [showPassword, setShowPassword] = useState(false)
  const [pending, setPending] = useState(false)

  if (getToken()) return <Navigate to="/" replace />

  async function onSubmit(e: FormEvent) {
    e.preventDefault()
    setPending(true)
    try {
      const res = await api<{ token: string }>("/api/v1/auth/login", {
        method: "POST",
        body: JSON.stringify({ email: email.trim(), password }),
      })
      setToken(res.token)
      navigate("/", { replace: true })
    } catch (err) {
      toast.error(err instanceof Error ? err.message : t("common.error"))
    } finally {
      setPending(false)
    }
  }

  return (
    <AuthCardLayout activeTab="login" title={t("auth.loginTitle")}>
      <form className="space-y-4" onSubmit={onSubmit}>
        <div className="space-y-2">
          <Label htmlFor="email">{t("auth.email")}</Label>
          <Input
            id="email"
            name="email"
            type="email"
            inputMode="email"
            autoComplete="username"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            placeholder={t("auth.emailPlaceholder")}
            required
            autoFocus
          />
        </div>
        <div className="space-y-2">
          <Label htmlFor="password">{t("auth.password")}</Label>
          <div className="relative">
            <Input
              id="password"
              name="password"
              type={showPassword ? "text" : "password"}
              autoComplete="current-password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              placeholder={t("auth.passwordPlaceholder")}
              className="pr-10"
              required
            />
            <button
              type="button"
              onClick={() => setShowPassword((open) => !open)}
              className="absolute right-3 top-1/2 -translate-y-1/2 p-1 text-muted-foreground hover:text-foreground"
              aria-label={showPassword ? t("auth.hidePassword") : t("auth.showPassword")}
            >
              {showPassword ? <EyeOff className="h-4 w-4" /> : <Eye className="h-4 w-4" />}
            </button>
          </div>
        </div>
        <Button type="submit" className="w-full" disabled={pending}>
          {pending ? t("common.loading") : t("auth.login")}
        </Button>
      </form>
    </AuthCardLayout>
  )
}
