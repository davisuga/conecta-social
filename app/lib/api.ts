import type {
  Appointment,
  Message,
  Paginated,
  Profile,
  StatsSummary,
  TriggerDescriptor,
  Unit,
} from "./types"

export const API_BASE =
  (typeof import.meta !== "undefined" &&
    (import.meta as ImportMeta & { env?: { VITE_API_BASE?: string } }).env
      ?.VITE_API_BASE) ||
  "http://localhost:8080"

class ApiError extends Error {
  status: number
  code?: string
  constructor(status: number, message: string, code?: string) {
    super(message)
    this.status = status
    this.code = code
  }
}

async function request<T>(
  path: string,
  init?: RequestInit & { query?: Record<string, unknown> }
): Promise<T> {
  const url = new URL(path, API_BASE)
  if (init?.query) {
    for (const [k, v] of Object.entries(init.query)) {
      if (v === undefined || v === null || v === "") continue
      url.searchParams.set(k, String(v))
    }
  }
  const res = await fetch(url.toString(), {
    ...init,
    headers: {
      "Content-Type": "application/json",
      ...(init?.headers ?? {}),
    },
  })
  if (!res.ok) {
    let code: string | undefined
    let message = `HTTP ${res.status}`
    try {
      const body = (await res.json()) as { error?: { code: string; message: string } }
      if (body?.error) {
        code = body.error.code
        message = body.error.message
      }
    } catch {}
    throw new ApiError(res.status, message, code)
  }
  if (res.status === 204) return undefined as T
  return (await res.json()) as T
}

export const api = {
  health: () => request<{ status: string }>("/health"),
  stats: {
    summary: () => request<StatsSummary>("/api/stats/summary"),
  },
  messages: {
    list: (params: {
      limit?: number
      offset?: number
      trigger?: string
      channel?: string
      status?: string
    } = {}) =>
      request<Paginated<Message>>("/api/messages", { query: params }),
    recent: (limit = 5) =>
      request<Message[]>("/api/messages/recent", { query: { limit } }),
    dispatch: (body: { nis: string; trigger: string }) =>
      request<Message>("/api/messages/dispatch", {
        method: "POST",
        body: JSON.stringify(body),
      }),
  },
  appointments: {
    list: (params: {
      limit?: number
      offset?: number
      service?: string
      status?: string
      unit_id?: string
    } = {}) =>
      request<Paginated<Appointment>>("/api/appointments", { query: params }),
    recent: (limit = 5) =>
      request<Appointment[]>("/api/appointments/recent", { query: { limit } }),
    updateStatus: (id: string, status: string) =>
      request<Appointment>(`/api/appointments/${id}`, {
        method: "PATCH",
        body: JSON.stringify({ status }),
      }),
  },
  profiles: {
    list: (params: { limit?: number; offset?: number } = {}) =>
      request<Paginated<Profile>>("/api/profiles", { query: params }),
    get: (nis: string) => request<Profile>(`/api/profiles/${nis}`),
    setOptIn: (nis: string, opt_in: boolean) =>
      request<Profile>(`/api/profiles/${nis}/opt-in`, {
        method: "POST",
        body: JSON.stringify({ opt_in }),
      }),
  },
  units: {
    list: () => request<Unit[]>("/api/units"),
  },
  triggers: {
    list: () => request<TriggerDescriptor[]>("/api/triggers"),
    evaluate: (body: { nis?: string } = {}) =>
      request<Message[]>("/api/triggers/evaluate", {
        method: "POST",
        body: JSON.stringify(body),
      }),
  },
}

export { ApiError }

export const queryKeys = {
  stats: { summary: ["stats", "summary"] as const },
  messages: {
    recent: (limit: number) => ["messages", "recent", limit] as const,
    list: (params: Record<string, unknown>) =>
      ["messages", "list", params] as const,
  },
  appointments: {
    recent: (limit: number) => ["appointments", "recent", limit] as const,
    list: (params: Record<string, unknown>) =>
      ["appointments", "list", params] as const,
  },
  profiles: {
    list: (params: Record<string, unknown>) =>
      ["profiles", "list", params] as const,
    detail: (nis: string) => ["profiles", "detail", nis] as const,
  },
  units: { list: ["units", "list"] as const },
  triggers: { list: ["triggers", "list"] as const },
}
