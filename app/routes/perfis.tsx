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
import { Badge } from "~/components/ui/badge"
import { Button } from "~/components/ui/button"
import { Switch } from "~/components/ui/switch"
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
  benefitLabel,
  formatBRL,
  formatDate,
} from "~/lib/format"

export function meta() {
  return [{ title: "Perfis — Conecta Social" }]
}

const PAGE_SIZE = 10

export default function PerfisRoute() {
  return (
    <AdminShell>
      <Perfis />
    </AdminShell>
  )
}

function Perfis() {
  const [page, setPage] = useState(1)

  const params = useMemo(
    () => ({ limit: PAGE_SIZE, offset: (page - 1) * PAGE_SIZE }),
    [page]
  )

  const { data, isLoading, isError, refetch, isFetching } = useQuery({
    queryKey: queryKeys.profiles.list(params),
    queryFn: () => api.profiles.list(params),
  })

  const queryClient = useQueryClient()

  const toggleOptIn = useMutation({
    mutationFn: ({ nis, opt_in }: { nis: string; opt_in: boolean }) =>
      api.profiles.setOptIn(nis, opt_in),
    onSuccess: (p) => {
      toast.success(
        p.opt_in
          ? `Opt-in registrado para ${p.name}.`
          : `Opt-in removido de ${p.name}.`
      )
      queryClient.invalidateQueries({ queryKey: ["profiles"] })
      queryClient.invalidateQueries({ queryKey: ["stats"] })
    },
    onError: (e: Error) => toast.error(`Falha: ${e.message}`),
  })

  const evaluate = useMutation({
    mutationFn: (nis: string) => api.triggers.evaluate({ nis }),
    onSuccess: (msgs, nis) => {
      toast.success(
        msgs.length === 0
          ? `Nenhum gatilho disparado para ${nis}.`
          : `${msgs.length} mensagens disparadas para ${nis}.`
      )
      queryClient.invalidateQueries({ queryKey: ["messages"] })
      queryClient.invalidateQueries({ queryKey: ["stats"] })
    },
    onError: (e: Error) => toast.error(`Falha: ${e.message}`),
  })

  const evaluateAll = useMutation({
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

  const total = data?.total ?? 0
  const items = data?.items ?? []

  return (
    <div className="flex flex-col gap-6">
      <div className="flex flex-col gap-1">
        <h2 className="text-2xl font-semibold tracking-tight">Perfis (Mock)</h2>
        <p className="text-sm text-muted-foreground">
          Famílias cadastradas no banco simulado do CadÚnico.
        </p>
      </div>

      <DataCard
        title={`Perfis (${total})`}
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
              onClick={() => evaluateAll.mutate()}
              disabled={evaluateAll.isPending}
            >
              <Sparkles data-icon="inline-start" />
              {evaluateAll.isPending ? "Avaliando…" : "Avaliar todos"}
            </Button>
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
              <TableHead>NIS</TableHead>
              <TableHead>Família</TableHead>
              <TableHead>Composição</TableHead>
              <TableHead>Renda per capita</TableHead>
              <TableHead>Benefícios ativos</TableHead>
              <TableHead>Última visita</TableHead>
              <TableHead className="w-24">Opt-in</TableHead>
              <TableHead className="w-32" />
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
                  Nenhum perfil cadastrado.
                </TableCell>
              </TableRow>
            )}
            {items.map((p) => (
              <TableRow key={p.nis}>
                <TableCell className="tabular-nums font-mono text-xs">
                  {p.nis}
                </TableCell>
                <TableCell>
                  <div className="flex flex-col">
                    <span className="font-medium">{p.name}</span>
                    {p.phone && (
                      <span className="text-xs text-muted-foreground">
                        {p.phone}
                      </span>
                    )}
                  </div>
                </TableCell>
                <TableCell className="text-muted-foreground">
                  <span className="text-xs">
                    {p.family.total} pessoas · {p.family.adults}a /{" "}
                    {p.family.children}c / {p.family.elderly}i
                  </span>
                </TableCell>
                <TableCell className="tabular-nums">
                  {formatBRL(p.per_capita_income)}
                </TableCell>
                <TableCell>
                  {p.active_benefits.length === 0 ? (
                    <span className="text-xs text-muted-foreground">—</span>
                  ) : (
                    <div className="flex flex-wrap gap-1">
                      {p.active_benefits.map((b) => (
                        <Badge
                          key={b}
                          variant="secondary"
                          className="bg-emerald-100 text-emerald-700"
                        >
                          {benefitLabel(b)}
                        </Badge>
                      ))}
                    </div>
                  )}
                </TableCell>
                <TableCell className="tabular-nums text-muted-foreground">
                  {formatDate(p.last_visit_at)}
                </TableCell>
                <TableCell>
                  <Switch
                    checked={p.opt_in}
                    disabled={toggleOptIn.isPending}
                    onCheckedChange={(opt_in) =>
                      toggleOptIn.mutate({ nis: p.nis, opt_in })
                    }
                    aria-label={`Opt-in ${p.name}`}
                  />
                </TableCell>
                <TableCell>
                  <Button
                    variant="outline"
                    size="sm"
                    disabled={!p.opt_in || evaluate.isPending}
                    onClick={() => evaluate.mutate(p.nis)}
                  >
                    <Sparkles data-icon="inline-start" />
                    Avaliar
                  </Button>
                </TableCell>
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </DataCard>
    </div>
  )
}
