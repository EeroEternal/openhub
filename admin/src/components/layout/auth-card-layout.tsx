import type { ReactNode } from "react"
import { Link } from "react-router-dom"
import { Card } from "@/components/ui/card"
import { cn } from "@/lib/utils"
import { t, useI18n } from "@/lib/i18n"

interface AuthCardLayoutProps {
  activeTab: "register" | "login"
  title: string
  subtitle: string
  children: ReactNode
}

export function AuthCardLayout({
  activeTab,
  title,
  subtitle,
  children,
}: AuthCardLayoutProps) {
  useI18n()

  return (
    <div className="w-full max-w-4xl grid grid-cols-1 md:grid-cols-2 gap-6 items-stretch my-auto">
      {/* Left Column: Product Intro */}
      <Card className="p-8 flex flex-col justify-between bg-card/60 backdrop-blur-sm border shadow-sm">
        <div>
          {/* Logo & Platform Name */}
          <div className="flex items-center gap-3">
            <div className="flex h-10 w-10 items-center justify-center rounded-xl bg-primary text-primary-foreground shadow-sm">
              <svg
                className="h-5 w-5"
                viewBox="0 0 24 24"
                fill="none"
                stroke="currentColor"
                strokeWidth="2.2"
                strokeLinecap="round"
                strokeLinejoin="round"
              >
                <path d="M21 12a9 9 0 0 0-9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" />
                <path d="M3 3v5h5" />
                <path d="M3 12a9 9 0 0 0 9 9 9.75 9.75 0 0 0 6.74-2.74L21 16" />
                <path d="M16 21h5v-5" />
              </svg>
            </div>
            <div>
              <div className="text-base font-bold text-foreground leading-tight">OpenHub</div>
              <div className="text-[10px] font-semibold tracking-wider text-muted-foreground uppercase">
                {t("auth.introBadge")}
              </div>
            </div>
          </div>

          {/* Headline & Description */}
          <div className="mt-8 space-y-3">
            <h2 className="text-2xl font-bold tracking-tight text-foreground leading-snug">
              {t("auth.introHeading")}
            </h2>
            <p className="text-xs sm:text-sm text-muted-foreground leading-relaxed">
              {t("auth.introSubheading")}
            </p>
          </div>

          {/* Feature Points */}
          <div className="mt-8 space-y-4 border-t pt-6">
            <div className="flex items-start gap-3">
              <span className="font-mono text-xs font-bold text-primary shrink-0 mt-0.5">
                01
              </span>
              <div>
                <div className="text-xs sm:text-sm font-semibold text-foreground">
                  {t("auth.introPoint1Title")}
                </div>
                <div className="text-xs text-muted-foreground mt-0.5">
                  {t("auth.introPoint1Desc")}
                </div>
              </div>
            </div>

            <div className="border-t pt-4 flex items-start gap-3">
              <span className="font-mono text-xs font-bold text-primary shrink-0 mt-0.5">
                02
              </span>
              <div>
                <div className="text-xs sm:text-sm font-semibold text-foreground">
                  {t("auth.introPoint2Title")}
                </div>
                <div className="text-xs text-muted-foreground mt-0.5">
                  {t("auth.introPoint2Desc")}
                </div>
              </div>
            </div>

            <div className="border-t pt-4 flex items-start gap-3">
              <span className="font-mono text-xs font-bold text-primary shrink-0 mt-0.5">
                03
              </span>
              <div>
                <div className="text-xs sm:text-sm font-semibold text-foreground">
                  {t("auth.introPoint3Title")}
                </div>
                <div className="text-xs text-muted-foreground mt-0.5">
                  {t("auth.introPoint3Desc")}
                </div>
              </div>
            </div>
          </div>
        </div>

        {/* Bottom tags */}
        <div className="mt-8 pt-6 border-t flex flex-wrap gap-2">
          <span className="inline-flex items-center rounded-full border bg-muted/60 px-2.5 py-1 text-xs font-medium text-muted-foreground">
            {t("auth.tagGit")}
          </span>
          <span className="inline-flex items-center rounded-full border bg-muted/60 px-2.5 py-1 text-xs font-medium text-muted-foreground">
            {t("auth.tagSession")}
          </span>
          <span className="inline-flex items-center rounded-full border bg-muted/60 px-2.5 py-1 text-xs font-medium text-muted-foreground">
            {t("auth.tagLoop")}
          </span>
        </div>
      </Card>

      {/* Right Column: Active Card */}
      <Card className="p-8 flex flex-col justify-between bg-card border shadow-sm">
        <div>
          {/* Segmented Switcher */}
          <div className="grid grid-cols-2 rounded-lg bg-muted p-1 mb-8 text-sm font-medium">
            <Link
              to="/register"
              className={cn(
                "rounded-md py-1.5 text-center transition-all",
                activeTab === "register"
                  ? "bg-background text-foreground shadow-sm font-semibold"
                  : "text-muted-foreground hover:text-foreground"
              )}
            >
              {t("auth.registerTab")}
            </Link>
            <Link
              to="/login"
              className={cn(
                "rounded-md py-1.5 text-center transition-all",
                activeTab === "login"
                  ? "bg-background text-foreground shadow-sm font-semibold"
                  : "text-muted-foreground hover:text-foreground"
              )}
            >
              {t("auth.loginTab")}
            </Link>
          </div>

          {/* Heading */}
          <div className="mb-6 space-y-1.5">
            <h1 className="text-2xl font-bold tracking-tight text-foreground">{title}</h1>
            <p className="text-xs sm:text-sm text-muted-foreground leading-relaxed">{subtitle}</p>
          </div>

          {/* Body */}
          {children}
        </div>

        {/* Footer Notice */}
        <p className="text-[11px] text-muted-foreground/70 text-center mt-6">
          {t("auth.termsNotice")}
        </p>
      </Card>
    </div>
  )
}
