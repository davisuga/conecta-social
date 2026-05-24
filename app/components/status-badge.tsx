import { MessageCircle, MessageSquare } from "lucide-react"
import { Badge } from "~/components/ui/badge"
import { channelLabel } from "~/lib/format"
import type { AppointmentStatus, Channel, MessageStatus } from "~/lib/types"

const MESSAGE_LABELS: Record<MessageStatus, string> = {
  queued: "Na fila",
  sent: "Enviado",
  delivered: "Entregue",
  failed: "Falhou",
}

const APPOINTMENT_LABELS: Record<AppointmentStatus, string> = {
  confirmado: "Confirmado",
  cancelado: "Cancelado",
  concluido: "Concluído",
}

export function MessageStatusBadge({ value }: { value: MessageStatus }) {
  const ok = value === "sent" || value === "delivered"
  const bad = value === "failed"
  return (
    <Badge
      variant="secondary"
      className={
        bad
          ? "bg-red-100 font-medium text-red-700"
          : ok
            ? "bg-emerald-100 font-medium text-emerald-700"
            : "bg-amber-100 font-medium text-amber-700"
      }
    >
      {MESSAGE_LABELS[value] ?? value}
    </Badge>
  )
}

export function AppointmentStatusBadge({ value }: { value: AppointmentStatus }) {
  const tone =
    value === "confirmado"
      ? "bg-emerald-100 text-emerald-700"
      : value === "concluido"
        ? "bg-sky-100 text-sky-700"
        : "bg-muted text-muted-foreground"
  return (
    <Badge variant="secondary" className={`font-medium ${tone}`}>
      {APPOINTMENT_LABELS[value] ?? value}
    </Badge>
  )
}

export function ChannelGlyph({ channel }: { channel: Channel }) {
  if (channel === "whatsapp") {
    return (
      <span
        title={channelLabel.whatsapp}
        className="inline-flex size-6 items-center justify-center rounded-full bg-emerald-100 text-emerald-700"
      >
        <MessageCircle className="size-3.5" />
      </span>
    )
  }
  return (
    <span
      title={channelLabel.sms}
      className="inline-flex size-6 items-center justify-center rounded-full bg-sky-100 text-sky-700"
    >
      <MessageSquare className="size-3.5" />
    </span>
  )
}
