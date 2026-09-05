const TOKEN_KEY = "openhub.token"

export function getToken(): string | null {
  try {
    return localStorage.getItem(TOKEN_KEY)
  } catch {
    return null
  }
}

export function setToken(token: string | null) {
  try {
    if (token) localStorage.setItem(TOKEN_KEY, token)
    else localStorage.removeItem(TOKEN_KEY)
  } catch {
    /* ignore */
  }
}

export async function api<T = unknown>(path: string, init: RequestInit = {}): Promise<T> {
  const headers = new Headers(init.headers)
  if (init.body && !headers.has("content-type")) {
    headers.set("content-type", "application/json")
  }
  const token = getToken()
  if (token) headers.set("authorization", `Bearer ${token}`)
  const res = await fetch(path, { ...init, headers })
  const text = await res.text()
  let json: unknown = null
  if (text) {
    try {
      json = JSON.parse(text)
    } catch {
      json = { raw: text }
    }
  }
  if (!res.ok) {
    const message =
      typeof json === "object" && json && "error" in json
        ? String((json as { error?: { message?: string } }).error?.message ?? res.status)
        : String(res.status)
    throw new Error(message)
  }
  return json as T
}
