import * as React from "react"
import { en } from "@/lib/locales/en"
import { zh } from "@/lib/locales/zh"

export type Lang = "en" | "zh"

const KEY = "openhub.lang"

function readLang(): Lang {
  try {
    const v = localStorage.getItem(KEY)
    if (v === "zh" || v === "en") return v
  } catch {
    /* ignore */
  }
  return "en"
}

let language: Lang = typeof window === "undefined" ? "en" : readLang()
const listeners = new Set<() => void>()

export function t(key: string, fallback?: string): string {
  const dict = language === "zh" ? zh : en
  return dict[key] ?? fallback ?? key
}

export function useI18n() {
  const [, bump] = React.useState(0)
  React.useEffect(() => {
    const fn = () => bump((n) => n + 1)
    listeners.add(fn)
    return () => {
      listeners.delete(fn)
    }
  }, [])
  return {
    language,
    setLanguage: (next: Lang) => {
      language = next
      try {
        localStorage.setItem(KEY, next)
      } catch {
        /* ignore */
      }
      listeners.forEach((fn) => fn())
    },
  }
}
