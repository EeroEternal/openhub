import { useState } from "react"
import { Link, Navigate } from "react-router-dom"
import { toast } from "sonner"
import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { api, getToken } from "@/lib/api"
import { t, useI18n } from "@/lib/i18n"

export default function CliLoginPage() {
  useI18n()
  const code = (new URLSearchParams(window.location.search).get("code") ?? "")
    .trim()
    .toUpperCase()
    .replace(/-/g, "")
  const [pending, setPending] = useState(false)
  const [done, setDone] = useState(false)

  if (!code) {
    return (
      <div className="flex min-h-screen items-center justify-center bg-background p-6">
        <p className="text-sm text-muted-foreground">{t("cli.missingCode")}</p>
      </div>
    )
  }

  if (!getToken()) {
    const next = `/cli?code=${encodeURIComponent(code)}`
    return <Navigate to={`/login?next=${encodeURIComponent(next)}`} replace />
  }

  async function approve() {
    setPending(true)
    try {
      await api("/api/v1/auth/cli/approve", {
        method: "POST",
        body: JSON.stringify({ code }),
      })
      setDone(true)
    } catch (err) {
      toast.error(err instanceof Error ? err.message : t("common.error"))
    } finally {
      setPending(false)
    }
  }

  return (
    <div className="flex min-h-screen items-center justify-center bg-background p-6">
      <Card className="w-full max-w-md space-y-4 p-8">
        <h1 className="text-page-title">{t("cli.title")}</h1>
        {done ? (
          <p className="text-sm text-muted-foreground">{t("cli.done")}</p>
        ) : (
          <>
            <p className="text-sm text-muted-foreground">{t("cli.body")}</p>
            <p className="font-mono text-sm">{code}</p>
            <Button className="w-full" onClick={() => void approve()} disabled={pending}>
              {pending ? t("common.loading") : t("cli.authorize")}
            </Button>
          </>
        )}
        <p className="text-sm">
          <Link to="/" className="text-primary underline-offset-4 hover:underline">
            {t("cli.console")}
          </Link>
        </p>
      </Card>
    </div>
  )
}
