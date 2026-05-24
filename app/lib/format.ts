import type { Channel, ServiceType, TriggerType } from "./types"

const dt = new Intl.DateTimeFormat("pt-BR", {
  day: "2-digit",
  month: "2-digit",
  year: "numeric",
  hour: "2-digit",
  minute: "2-digit",
})

const d = new Intl.DateTimeFormat("pt-BR", {
  day: "2-digit",
  month: "2-digit",
  year: "numeric",
})

const t = new Intl.DateTimeFormat("pt-BR", {
  hour: "2-digit",
  minute: "2-digit",
})

export function formatDateTime(iso?: string) {
  if (!iso) return "—"
  const date = new Date(iso)
  return Number.isNaN(date.getTime()) ? "—" : dt.format(date)
}

export function formatDate(iso?: string) {
  if (!iso) return "—"
  const date = new Date(iso)
  return Number.isNaN(date.getTime()) ? "—" : d.format(date)
}

export function formatTime(iso?: string) {
  if (!iso) return "—"
  const date = new Date(iso)
  return Number.isNaN(date.getTime()) ? "—" : t.format(date)
}

export const triggerLabel: Record<TriggerType, string> = {
  BOLSA_FAMILIA_ELEGIVEL: "Elegível ao Bolsa Família",
  RISCO_CONDICIONALIDADE: "Risco de condicionalidade",
  RECADASTRAMENTO_PROXIMO: "Recadastramento próximo",
  BPC_NAO_REQUERIDO: "BPC não requerido",
  PERFIL_SCFV: "Perfil adequado ao SCFV",
}

export const serviceLabel: Record<ServiceType, string> = {
  bolsa_familia: "Bolsa Família",
  cadastro_unico: "Cadastro Único",
  bpc: "BPC",
  outro_atendimento: "Outro atendimento",
}

export const channelLabel: Record<Channel, string> = {
  whatsapp: "WhatsApp",
  sms: "SMS",
}

export function formatPercent(value: number) {
  return `${Math.round(value * 100)}%`
}

const brl = new Intl.NumberFormat("pt-BR", {
  style: "currency",
  currency: "BRL",
})

export function formatBRL(value: number) {
  return brl.format(value)
}

const BENEFIT_LABELS: Record<string, string> = {
  bolsa_familia: "Bolsa Família",
  bpc: "BPC",
  auxilio_brasil: "Auxílio Brasil",
  scfv: "SCFV",
  cadastro_unico: "CadÚnico",
}

export function benefitLabel(key: string) {
  return BENEFIT_LABELS[key] ?? key
}
