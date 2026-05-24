import { Fragment } from "react"
import { useQuery } from "@tanstack/react-query"
import {
  CalendarCheck,
  CheckCircle2,
  ClipboardList,
  Cog,
  Database,
  MessageCircle,
  MessageSquare,
  QrCode,
  Send,
  Users,
} from "lucide-react"
import { Link } from "react-router"

import { AdminShell } from "~/components/admin-shell"
import { Badge } from "~/components/ui/badge"
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "~/components/ui/card"
import { Skeleton } from "~/components/ui/skeleton"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "~/components/ui/table"
import { api, queryKeys } from "~/lib/api"
import {
  channelLabel,
  formatDateTime,
  formatPercent,
  formatTime,
  serviceLabel,
  triggerLabel,
} from "~/lib/format"
import type { Channel, Message } from "~/lib/types"

export function meta() {
  return [{ title: "Resumo — Conecta Social" }]
}

export default function HomeRoute() {
  return (
    <AdminShell>
      <Resumo />
    </AdminShell>
  )
}

function Resumo() {
  return (
    <div className="flex flex-col gap-6">
      <div className="flex flex-col gap-1">
        <h2 className="text-2xl font-semibold tracking-tight">Resumo</h2>
        <p className="text-sm text-muted-foreground">
          Visão geral das atividades do sistema.
        </p>
      </div>

      <StatsGrid />

      <div className="grid grid-cols-1 gap-6 xl:grid-cols-2">
        <RecentMessagesCard />
        <RecentAppointmentsCard />
      </div>

      <HowItWorksCard />
    </div>
  )
}

// ---------- Stats ----------

function StatsGrid() {
  const { data, isLoading, isError } = useQuery({
    queryKey: queryKeys.stats.summary,
    queryFn: api.stats.summary,
  })

  return (
    <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 xl:grid-cols-4">
      <StatCard
        tone="emerald"
        icon={MessageSquare}
        label="Mensagens enviadas"
        value={data?.messages.total}
        sub={data ? `Hoje: ${data.messages.today}` : undefined}
        loading={isLoading}
        error={isError}
      />
      <StatCard
        tone="sky"
        icon={CalendarCheck}
        label="Agendamentos criados"
        value={data?.appointments.total}
        sub={data ? `Hoje: ${data.appointments.today}` : undefined}
        loading={isLoading}
        error={isError}
      />
      <StatCard
        tone="amber"
        icon={Users}
        label="Perfis mock ativos"
        value={data?.profiles.active}
        sub="Total cadastrados"
        loading={isLoading}
        error={isError}
      />
      <StatCard
        tone="violet"
        icon={CheckCircle2}
        label="Taxa de opt-in"
        value={data ? formatPercent(data.opt_in.rate) : undefined}
        sub={
          data
            ? `${data.opt_in.granted} de ${data.opt_in.total} perfis`
            : undefined
        }
        loading={isLoading}
        error={isError}
      />
    </div>
  )
}

const TONES = {
  emerald: {
    card: "bg-emerald-50/70 ring-emerald-200/60",
    pill: "bg-emerald-100 text-emerald-700",
    value: "text-emerald-700",
  },
  sky: {
    card: "bg-sky-50/70 ring-sky-200/60",
    pill: "bg-sky-100 text-sky-700",
    value: "text-sky-700",
  },
  amber: {
    card: "bg-amber-50/70 ring-amber-200/60",
    pill: "bg-amber-100 text-amber-700",
    value: "text-amber-700",
  },
  violet: {
    card: "bg-violet-50/70 ring-violet-200/60",
    pill: "bg-violet-100 text-violet-700",
    value: "text-violet-700",
  },
} as const

function StatCard({
  tone,
  icon: Icon,
  label,
  value,
  sub,
  loading,
  error,
}: {
  tone: keyof typeof TONES
  icon: typeof MessageSquare
  label: string
  value: number | string | undefined
  sub?: string
  loading?: boolean
  error?: boolean
}) {
  const t = TONES[tone]
  return (
    <Card className={`${t.card} ring-1`}>
      <CardContent className="flex items-start gap-4 py-4">
        <div
          className={`flex size-12 shrink-0 items-center justify-center rounded-xl ${t.pill}`}
        >
          <Icon className="size-6" />
        </div>
        <div className="flex min-w-0 flex-col gap-1">
          <span className="text-sm text-muted-foreground">{label}</span>
          {loading ? (
            <Skeleton className="h-8 w-20" />
          ) : error ? (
            <span className="text-2xl font-semibold text-muted-foreground">
              —
            </span>
          ) : (
            <span className={`text-3xl font-bold tabular-nums ${t.value}`}>
              {value ?? "—"}
            </span>
          )}
          {sub && <span className="text-xs text-muted-foreground">{sub}</span>}
        </div>
      </CardContent>
    </Card>
  )
}

// ---------- Recent messages ----------

function RecentMessagesCard() {
  const { data, isLoading, isError } = useQuery({
    queryKey: queryKeys.messages.recent(5),
    queryFn: () => api.messages.recent(5),
  })

  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between gap-4">
        <CardTitle>Alertas e mensagens recentes</CardTitle>
        <Link
          to="/alertas"
          className="text-sm font-medium text-primary hover:underline"
        >
          Ver todos
        </Link>
      </CardHeader>
      <CardContent className="p-0">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Data/Hora</TableHead>
              <TableHead>NIS</TableHead>
              <TableHead>Gatilho</TableHead>
              <TableHead className="w-16">Canal</TableHead>
              <TableHead className="w-24">Status</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {isLoading && (
              <TableSkeletonRows cols={5} rows={5} />
            )}
            {isError && (
              <TableRow>
                <TableCell colSpan={5} className="text-center text-muted-foreground">
                  Não foi possível carregar.
                </TableCell>
              </TableRow>
            )}
            {!isLoading && !isError && (!data || data.length === 0) && (
              <TableRow>
                <TableCell colSpan={5} className="text-center text-muted-foreground">
                  Nenhuma mensagem ainda.
                </TableCell>
              </TableRow>
            )}
            {data?.map((m) => (
              <TableRow key={m.id}>
                <TableCell className="tabular-nums">
                  {formatDateTime(m.sent_at ?? m.created_at)}
                </TableCell>
                <TableCell className="tabular-nums">{m.nis}</TableCell>
                <TableCell>{triggerLabel[m.trigger] ?? m.trigger}</TableCell>
                <TableCell>
                  <ChannelGlyph channel={m.channel} />
                </TableCell>
                <TableCell>
                  <StatusBadge value={m.status} kind="message" />
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
        <div className="flex items-center gap-4 border-t px-4 py-2 text-xs text-muted-foreground">
          <span className="inline-flex items-center gap-1.5">
            <ChannelGlyph channel="whatsapp" />
            WhatsApp (principal)
          </span>
          <span className="inline-flex items-center gap-1.5">
            <ChannelGlyph channel="sms" />
            SMS (fallback)
          </span>
        </div>
      </CardContent>
    </Card>
  )
}

function ChannelGlyph({ channel }: { channel: Channel }) {
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

function StatusBadge({
  value,
  kind,
}: {
  value: Message["status"] | string
  kind: "message" | "appointment"
}) {
  const ok =
    kind === "message"
      ? value === "sent" || value === "delivered"
      : value === "confirmado"
  const text =
    kind === "message"
      ? value === "sent"
        ? "Enviado"
        : value === "delivered"
          ? "Entregue"
          : value === "queued"
            ? "Na fila"
            : value === "failed"
              ? "Falhou"
              : String(value)
      : value === "confirmado"
        ? "Confirmado"
        : value === "cancelado"
          ? "Cancelado"
          : value === "concluido"
            ? "Concluído"
            : String(value)
  return (
    <Badge
      variant="secondary"
      className={
        ok
          ? "bg-emerald-100 font-medium text-emerald-700"
          : "bg-muted text-muted-foreground"
      }
    >
      {text}
    </Badge>
  )
}

// ---------- Recent appointments ----------

function RecentAppointmentsCard() {
  const { data, isLoading, isError } = useQuery({
    queryKey: queryKeys.appointments.recent(5),
    queryFn: () => api.appointments.recent(5),
  })

  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between gap-4">
        <CardTitle>Agendamentos recentes</CardTitle>
        <Link
          to="/agendamentos"
          className="text-sm font-medium text-primary hover:underline"
        >
          Ver todos
        </Link>
      </CardHeader>
      <CardContent className="p-0">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Data/Hora</TableHead>
              <TableHead>NIS</TableHead>
              <TableHead>Serviço</TableHead>
              <TableHead>Unidade</TableHead>
              <TableHead>Horário</TableHead>
              <TableHead className="w-24">Status</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {isLoading && <TableSkeletonRows cols={6} rows={5} />}
            {isError && (
              <TableRow>
                <TableCell colSpan={6} className="text-center text-muted-foreground">
                  Não foi possível carregar.
                </TableCell>
              </TableRow>
            )}
            {!isLoading && !isError && (!data || data.length === 0) && (
              <TableRow>
                <TableCell colSpan={6} className="text-center text-muted-foreground">
                  Nenhum agendamento ainda.
                </TableCell>
              </TableRow>
            )}
            {data?.map((a) => (
              <TableRow key={a.id}>
                <TableCell className="tabular-nums">
                  {formatDateTime(a.created_at)}
                </TableCell>
                <TableCell className="tabular-nums">{a.nis}</TableCell>
                <TableCell>{serviceLabel[a.service] ?? a.service}</TableCell>
                <TableCell className="text-muted-foreground">
                  {a.unit?.name ?? "—"}
                </TableCell>
                <TableCell className="tabular-nums">
                  {formatTime(a.scheduled_at)}
                </TableCell>
                <TableCell>
                  <StatusBadge value={a.status} kind="appointment" />
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </CardContent>
    </Card>
  )
}

function TableSkeletonRows({ cols, rows }: { cols: number; rows: number }) {
  return (
    <>
      {Array.from({ length: rows }).map((_, r) => (
        <TableRow key={r}>
          {Array.from({ length: cols }).map((_, c) => (
            <TableCell key={c}>
              <Skeleton className="h-4 w-full" />
            </TableCell>
          ))}
        </TableRow>
      ))}
    </>
  )
}

// ---------- How it works ----------

function HowItWorksCard() {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Como funciona o sistema</CardTitle>
      </CardHeader>
      <CardContent>
        <div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
          <FlowSection
            label="Camada 1 — Comunicação Proativa"
            tone="emerald"
            steps={[
              {
                icon: Database,
                title: "1. Dados (Mock)",
                sub: "Perfis simulados com NIS",
              },
              {
                icon: Cog,
                title: "2. Motor de Regras",
                sub: "Identifica o gatilho correto",
              },
              {
                icon: Send,
                title: "3. Disparo",
                sub: "Envia mensagem via WhatsApp ou SMS",
              },
            ]}
          />
          <FlowSection
            label="Camada 2 — Triagem Digital"
            tone="sky"
            steps={[
              {
                icon: QrCode,
                title: "1. Acesso",
                sub: "QR Code ou link WhatsApp",
              },
              {
                icon: ClipboardList,
                title: "2. 3 Perguntas",
                sub: "Triagem rápida e simples",
              },
              {
                icon: CalendarCheck,
                title: "3. Agendamento",
                sub: "Gera agendamento e envia confirmação",
              },
            ]}
          />
        </div>
      </CardContent>
    </Card>
  )
}

function FlowSection({
  label,
  tone,
  steps,
}: {
  label: string
  tone: "emerald" | "sky"
  steps: { icon: typeof Database; title: string; sub: string }[]
}) {
  const pill =
    tone === "emerald"
      ? "bg-emerald-100 text-emerald-700"
      : "bg-sky-100 text-sky-700"
  const arrow = tone === "emerald" ? "text-emerald-400" : "text-sky-400"
  return (
    <div className="flex flex-col gap-3">
      <span className="text-xs font-semibold tracking-wide text-muted-foreground uppercase">
        {label}
      </span>
      <div className="grid grid-cols-[1fr_auto_1fr_auto_1fr] items-center gap-2">
        {steps.map((s, i) => (
          <Fragment key={s.title}>
            <div className="flex flex-col items-center gap-2 text-center">
              <div
                className={`flex size-14 items-center justify-center rounded-xl ${pill}`}
              >
                <s.icon className="size-7" />
              </div>
              <span className="text-sm font-semibold">{s.title}</span>
              <span className="text-xs leading-snug text-muted-foreground">
                {s.sub}
              </span>
            </div>
            {i < steps.length - 1 && (
              <div className={`text-xl ${arrow}`}>→</div>
            )}
          </Fragment>
        ))}
      </div>
    </div>
  )
}
