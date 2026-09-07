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

export function t(key: string, paramsOrFallback?: Record<string, string | number> | string, fallback?: string): string {
  const dict = language === "zh" ? zh : en
  let text = dict[key]
  let params: Record<string, string | number> | undefined

  if (typeof paramsOrFallback === "object" && paramsOrFallback !== null) {
    params = paramsOrFallback
  } else if (typeof paramsOrFallback === "string") {
    fallback = paramsOrFallback
  }

  text = text ?? fallback ?? key

  if (params) {
    for (const [k, v] of Object.entries(params)) {
      text = text.replace(new RegExp(`\\{\\{${k}\\}\\}`, "g"), String(v))
    }
  }

  return text
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
