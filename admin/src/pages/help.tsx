import { useEffect, useState } from "react"
import { Link } from "react-router-dom"
import ReactMarkdown from "react-markdown"
import remarkGfm from "remark-gfm"
import { LanguageSwitcher } from "@/components/layout/language-switcher"
import { Button } from "@/components/ui/button"
import { Card } from "@/components/ui/card"
import { getToken } from "@/lib/api"
import { t, useI18n } from "@/lib/i18n"
import { cn } from "@/lib/utils"

const DOCS = [
  { id: "guide", href: "/docs/guide.md", titleKey: "help.guide" },
  { id: "changelog", href: "/docs/changelog.md", titleKey: "help.changelog" },
] as const

type DocId = (typeof DOCS)[number]["id"]

export default function HelpPage() {
  useI18n()
  const [selectedId, setSelectedId] = useState<DocId>("guide")
  const [doc, setDoc] = useState<{ href: string; text: string } | null>(null)
  const [errorHref, setErrorHref] = useState<string | null>(null)
  const selected = DOCS.find((item) => item.id === selectedId) ?? DOCS[0]
  const signedIn = Boolean(getToken())
  const markdown = doc?.href === selected.href ? doc.text : ""
  const error = errorHref === selected.href
  const loading = !markdown && !error

  useEffect(() => {
    let cancelled = false
    const href = selected.href
    void fetch(href)
      .then((res) => {
        if (!res.ok) throw new Error(String(res.status))
        return res.text()
      })
      .then((text) => {
        if (!cancelled) {
          setDoc({ href, text })
          setErrorHref((current) => (current === href ? null : current))
        }
      })
      .catch(() => {
        if (!cancelled) setErrorHref(href)
      })
    return () => {
      cancelled = true
    }
  }, [selected.href])

  return (
    <div className="flex min-h-screen flex-col bg-background">
      <header className="flex h-14 shrink-0 items-center border-b bg-background px-4">
        <Link to="/" className="text-sm font-semibold text-foreground">
          {t("auth.productName")}
        </Link>
        <div className="ml-auto flex items-center gap-2">
          <LanguageSwitcher />
          <Button variant="outline" asChild>
            <Link to={signedIn ? "/" : "/login"}>
              {signedIn ? t("help.console") : t("auth.login")}
            </Link>
          </Button>
        </div>
      </header>

      <div className="mx-auto flex w-full max-w-6xl flex-1 min-h-0 flex-col gap-6 p-8 xl:flex-row">
        <nav className="w-full shrink-0 xl:w-56">
          <ul className="space-y-1">
            {DOCS.map((item) => (
              <li key={item.id}>
                <button
                  type="button"
                  onClick={() => setSelectedId(item.id)}
                  className={cn(
                    "flex w-full rounded-md px-3 py-2 text-left text-sm leading-5",
                    item.id === selectedId
                      ? "bg-primary/10 font-medium text-primary"
                      : "hover:bg-muted",
                  )}
                >
                  {t(item.titleKey)}
                </button>
              </li>
            ))}
          </ul>
        </nav>

        <Card className="min-h-0 flex-1 gap-0 overflow-hidden py-0">
          <div className="flex items-center justify-end border-b px-6 py-3">
            <a
              href={selected.href}
              target="_blank"
              rel="noreferrer"
              className="text-meta-sm font-medium text-primary underline-offset-4 hover:underline"
            >
              {t("help.markdownSource")}
            </a>
          </div>
          <div className="min-h-0 flex-1 overflow-y-auto p-6">
            {error ? (
              <p className="text-body-md text-muted-foreground">{t("help.loadError")}</p>
            ) : loading ? (
              <p className="text-body-md text-muted-foreground">{t("common.loading")}</p>
            ) : (
              <ReactMarkdown
                remarkPlugins={[remarkGfm]}
                components={{
                  h1: ({ children }) => (
                    <h1 className="mb-4 text-page-title text-foreground">{children}</h1>
                  ),
                  h2: ({ children }) => (
                    <h2 className="mb-2 mt-6 text-section-title text-foreground">{children}</h2>
                  ),
                  h3: ({ children }) => (
                    <h3 className="mb-2 mt-4 text-sm font-semibold text-foreground">{children}</h3>
                  ),
                  p: ({ children }) => (
                    <p className="mb-3 text-body-md leading-relaxed text-foreground">{children}</p>
                  ),
                  a: ({ href, children }) => (
                    <a
                      href={href}
                      className="text-primary underline-offset-4 hover:underline"
                    >
                      {children}
                    </a>
                  ),
                  ul: ({ children }) => (
                    <ul className="mb-3 list-disc space-y-1 pl-5 text-body-md">{children}</ul>
                  ),
                  ol: ({ children }) => (
                    <ol className="mb-3 list-decimal space-y-1 pl-5 text-body-md">{children}</ol>
                  ),
                  li: ({ children }) => <li className="leading-relaxed">{children}</li>,
                  pre: ({ children }) => (
                    <pre className="mb-3 overflow-x-auto rounded-md bg-muted/50 p-3 font-mono text-meta-sm">
                      {children}
                    </pre>
                  ),
                  code: ({ children, className }) =>
                    className ? (
                      <code className={className}>{children}</code>
                    ) : (
                      <code className="rounded bg-muted px-1 font-mono text-meta-sm">{children}</code>
                    ),
                  table: ({ children }) => (
                    <div className="mb-3 overflow-x-auto">
                      <table className="w-full border-collapse text-body-md">{children}</table>
                    </div>
                  ),
                  th: ({ children }) => (
                    <th className="border-b border-border px-2 py-1.5 text-left font-medium">
                      {children}
                    </th>
                  ),
                  td: ({ children }) => (
                    <td className="border-b border-border px-2 py-1.5 align-top">{children}</td>
                  ),
                }}
              >
                {markdown}
              </ReactMarkdown>
            )}
          </div>
        </Card>
      </div>
    </div>
  )
}
