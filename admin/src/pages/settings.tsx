/* eslint-disable react-hooks/set-state-in-effect */
import { useEffect, useState } from "react"
import {
  Check,
  Copy,
  Download,
  KeyRound,
  Lock,
  Palette,
  Plus,
  RotateCcw,
  Sliders,
  Trash2,
  User,
} from "lucide-react"
import { SectionCard } from "@/common/section-card"
import { PageContainer } from "@/components/layout/page-container"
import { PageHeader } from "@/components/layout/page-header"
import { PageShell } from "@/components/layout/page-shell"
import { SettingsSaveBar } from "@/components/settings/SettingsSaveBar"
import { SettingsSectionNav, type SettingsSection } from "@/components/settings/SettingsSectionNav"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Select } from "@/components/ui/select"
import { api } from "@/lib/api"
import { t, useI18n } from "@/lib/i18n"
import { toast } from "sonner"

interface PatTokenItem {
  id: string
  name: string
  token_prefix: string
  expires_at: string | null
  created_at: string
  last_used_at: string | null
}

const SECTIONS: SettingsSection[] = [
  { id: "account", label: "settings.account", icon: User },
  { id: "tokens", label: "settings.tokens", icon: KeyRound },
  { id: "appearance", label: "settings.appearance", icon: Palette },
]

export default function SettingsPage() {
  const { language, setLanguage } = useI18n()
  const [activeSection, setActiveSection] = useState("account")
  const [email, setEmail] = useState("")
  const [username, setUsername] = useState("")
  const [passwordDialogOpen, setPasswordDialogOpen] = useState(false)

  // PAT (Personal Access Tokens) state
  const [tokens, setTokens] = useState<PatTokenItem[]>([])
  const [tokensLoading, setTokensLoading] = useState(false)
  const [createTokenOpen, setCreateTokenOpen] = useState(false)
  const [newTokenName, setNewTokenName] = useState("")
  const [newTokenDays, setNewTokenDays] = useState("30")
  const [createTokenPending, setCreateTokenPending] = useState(false)
  const [revealedToken, setRevealedToken] = useState<{
    id: string
    name: string
    token: string
  } | null>(null)
  const [tokenCopied, setTokenCopied] = useState(false)
  const [tokenToDelete, setTokenToDelete] = useState<string | null>(null)
  const [deletePending, setDeletePending] = useState(false)

  // Appearance settings state & draft state
  const [isEditingAppearance, setIsEditingAppearance] = useState(false)
  const [draftLang, setDraftLang] = useState(language)
  const [appearanceSaving, setAppearanceSaving] = useState(false)
  const [appearanceSaveMessage, setAppearanceSaveMessage] = useState<{
    type: "success" | "error"
    text: string
  } | null>(null)

  // Password change state
  const [currentPassword, setCurrentPassword] = useState("")
  const [newPassword, setNewPassword] = useState("")
  const [passwordPending, setPasswordPending] = useState(false)

  useEffect(() => {
    void api<{ email: string; username?: string }>("/api/v1/me")
      .then((me) => {
        setEmail(me.email)
        setUsername(me.username || "")
      })
      .catch(() => {
        setEmail("")
        setUsername("")
      })
  }, [])

  function fetchTokens() {
    setTokensLoading(true)
    api<{ tokens: PatTokenItem[] }>("/api/v1/me/tokens")
      .then((res) => setTokens(res.tokens))
      .catch(() => setTokens([]))
      .finally(() => setTokensLoading(false))
  }

  useEffect(() => {
    if (activeSection === "tokens") {
      fetchTokens()
    }
  }, [activeSection])

  function handleStartEditAppearance() {
    setDraftLang(language)
    setIsEditingAppearance(true)
    setAppearanceSaveMessage(null)
  }

  function handleCancelEditAppearance() {
    setDraftLang(language)
    setIsEditingAppearance(false)
    setAppearanceSaveMessage(null)
  }

  function handleSaveAppearance() {
    setAppearanceSaving(true)
    setTimeout(() => {
      setLanguage(draftLang === "zh" ? "zh" : "en")
      setAppearanceSaving(false)
      setIsEditingAppearance(false)
      setAppearanceSaveMessage({
        type: "success",
        text: t("settings.savedSuccess"),
      })
      setTimeout(() => setAppearanceSaveMessage(null), 3000)
    }, 200)
  }

  function handleSectionChange(sectionId: string) {
    setIsEditingAppearance(false)
    setActiveSection(sectionId)
    setAppearanceSaveMessage(null)
  }

  async function handlePasswordChange(e: React.FormEvent) {
    e.preventDefault()
    if (!currentPassword || !newPassword) return
    setPasswordPending(true)
    try {
      await api("/api/v1/me/password", {
        method: "POST",
        body: JSON.stringify({
          current_password: currentPassword,
          new_password: newPassword,
        }),
      })
      toast.success(t("settings.passwordChanged"))
      setCurrentPassword("")
      setNewPassword("")
      setPasswordDialogOpen(false)
    } catch (err) {
      toast.error(err instanceof Error ? err.message : t("common.error"))
    } finally {
      setPasswordPending(false)
    }
  }

  async function handleCreateToken(e: React.FormEvent) {
    e.preventDefault()
    if (!newTokenName.trim()) return
    setCreateTokenPending(true)
    try {
      const days = parseInt(newTokenDays, 10)
      const res = await api<{
        id: string
        name: string
        token: string
        token_prefix: string
        expires_at: string | null
        created_at: string
      }>("/api/v1/me/tokens", {
        method: "POST",
        body: JSON.stringify({
          name: newTokenName.trim(),
          expires_in_days: days > 0 ? days : null,
        }),
      })
      setCreateTokenOpen(false)
      setNewTokenName("")
      setRevealedToken({ id: res.id, name: res.name, token: res.token })
      fetchTokens()
    } catch (err) {
      toast.error(err instanceof Error ? err.message : t("common.error"))
    } finally {
      setCreateTokenPending(false)
    }
  }

  function handleDeleteToken(tokenId: string) {
    setTokenToDelete(tokenId)
  }

  async function confirmDeleteToken() {
    if (!tokenToDelete) return
    setDeletePending(true)
    try {
      await api(`/api/v1/me/tokens/${tokenToDelete}`, { method: "DELETE" })
      toast.success(t("settings.tokenDeleted"))
      setTokenToDelete(null)
      fetchTokens()
    } catch (err) {
      toast.error(err instanceof Error ? err.message : t("common.error"))
    } finally {
      setDeletePending(false)
    }
  }

  async function copyTokenText(text: string) {
    await navigator.clipboard.writeText(text)
    setTokenCopied(true)
    toast.success(t("common.copied"))
    setTimeout(() => setTokenCopied(false), 2000)
  }

  function downloadTokenFile(tokenObj: { name: string; token: string }) {
    const blob = new Blob([tokenObj.token], { type: "text/plain;charset=utf-8" })
    const url = URL.createObjectURL(blob)
    const a = document.createElement("a")
    a.href = url
    a.download = `openhub-token-${tokenObj.name.toLowerCase().replace(/[^a-z0-9]/g, "-")}.txt`
    document.body.appendChild(a)
    a.click()
    document.body.removeChild(a)
    URL.revokeObjectURL(url)
  }

  const localizedSections = SECTIONS.map((s) => ({
    ...s,
    label: t(s.label),
  }))

  return (
    <PageShell className="overflow-y-auto">
      <PageContainer className="max-w-[1100px] pb-8 gap-4">
        <PageHeader title={t("settings.title")} className="mb-0" />

        <div className="flex flex-col gap-4">
          <SettingsSectionNav
            sections={localizedSections}
            activeSection={activeSection}
            onSectionChange={handleSectionChange}
          />

          <div className="w-full space-y-4">
            {activeSection === "account" && (
              <>
                <SectionCard
                  title={t("settings.account")}
                  description={t("settings.accountDesc")}
                  headerExtra={
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={() => setPasswordDialogOpen(true)}
                      className="h-8 gap-1.5 text-xs font-medium"
                    >
                      <Lock className="h-3.5 w-3.5 text-primary" />
                      {t("settings.password")}
                    </Button>
                  }
                >
                  <div className="space-y-4">
                    <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
                      <div className="flex flex-col justify-between rounded-lg border border-border/70 bg-card p-3.5 shadow-sm">
                        <div className="flex items-center justify-between text-xs text-muted-foreground">
                          <span className="font-medium">{t("auth.email")}</span>
                          <User className="h-3.5 w-3.5 text-muted-foreground" />
                        </div>
                        <div className="mt-2 text-sm font-semibold text-foreground truncate">
                          {email || "—"}
                        </div>
                      </div>

                      {username ? (
                        <div className="flex flex-col justify-between rounded-lg border border-border/70 bg-card p-3.5 shadow-sm">
                          <div className="flex items-center justify-between text-xs text-muted-foreground">
                            <span className="font-medium">{t("auth.username")}</span>
                            <User className="h-3.5 w-3.5 text-muted-foreground" />
                          </div>
                          <div className="mt-2 text-sm font-semibold text-foreground truncate">
                            {username}
                          </div>
                        </div>
                      ) : null}
                    </div>
                  </div>
                </SectionCard>

                <Dialog open={passwordDialogOpen} onOpenChange={setPasswordDialogOpen}>
                  <DialogContent className="max-w-md">
                    <DialogHeader>
                      <DialogTitle>{t("settings.password")}</DialogTitle>
                      <DialogDescription>{t("settings.passwordDesc")}</DialogDescription>
                    </DialogHeader>
                    <form onSubmit={handlePasswordChange} className="space-y-4">
                      <div className="space-y-2">
                        <Label htmlFor="current-password" className="text-xs font-medium">
                          {t("settings.currentPassword")}
                        </Label>
                        <Input
                          id="current-password"
                          type="password"
                          autoComplete="current-password"
                          value={currentPassword}
                          onChange={(e) => setCurrentPassword(e.target.value)}
                          className="h-9 text-sm"
                          required
                        />
                      </div>
                      <div className="space-y-2">
                        <Label htmlFor="new-password" className="text-xs font-medium">
                          {t("settings.newPassword")}
                        </Label>
                        <Input
                          id="new-password"
                          type="password"
                          autoComplete="new-password"
                          value={newPassword}
                          onChange={(e) => setNewPassword(e.target.value)}
                          className="h-9 text-sm"
                          minLength={8}
                          required
                        />
                      </div>
                      <DialogFooter>
                        <Button
                          type="button"
                          variant="outline"
                          onClick={() => setPasswordDialogOpen(false)}
                          className="h-9"
                        >
                          {t("settings.cancel")}
                        </Button>
                        <Button
                          type="submit"
                          disabled={passwordPending || !currentPassword || !newPassword}
                          className="h-9 gap-1.5"
                        >
                          <Lock className="h-3.5 w-3.5" />
                          {t("settings.password")}
                        </Button>
                      </DialogFooter>
                    </form>
                  </DialogContent>
                </Dialog>
              </>
            )}

            {activeSection === "tokens" && (
              <>
                <SectionCard
                  title={t("settings.tokens")}
                  description={t("settings.tokensDesc")}
                  headerExtra={
                    <Button
                      size="sm"
                      onClick={() => setCreateTokenOpen(true)}
                      className="h-8 gap-1.5 text-xs font-medium"
                    >
                      <Plus className="h-3.5 w-3.5" />
                      {t("settings.generateToken")}
                    </Button>
                  }
                >
                  {tokensLoading ? (
                    <div className="py-6 text-center text-xs text-muted-foreground">
                      ...
                    </div>
                  ) : tokens.length === 0 ? (
                    <div className="rounded-lg border border-dashed border-border py-8 text-center text-xs text-muted-foreground">
                      {t("settings.noTokens")}
                    </div>
                  ) : (
                    <div className="divide-y divide-border rounded-lg border border-border/70">
                      {tokens.map((tok) => (
                        <div
                          key={tok.id}
                          className="flex items-center justify-between p-3.5 hover:bg-muted/30 transition-colors"
                        >
                          <div className="min-w-0 space-y-1">
                            <div className="flex items-center gap-2">
                              <span className="text-sm font-semibold text-foreground">
                                {tok.name}
                              </span>
                              <span className="font-mono text-xs text-muted-foreground bg-muted/60 px-1.5 py-0.5 rounded">
                                {tok.token_prefix}
                              </span>
                            </div>
                            <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-xs text-muted-foreground">
                              <span>
                                {t("settings.created")}: {tok.created_at.slice(0, 10)}
                              </span>
                              <span>
                                {t("settings.lastUsed")}:{" "}
                                {tok.last_used_at
                                  ? tok.last_used_at.slice(0, 10)
                                  : t("settings.neverUsed")}
                              </span>
                              {tok.expires_at ? (
                                <span>
                                  {t("settings.expiration")}: {tok.expires_at.slice(0, 10)}
                                </span>
                              ) : null}
                            </div>
                          </div>

                          <Button
                            variant="ghost"
                            size="sm"
                            onClick={() => handleDeleteToken(tok.id)}
                            className="h-8 w-8 p-0 text-muted-foreground hover:text-destructive hover:bg-destructive/10"
                            title={t("settings.deleteToken")}
                          >
                            <Trash2 className="h-4 w-4" />
                          </Button>
                        </div>
                      ))}
                    </div>
                  )}
                </SectionCard>

                {/* Create PAT Dialog */}
                <Dialog open={createTokenOpen} onOpenChange={setCreateTokenOpen}>
                  <DialogContent className="max-w-md">
                    <DialogHeader>
                      <DialogTitle>{t("settings.generateToken")}</DialogTitle>
                      <DialogDescription>{t("settings.tokensDesc")}</DialogDescription>
                    </DialogHeader>
                    <form onSubmit={handleCreateToken} className="space-y-4">
                      <div className="space-y-2">
                        <Label htmlFor="token-name" className="text-xs font-medium">
                          {t("settings.tokenName")}
                        </Label>
                        <Input
                          id="token-name"
                          value={newTokenName}
                          onChange={(e) => setNewTokenName(e.target.value)}
                          placeholder={t("settings.tokenNamePlaceholder")}
                          className="h-9 text-sm"
                          required
                          autoFocus
                        />
                      </div>

                      <div className="space-y-2">
                        <Label htmlFor="token-expiry" className="text-xs font-medium">
                          {t("settings.expiration")}
                        </Label>
                        <Select
                          id="token-expiry"
                          value={newTokenDays}
                          onChange={(v) => setNewTokenDays(v)}
                          className="w-full"
                          options={[
                            { value: "30", label: t("settings.exp30d") },
                            { value: "60", label: t("settings.exp60d") },
                            { value: "90", label: t("settings.exp90d") },
                            { value: "0", label: t("settings.expNever") },
                          ]}
                        />
                      </div>

                      <DialogFooter>
                        <Button
                          type="button"
                          variant="outline"
                          onClick={() => setCreateTokenOpen(false)}
                          className="h-9"
                        >
                          {t("settings.cancel")}
                        </Button>
                        <Button
                          type="submit"
                          disabled={createTokenPending || !newTokenName.trim()}
                          className="h-9 gap-1.5"
                        >
                          <Plus className="h-3.5 w-3.5" />
                          {t("settings.generateToken")}
                        </Button>
                      </DialogFooter>
                    </form>
                  </DialogContent>
                </Dialog>

                {/* Single Reveal Modal for Newly Created Token */}
                <Dialog
                  open={!!revealedToken}
                  onOpenChange={(open) => {
                    if (!open) setRevealedToken(null)
                  }}
                >
                  <DialogContent className="max-w-lg">
                    <DialogHeader>
                      <DialogTitle className="flex items-center gap-2 text-foreground">
                        <Check className="h-5 w-5 text-success" />
                        {t("settings.tokenCreated")}
                      </DialogTitle>
                      <DialogDescription className="text-xs text-muted-foreground pt-1">
                        {t("settings.tokenCreatedWarning")}
                      </DialogDescription>
                    </DialogHeader>

                    {revealedToken && (
                      <div className="space-y-4 py-2">
                        <div className="rounded-lg border border-border bg-muted/40 p-3">
                          <div className="text-[11px] font-medium uppercase tracking-wider text-muted-foreground mb-1.5">
                            {revealedToken.name}
                          </div>
                          <div className="flex items-center justify-between gap-2">
                            <span className="font-mono text-sm text-foreground break-all select-all font-semibold">
                              {revealedToken.token}
                            </span>
                            <Button
                              type="button"
                              variant="outline"
                              size="sm"
                              onClick={() => copyTokenText(revealedToken.token)}
                              className="h-8 shrink-0 gap-1 text-xs"
                            >
                              {tokenCopied ? (
                                <Check className="h-3.5 w-3.5 text-success" />
                              ) : (
                                <Copy className="h-3.5 w-3.5" />
                              )}
                              {t("settings.copyToken")}
                            </Button>
                          </div>
                        </div>

                        <div className="flex items-center justify-between pt-2">
                          <Button
                            type="button"
                            variant="outline"
                            size="sm"
                            onClick={() => downloadTokenFile(revealedToken)}
                            className="h-9 gap-1.5 text-xs"
                          >
                            <Download className="h-3.5 w-3.5" />
                            {t("settings.downloadToken")}
                          </Button>
                          <Button
                            type="button"
                            onClick={() => setRevealedToken(null)}
                            className="h-9 px-5 text-xs"
                          >
                            {t("settings.done")}
                          </Button>
                        </div>
                      </div>
                    )}
                  </DialogContent>
                </Dialog>
              </>
            )}

            {activeSection === "appearance" && (
              <SectionCard
                title={t("settings.appearance")}
                description={t("settings.appearanceDesc")}
                headerExtra={
                  <div className="flex items-center gap-2">
                    {!isEditingAppearance ? (
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={handleStartEditAppearance}
                        className="h-8 gap-1.5 text-xs font-medium"
                      >
                        <Sliders className="h-3.5 w-3.5 text-primary" />
                        {t("settings.editSettings")}
                      </Button>
                    ) : (
                      <Button
                        variant="outline"
                        size="sm"
                        onClick={handleCancelEditAppearance}
                        className="h-8 gap-1.5 text-xs"
                      >
                        <RotateCcw className="h-3.5 w-3.5" />
                        {t("settings.cancelEdit")}
                      </Button>
                    )}
                  </div>
                }
              >
                {!isEditingAppearance ? (
                  /* READ-ONLY / DISPLAY MODE */
                  <div className="space-y-4">
                    <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
                      <div className="flex flex-col justify-between rounded-lg border border-border/70 bg-card p-3.5 shadow-sm">
                        <div className="flex items-center justify-between text-xs text-muted-foreground">
                          <span className="font-medium">{t("settings.language")}</span>
                          <Palette className="h-3.5 w-3.5 text-muted-foreground" />
                        </div>
                        <div className="mt-2 text-sm font-semibold text-foreground">
                          {t(language === "zh" ? "lang.zh" : "lang.en")}
                        </div>
                      </div>
                    </div>
                  </div>
                ) : (
                  /* DRAFT EDIT MODE */
                  <div className="space-y-4 max-w-lg">
                    <div className="space-y-2">
                      <Label htmlFor="language" className="text-xs font-medium">
                        {t("settings.language")}
                      </Label>
                      <Select
                        id="language"
                        value={draftLang}
                        onChange={(v) => setDraftLang(v === "zh" ? "zh" : "en")}
                        className="w-48"
                        options={[
                          { value: "en", label: t("lang.en") },
                          { value: "zh", label: t("lang.zh") },
                        ]}
                      />
                    </div>
                  </div>
                )}
              </SectionCard>
            )}

            {isEditingAppearance && (
              <SettingsSaveBar
                saving={appearanceSaving}
                message={appearanceSaveMessage}
                onReset={handleCancelEditAppearance}
                onSave={handleSaveAppearance}
                resetLabel={t("settings.cancel")}
                saveLabel={t("settings.saveChanges")}
              />
            )}
          </div>
        </div>

        {/* Delete Confirmation Dialog */}
        <Dialog open={Boolean(tokenToDelete)} onOpenChange={(open) => !open && setTokenToDelete(null)}>
          <DialogContent className="max-w-sm">
            <DialogHeader>
              <DialogTitle>{t("settings.deleteToken")}</DialogTitle>
              <DialogDescription>{t("settings.deleteTokenConfirm")}</DialogDescription>
            </DialogHeader>
            <DialogFooter className="gap-2 sm:justify-end">
              <Button
                variant="outline"
                size="sm"
                onClick={() => setTokenToDelete(null)}
                disabled={deletePending}
              >
                {t("settings.cancel")}
              </Button>
              <Button
                variant="destructive"
                size="sm"
                onClick={() => void confirmDeleteToken()}
                disabled={deletePending}
              >
                {t("settings.delete")}
              </Button>
            </DialogFooter>
          </DialogContent>
        </Dialog>
      </PageContainer>
    </PageShell>
  )
}

