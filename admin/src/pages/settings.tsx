import { useEffect, useState } from "react"
import { useNavigate } from "react-router-dom"
import { SectionCard } from "@/common/section-card"
import { PageContainer } from "@/components/layout/page-container"
import { PageHeader } from "@/components/layout/page-header"
import { PageShell } from "@/components/layout/page-shell"
import { Button } from "@/components/ui/button"
import { Select } from "@/components/ui/select"
import { api, setToken } from "@/lib/api"
import { t, useI18n } from "@/lib/i18n"

export default function SettingsPage() {
  const { language, setLanguage } = useI18n()
  const navigate = useNavigate()
  const [email, setEmail] = useState("")

  useEffect(() => {
    void api<{ email: string }>("/api/v1/me")
      .then((me) => setEmail(me.email))
      .catch(() => setEmail(""))
  }, [])

  return (
    <PageShell className="overflow-y-auto">
      <PageContainer>
        <PageHeader title={t("settings.title")} />
        <div className="space-y-4">
          <SectionCard title={t("settings.account")}>
            <p className="text-body-md text-foreground">{email}</p>
            <Button
              className="mt-4"
              variant="outline"
              onClick={() => {
                setToken(null)
                navigate("/login", { replace: true })
              }}
            >
              {t("settings.logout")}
            </Button>
          </SectionCard>
          <SectionCard title={t("settings.language")}>
            <Select
              value={language}
              onChange={(v) => setLanguage(v === "zh" ? "zh" : "en")}
              className="w-48"
              options={[
                { value: "en", label: t("lang.en") },
                { value: "zh", label: t("lang.zh") },
              ]}
            />
          </SectionCard>
        </div>
      </PageContainer>
    </PageShell>
  )
}
