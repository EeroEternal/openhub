import { useEffect, useState } from "react"
import { PageContainer } from "@/components/layout/page-container"
import { PageHeader } from "@/components/layout/page-header"
import { PageShell } from "@/components/layout/page-shell"
import { Button } from "@/components/ui/button"
import { Select } from "@/components/ui/select"
import { api } from "@/lib/api"
import { t, useI18n } from "@/lib/i18n"
import { cn } from "@/lib/utils"

type Section = "account" | "appearance"

export default function SettingsPage() {
  const { language, setLanguage } = useI18n()
  const [section, setSection] = useState<Section>("account")
  const [email, setEmail] = useState("")
  const [editing, setEditing] = useState(false)
  const [draftLang, setDraftLang] = useState(language)

  useEffect(() => {
    void api<{ email: string }>("/api/v1/me")
      .then((me) => setEmail(me.email))
      .catch(() => setEmail(""))
  }, [])

  function startEdit() {
    setDraftLang(language)
    setEditing(true)
  }

  function cancelEdit() {
    setDraftLang(language)
    setEditing(false)
  }

  function saveEdit() {
    setLanguage(draftLang === "zh" ? "zh" : "en")
    setEditing(false)
  }

  return (
    <PageShell className="overflow-y-auto">
      <PageContainer>
        <PageHeader title={t("settings.title")} />
        <nav className="mb-6 flex h-10 items-end gap-6 border-b border-border">
          {(["account", "appearance"] as const).map((item) => (
            <button
              key={item}
              type="button"
              className={cn(
                "-mb-px border-b-2 pb-2 text-sm",
                section === item
                  ? "border-primary font-medium text-primary"
                  : "border-transparent text-muted-foreground"
              )}
              onClick={() => {
                setSection(item)
                setEditing(false)
              }}
            >
              {t(item === "account" ? "settings.account" : "settings.appearance")}
            </button>
          ))}
        </nav>

        {section === "account" ? (
          <div className="max-w-2xl py-2">
            <div className="flex h-10 items-center justify-between gap-4">
              <span className="text-sm">{t("auth.email")}</span>
              <span className="text-sm text-foreground">{email}</span>
            </div>
          </div>
        ) : (
          <div className="max-w-2xl py-2">
            {editing ? (
              <>
                <div className="flex h-10 items-center justify-between gap-4">
                  <span className="text-sm">{t("settings.language")}</span>
                  <Select
                    value={draftLang}
                    onChange={(v) => setDraftLang(v === "zh" ? "zh" : "en")}
                    className="w-48"
                    options={[
                      { value: "en", label: t("lang.en") },
                      { value: "zh", label: t("lang.zh") },
                    ]}
                  />
                </div>
                <div className="sticky bottom-0 mt-6 flex h-10 items-center justify-end gap-2 bg-muted/40">
                  <Button type="button" variant="outline" onClick={cancelEdit}>
                    {t("settings.cancel")}
                  </Button>
                  <Button type="button" onClick={saveEdit}>
                    {t("settings.save")}
                  </Button>
                </div>
              </>
            ) : (
              <div className="flex h-10 items-center justify-between gap-4">
                <span className="text-sm">{t("settings.language")}</span>
                <div className="flex items-center gap-2">
                  <span className="text-sm">{t(language === "zh" ? "lang.zh" : "lang.en")}</span>
                  <Button type="button" variant="outline" onClick={startEdit}>
                    {t("settings.edit")}
                  </Button>
                </div>
              </div>
            )}
          </div>
        )}
      </PageContainer>
    </PageShell>
  )
}
