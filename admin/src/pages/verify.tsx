import { useState, type FormEvent } from "react"
import { Navigate, useNavigate, useSearchParams } from "react-router-dom"
import { toast } from "sonner"
import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { api, getToken, setToken } from "@/lib/api"
import { t, useI18n } from "@/lib/i18n"

export default function VerifyPage() {
  useI18n()
  const navigate = useNavigate()
  const [params] = useSearchParams()
  const token = params.get("token") ?? ""
  const [password, setPassword] = useState("")
  const [confirm, setConfirm] = useState("")
  const [pending, setPending] = useState(false)

  if (getToken()) return <Navigate to="/" replace />

  async function onSubmit(e: FormEvent) {
    e.preventDefault()
    if (!token) {
      toast.error(t("auth.invalidToken"))
      return
    }
    if (password !== confirm) {
      toast.error(t("auth.passwordMismatch"))
      return
    }
    setPending(true)
    try {
      const res = await api<{ token: string }>("/api/v1/auth/password", {
        method: "POST",
        body: JSON.stringify({ token, password }),
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
    <div className="flex min-h-svh items-center justify-center bg-muted/40 p-4">
      <Card className="w-full max-w-sm p-6">
        <h1 className="mb-6 text-page-title text-foreground">{t("auth.verifyTitle")}</h1>
        <form className="space-y-4" onSubmit={onSubmit}>
          <div className="space-y-2">
            <Label htmlFor="password">{t("auth.password")}</Label>
            <Input
              id="password"
              type="password"
              autoComplete="new-password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              minLength={8}
              required
            />
          </div>
          <div className="space-y-2">
            <Label htmlFor="confirm">{t("auth.confirmPassword")}</Label>
            <Input
              id="confirm"
              type="password"
              autoComplete="new-password"
              value={confirm}
              onChange={(e) => setConfirm(e.target.value)}
              minLength={8}
              required
            />
          </div>
          <Button type="submit" className="w-full" disabled={pending}>
            {t("auth.verifyTitle")}
          </Button>
        </form>
      </Card>
    </div>
  )
}
