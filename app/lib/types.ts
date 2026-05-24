export type Channel = "whatsapp" | "sms"
export type MessageStatus = "queued" | "sent" | "delivered" | "failed"
export type TriggerType =
  | "BOLSA_FAMILIA_ELEGIVEL"
  | "RISCO_CONDICIONALIDADE"
  | "RECADASTRAMENTO_PROXIMO"
  | "BPC_NAO_REQUERIDO"
  | "PERFIL_SCFV"
export type ServiceType =
  | "bolsa_familia"
  | "cadastro_unico"
  | "bpc"
  | "outro_atendimento"
export type AppointmentStatus = "confirmado" | "cancelado" | "concluido"

export interface Profile {
  nis: string
  cpf?: string
  name: string
  phone?: string
  family: { adults: number; children: number; elderly: number; total: number }
  per_capita_income: number
  active_benefits: string[]
  opt_in: boolean
  opt_in_at?: string
  last_visit_at?: string
  created_at: string
  updated_at: string
}

export interface Message {
  id: string
  nis: string
  trigger: TriggerType
  channel: Channel
  status: MessageStatus
  body: string
  sent_at?: string
  created_at: string
}

export interface Unit {
  id: string
  name: string
  address: string
  type: "CRAS" | "CREAS"
}

export interface Appointment {
  id: string
  code: string
  nis: string
  service: ServiceType
  unit: Unit
  scheduled_at: string
  required_documents: string[]
  status: AppointmentStatus
  created_at: string
}

export interface StatsSummary {
  messages: { total: number; today: number }
  appointments: { total: number; today: number }
  profiles: { active: number }
  opt_in: { rate: number; granted: number; total: number }
}

export interface Paginated<T> {
  items: T[]
  total: number
}

export interface TriggerDescriptor {
  type: TriggerType
  label: string
  description: string
}
