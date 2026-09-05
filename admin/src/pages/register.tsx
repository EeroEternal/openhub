import { useState, type FormEvent } from "react"
import { Link, Navigate } from "react-router-dom"
import { toast } from "sonner"
import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { api, getToken } from "@/lib/api"
import { t, useI18n } from "@/lib/i18n"

export default function RegisterPage() {
  useI18n()
  const [email, setEmail] = useState("")
  const [pending, setPending] = useState(false)
  const [done, setDone] = useState(false)

  if (getToken()) return <Navigate to="/" replace />

  async function onSubmit(e: FormEvent) {
    e.preventDefault()
    setPending(true)
    try {
      await api("/api/v1/auth/register", {
        method: "POST",
        body: JSON.stringify({ email }),
      })
      setDone(true)
    } catch (err) {
      toast.error(err instanceof Error ? err.message : t("common.error"))
    } finally {
      setPending(false)
    }
  }

  return (
    <div className="flex min-h-svh items-center justify-center bg-muted/40 p-4">
      <Card className="w-full max-w-sm p-6">
        <h1 className="mb-6 text-page-title text-foreground">{t("auth.register")}</h1>
        {done ? (
          <p className="text-body-md text-foreground">{t("auth.registerOk")}</p>
        ) : (
          <form className="space-y-4" onSubmit={onSubmit}>
            <p className="text-body-md text-muted-foreground">{t("auth.registerHint")}</p>
            <div className="space-y-2">
              <Label htmlFor="email">{t("auth.email")}</Label>
              <Input
                id="email"
                type="email"
                autoComplete="username"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                required
              />
            </div>
            <Button type="submit" className="w-full" disabled={pending}>
              {t("auth.register")}
            </Button>
          </form>
        )}
        <p className="mt-4 text-body-md text-muted-foreground">
          {t("auth.hasAccount")}{" "}
          <Link to="/login" className="text-foreground underline">
            {t("auth.login")}
          </Link>
        </p>
      </Card>
    </div>
  )
}
