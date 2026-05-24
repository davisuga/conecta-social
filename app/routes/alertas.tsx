import { useMemo, useState } from "react"
import {
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query"
import { RefreshCw, Sparkles } from "lucide-react"
import { toast } from "sonner"

import { AdminShell } from "~/components/admin-shell"
import { DataCard } from "~/components/data-card"
import { PaginationBar } from "~/components/pagination-bar"
import {
  ChannelGlyph,
  MessageStatusBadge,
} from "~/components/status-badge"
import { Button } from "~/components/ui/button"
import {
  Select,
  SelectContent,
  SelectGroup,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "~/components/ui/select"
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "~/components/ui/table"
import { Skeleton } from "~/components/ui/skeleton"
import { api, queryKeys } from "~/lib/api"
import {
  channelLabel,
  formatDateTime,
  triggerLabel,
} from "~/lib/format"
import type {
  Channel,
  MessageStatus,
  TriggerType,
} from "~/lib/types"

export function meta() {
  return [{ title: "Alertas e Mensagens — Conecta SUAS" }]
}

const PAGE_SIZE = 10
const ALL = "all" as const
type Opt<T extends string> = T | typeof ALL

const TRIGGER_OPTS: TriggerType[] = [
  "BOLSA_FAMILIA_ELEGIVEL",
  "RISCO_CONDICIONALIDADE",
  "RECADASTRAMENTO_PROXIMO",
  "BPC_NAO_REQUERIDO",
  "PERFIL_SCFV",
]
const CHANNEL_OPTS: Channel[] = ["whatsapp", "sms"]
const STATUS_OPTS: MessageStatus[] = ["queued", "sent", "delivered", "failed"]

export default function AlertasRoute() {
  return (
    <AdminShell>
      <Alertas />
    </AdminShell>
  )
}

function Alertas() {
  const [page, setPage] = useState(1)
  const [trigger, setTrigger] = useState<Opt<TriggerType>>(ALL)
  const [channel, setChannel] = useState<Opt<Channel>>(ALL)
  const [status, setStatus] = useState<Opt<MessageStatus>>(ALL)

  const params = useMemo(
    () => ({
      limit: PAGE_SIZE,
      offset: (page - 1) * PAGE_SIZE,
      trigger: trigger === ALL ? undefined : trigger,
      channel: channel === ALL ? undefined : channel,
      status: status === ALL ? undefined : status,
    }),
    [page, trigger, channel, status]
  )

  const { data, isLoading, isError, refetch, isFetching } = useQuery({
    queryKey: queryKeys.messages.list(params),
    queryFn: () => api.messages.list(params),
  })

  const queryClient = useQueryClient()
  const evaluate = useMutation({
    mutationFn: () => api.triggers.evaluate(),
    onSuccess: (msgs) => {
      toast.success(
        msgs.length === 0
          ? "Nenhum gatilho disparado."
          : `${msgs.length} mensagens disparadas.`
      )
      queryClient.invalidateQueries({ queryKey: ["messages"] })
      queryClient.invalidateQueries({ queryKey: ["stats"] })
    },
    onError: (e: Error) => toast.error(`Falha: ${e.message}`),
  })

  function reset() {
    setTrigger(ALL)
    setChannel(ALL)
    setStatus(ALL)
    setPage(1)
  }

  function setFilter<T extends string>(
    setter: (v: Opt<T>) => void
  ): (v: string) => void {
    return (v) => {
      setter(v as Opt<T>)
      setPage(1)
    }
  }

  const total = data?.total ?? 0
  const items = data?.items ?? []

  return (
    <div className="flex flex-col gap-6">
      <div className="flex flex-col gap-1">
        <h2 className="text-2xl font-semibold tracking-tight">
          Alertas e Mensagens
        </h2>
        <p className="text-sm text-muted-foreground">
          Histórico de disparos do motor de regras.
        </p>
      </div>

      <DataCard
        title={`Mensagens (${total})`}
        action={
          <div className="flex items-center gap-2">
            <Button
              variant="outline"
              onClick={() => refetch()}
              disabled={isFetching}
            >
              <RefreshCw data-icon="inline-start" />
              Atualizar
            </Button>
            <Button
              onClick={() => evaluate.mutate()}
              disabled={evaluate.isPending}
            >
              <Sparkles data-icon="inline-start" />
              {evaluate.isPending ? "Avaliando…" : "Reavaliar gatilhos"}
            </Button>
          </div>
        }
        toolbar={
          <div className="flex flex-wrap items-center gap-2">
            <FilterSelect
              label="Gatilho"
              value={trigger}
              onChange={setFilter<TriggerType>(setTrigger)}
              options={TRIGGER_OPTS.map((t) => ({
                value: t,
                label: triggerLabel[t],
              }))}
            />
            <FilterSelect
              label="Canal"
              value={channel}
              onChange={setFilter<Channel>(setChannel)}
              options={CHANNEL_OPTS.map((c) => ({
                value: c,
                label: channelLabel[c],
              }))}
            />
            <FilterSelect
              label="Status"
              value={status}
              onChange={setFilter<MessageStatus>(setStatus)}
              options={STATUS_OPTS.map((s) => ({
                value: s,
                label: statusOptLabel(s),
              }))}
            />
            {(trigger !== ALL ||
              channel !== ALL ||
              status !== ALL) && (
              <Button variant="ghost" size="sm" onClick={reset}>
                Limpar filtros
              </Button>
            )}
          </div>
        }
        footer={
          <PaginationBar
            page={page}
            pageSize={PAGE_SIZE}
            total={total}
            onPageChange={setPage}
          />
        }
      >
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>Data/Hora</TableHead>
              <TableHead>NIS</TableHead>
              <TableHead>Gatilho</TableHead>
              <TableHead>Mensagem</TableHead>
              <TableHead className="w-16">Canal</TableHead>
              <TableHead className="w-24">Status</TableHead>
            </TableRow>
          </TableHeader>
          <TableBody>
            {isLoading && (
              <>
                {Array.from({ length: 6 }).map((_, r) => (
                  <TableRow key={r}>
                    {Array.from({ length: 6 }).map((__, c) => (
                      <TableCell key={c}>
                        <Skeleton className="h-4 w-full" />
                      </TableCell>
                    ))}
                  </TableRow>
                ))}
              </>
            )}
            {isError && (
              <TableRow>
                <TableCell colSpan={6} className="py-10 text-center text-muted-foreground">
                  Não foi possível carregar.
                </TableCell>
              </TableRow>
            )}
            {!isLoading && !isError && items.length === 0 && (
              <TableRow>
                <TableCell colSpan={6} className="py-10 text-center text-muted-foreground">
                  Nenhuma mensagem encontrada.
                </TableCell>
              </TableRow>
            )}
            {items.map((m) => (
              <TableRow key={m.id}>
                <TableCell className="tabular-nums whitespace-nowrap">
                  {formatDateTime(m.sent_at ?? m.created_at)}
                </TableCell>
                <TableCell className="tabular-nums">{m.nis}</TableCell>
                <TableCell>{triggerLabel[m.trigger] ?? m.trigger}</TableCell>
                <TableCell className="max-w-[28rem] truncate text-muted-foreground">
                  {m.body}
                </TableCell>
                <TableCell>
                  <ChannelGlyph channel={m.channel} />
                </TableCell>
                <TableCell>
                  <MessageStatusBadge value={m.status} />
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </DataCard>
    </div>
  )
}

function statusOptLabel(s: MessageStatus) {
  switch (s) {
    case "queued":
      return "Na fila"
    case "sent":
      return "Enviado"
    case "delivered":
      return "Entregue"
    case "failed":
      return "Falhou"
  }
}

function FilterSelect({
  label,
  value,
  onChange,
  options,
}: {
  label: string
  value: string
  onChange: (v: string) => void
  options: { value: string; label: string }[]
}) {
  return (
    <Select value={value} onValueChange={onChange}>
      <SelectTrigger size="sm" className="min-w-[10rem]">
        <SelectValue placeholder={label} />
      </SelectTrigger>
      <SelectContent>
        <SelectGroup>
          <SelectItem value="all">{label}: todos</SelectItem>
          {options.map((o) => (
            <SelectItem key={o.value} value={o.value}>
              {o.label}
            </SelectItem>
          ))}
        </SelectGroup>
      </SelectContent>
    </Select>
  )
}
