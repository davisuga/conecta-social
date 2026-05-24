import { useMemo, useState } from "react"
import {
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query"
import { Ban, CheckCheck, MoreHorizontal, RefreshCw } from "lucide-react"
import { toast } from "sonner"

import { AdminShell } from "~/components/admin-shell"
import { DataCard } from "~/components/data-card"
import { PaginationBar } from "~/components/pagination-bar"
import { AppointmentStatusBadge } from "~/components/status-badge"
import { Button } from "~/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuGroup,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "~/components/ui/dropdown-menu"
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
  formatDateTime,
  formatTime,
  serviceLabel,
} from "~/lib/format"
import type { AppointmentStatus, ServiceType } from "~/lib/types"

export function meta() {
  return [{ title: "Agendamentos — Conecta SUAS" }]
}

const PAGE_SIZE = 10
const ALL = "all" as const
type Opt<T extends string> = T | typeof ALL

const SERVICE_OPTS: ServiceType[] = [
  "bolsa_familia",
  "cadastro_unico",
  "bpc",
  "outro_atendimento",
]
const STATUS_OPTS: AppointmentStatus[] = [
  "confirmado",
  "concluido",
  "cancelado",
]

export default function AgendamentosRoute() {
  return (
    <AdminShell>
      <Agendamentos />
    </AdminShell>
  )
}

function Agendamentos() {
  const [page, setPage] = useState(1)
  const [service, setService] = useState<Opt<ServiceType>>(ALL)
  const [status, setStatus] = useState<Opt<AppointmentStatus>>(ALL)
  const [unitId, setUnitId] = useState<string>(ALL)

  const units = useQuery({
    queryKey: queryKeys.units.list,
    queryFn: api.units.list,
  })

  const params = useMemo(
    () => ({
      limit: PAGE_SIZE,
      offset: (page - 1) * PAGE_SIZE,
      service: service === ALL ? undefined : service,
      status: status === ALL ? undefined : status,
      unit_id: unitId === ALL ? undefined : unitId,
    }),
    [page, service, status, unitId]
  )

  const { data, isLoading, isError, refetch, isFetching } = useQuery({
    queryKey: queryKeys.appointments.list(params),
    queryFn: () => api.appointments.list(params),
  })

  const queryClient = useQueryClient()
  const updateStatus = useMutation({
    mutationFn: ({ id, status }: { id: string; status: AppointmentStatus }) =>
      api.appointments.updateStatus(id, status),
    onSuccess: () => {
      toast.success("Agendamento atualizado.")
      queryClient.invalidateQueries({ queryKey: ["appointments"] })
      queryClient.invalidateQueries({ queryKey: ["stats"] })
    },
    onError: (e: Error) => toast.error(`Falha: ${e.message}`),
  })

  function reset() {
    setService(ALL)
    setStatus(ALL)
    setUnitId(ALL)
    setPage(1)
  }

  const total = data?.total ?? 0
  const items = data?.items ?? []
  const filtered = service !== ALL || status !== ALL || unitId !== ALL

  return (
    <div className="flex flex-col gap-6">
      <div className="flex flex-col gap-1">
        <h2 className="text-2xl font-semibold tracking-tight">Agendamentos</h2>
        <p className="text-sm text-muted-foreground">
          Triagens convertidas em horário no CRAS/CREAS.
        </p>
      </div>

      <DataCard
        title={`Agendamentos (${total})`}
        action={
          <Button
            variant="outline"
            onClick={() => refetch()}
            disabled={isFetching}
          >
            <RefreshCw data-icon="inline-start" />
            Atualizar
          </Button>
        }
        toolbar={
          <div className="flex flex-wrap items-center gap-2">
            <FilterSelect
              label="Serviço"
              value={service}
              onChange={(v) => {
                setService(v as Opt<ServiceType>)
                setPage(1)
              }}
              options={SERVICE_OPTS.map((s) => ({
                value: s,
                label: serviceLabel[s],
              }))}
            />
            <FilterSelect
              label="Unidade"
              value={unitId}
              onChange={(v) => {
                setUnitId(v)
                setPage(1)
              }}
              options={
                units.data?.map((u) => ({ value: u.id, label: u.name })) ?? []
              }
            />
            <FilterSelect
              label="Status"
              value={status}
              onChange={(v) => {
                setStatus(v as Opt<AppointmentStatus>)
                setPage(1)
              }}
              options={STATUS_OPTS.map((s) => ({
                value: s,
                label: statusOptLabel(s),
              }))}
            />
            {filtered && (
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
              <TableHead>Código</TableHead>
              <TableHead>NIS</TableHead>
              <TableHead>Serviço</TableHead>
              <TableHead>Unidade</TableHead>
              <TableHead>Data/Hora</TableHead>
              <TableHead>Documentos</TableHead>
              <TableHead className="w-28">Status</TableHead>
              <TableHead className="w-12" />
            </TableRow>
          </TableHeader>
          <TableBody>
            {isLoading && (
              <>
                {Array.from({ length: 6 }).map((_, r) => (
                  <TableRow key={r}>
                    {Array.from({ length: 8 }).map((__, c) => (
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
                <TableCell colSpan={8} className="py-10 text-center text-muted-foreground">
                  Não foi possível carregar.
                </TableCell>
              </TableRow>
            )}
            {!isLoading && !isError && items.length === 0 && (
              <TableRow>
                <TableCell colSpan={8} className="py-10 text-center text-muted-foreground">
                  Nenhum agendamento encontrado.
                </TableCell>
              </TableRow>
            )}
            {items.map((a) => (
              <TableRow key={a.id}>
                <TableCell className="font-mono text-xs">{a.code}</TableCell>
                <TableCell className="tabular-nums">{a.nis}</TableCell>
                <TableCell>{serviceLabel[a.service] ?? a.service}</TableCell>
                <TableCell>
                  <div className="flex flex-col">
                    <span>{a.unit?.name ?? "—"}</span>
                    <span className="text-xs text-muted-foreground">
                      {a.unit?.type}
                    </span>
                  </div>
                </TableCell>
                <TableCell className="tabular-nums whitespace-nowrap">
                  <div className="flex flex-col">
                    <span>{formatDateTime(a.scheduled_at)}</span>
                    <span className="text-xs text-muted-foreground">
                      às {formatTime(a.scheduled_at)}
                    </span>
                  </div>
                </TableCell>
                <TableCell className="max-w-[18rem] text-muted-foreground">
                  <span className="line-clamp-2 text-xs">
                    {a.required_documents.join(" · ")}
                  </span>
                </TableCell>
                <TableCell>
                  <AppointmentStatusBadge value={a.status} />
                </TableCell>
                <TableCell>
                  <DropdownMenu>
                    <DropdownMenuTrigger asChild>
                      <Button
                        variant="ghost"
                        size="icon-sm"
                        aria-label="Ações"
                      >
                        <MoreHorizontal />
                      </Button>
                    </DropdownMenuTrigger>
                    <DropdownMenuContent align="end">
                      <DropdownMenuGroup>
                        <DropdownMenuItem
                          disabled={
                            a.status === "concluido" ||
                            a.status === "cancelado"
                          }
                          onSelect={() =>
                            updateStatus.mutate({
                              id: a.id,
                              status: "concluido",
                            })
                          }
                        >
                          <CheckCheck />
                          Marcar como concluído
                        </DropdownMenuItem>
                        <DropdownMenuItem
                          variant="destructive"
                          disabled={a.status === "cancelado"}
                          onSelect={() =>
                            updateStatus.mutate({
                              id: a.id,
                              status: "cancelado",
                            })
                          }
                        >
                          <Ban />
                          Cancelar
                        </DropdownMenuItem>
                      </DropdownMenuGroup>
                    </DropdownMenuContent>
                  </DropdownMenu>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </DataCard>
    </div>
  )
}

function statusOptLabel(s: AppointmentStatus) {
  switch (s) {
    case "confirmado":
      return "Confirmado"
    case "cancelado":
      return "Cancelado"
    case "concluido":
      return "Concluído"
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
