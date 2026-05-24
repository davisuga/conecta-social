import {
  useMutation,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query"
import {
  CheckCircle2,
  CircleAlert,
  MessageCircle,
  MessageSquare,
  ShieldCheck,
  Sparkles,
} from "lucide-react"
import { toast } from "sonner"

import { AdminShell } from "~/components/admin-shell"
import { Badge } from "~/components/ui/badge"
import { Button } from "~/components/ui/button"
import {
  Card,
  CardContent,
  CardDescription,
  CardHeader,
  CardTitle,
} from "~/components/ui/card"
import { Separator } from "~/components/ui/separator"
import { Skeleton } from "~/components/ui/skeleton"
import { API_BASE, api } from "~/lib/api"
import { triggerLabel } from "~/lib/format"

export function meta() {
  return [{ title: "Configurações — Conecta SUAS" }]
}

export default function ConfiguracoesRoute() {
  return (
    <AdminShell>
      <Configuracoes />
    </AdminShell>
  )
}

function Configuracoes() {
  return (
    <div className="flex flex-col gap-6">
      <div className="flex flex-col gap-1">
        <h2 className="text-2xl font-semibold tracking-tight">Configurações</h2>
        <p className="text-sm text-muted-foreground">
          Ajustes do MVP e estado das integrações.
        </p>
      </div>

      <div className="grid grid-cols-1 gap-6 lg:grid-cols-2">
        <ConnectionCard />
        <ChannelsCard />
        <TriggersCard />
        <ActionsCard />
      </div>
    </div>
  )
}

function ConnectionCard() {
  const health = useQuery({
    queryKey: ["health"],
    queryFn: api.health,
    retry: 0,
    refetchInterval: 30_000,
  })
  const ok = health.data?.status === "ok"
  return (
    <Card>
      <CardHeader>
        <CardTitle>Conexão com a API</CardTitle>
        <CardDescription>
          Endpoint usado por este painel.
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        <div className="flex items-center justify-between gap-4">
          <span className="text-sm text-muted-foreground">Base URL</span>
          <code className="rounded-md bg-muted px-2 py-1 font-mono text-xs">
            {API_BASE}
          </code>
        </div>
        <div className="flex items-center justify-between gap-4">
          <span className="text-sm text-muted-foreground">Status</span>
          {health.isLoading ? (
            <Skeleton className="h-6 w-24" />
          ) : ok ? (
            <Badge
              variant="secondary"
              className="bg-emerald-100 font-medium text-emerald-700"
            >
              <CheckCircle2 className="size-3.5" />
              Online
            </Badge>
          ) : (
            <Badge
              variant="secondary"
              className="bg-red-100 font-medium text-red-700"
            >
              <CircleAlert className="size-3.5" />
              Offline
            </Badge>
          )}
        </div>
        <Separator />
        <p className="text-xs text-muted-foreground">
          Para apontar para outro backend, defina{" "}
          <code className="font-mono">VITE_API_BASE</code> no ambiente do
          frontend.
        </p>
      </CardContent>
    </Card>
  )
}

function ChannelsCard() {
  return (
    <Card>
      <CardHeader>
        <CardTitle>Canais de envio</CardTitle>
        <CardDescription>
          WhatsApp como principal, SMS como fallback.
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-3">
        <ChannelRow
          icon={MessageCircle}
          tone="emerald"
          name="WhatsApp"
          tag="Principal"
        />
        <ChannelRow
          icon={MessageSquare}
          tone="sky"
          name="SMS"
          tag="Fallback"
        />
        <Separator />
        <p className="text-xs text-muted-foreground">
          Integração simulada para o MVP. Em produção, plugar provider
          (Twilio / Z-API / Evolution).
        </p>
      </CardContent>
    </Card>
  )
}

function ChannelRow({
  icon: Icon,
  tone,
  name,
  tag,
}: {
  icon: typeof MessageCircle
  tone: "emerald" | "sky"
  name: string
  tag: string
}) {
  const pill =
    tone === "emerald"
      ? "bg-emerald-100 text-emerald-700"
      : "bg-sky-100 text-sky-700"
  return (
    <div className="flex items-center justify-between gap-4 rounded-lg ring-1 ring-border px-3 py-2">
      <div className="flex items-center gap-3">
        <span
          className={`flex size-9 items-center justify-center rounded-lg ${pill}`}
        >
          <Icon className="size-4" />
        </span>
        <div className="flex flex-col">
          <span className="text-sm font-medium">{name}</span>
          <span className="text-xs text-muted-foreground">{tag}</span>
        </div>
      </div>
      <Badge
        variant="secondary"
        className="bg-emerald-100 font-medium text-emerald-700"
      >
        Ativo
      </Badge>
    </div>
  )
}

function TriggersCard() {
  const triggers = useQuery({
    queryKey: ["triggers", "list"],
    queryFn: api.triggers.list,
  })
  return (
    <Card>
      <CardHeader>
        <CardTitle>Gatilhos do motor de regras</CardTitle>
        <CardDescription>
          Eventos que disparam mensagem proativa.
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-2">
        {triggers.isLoading && (
          <>
            {Array.from({ length: 5 }).map((_, i) => (
              <Skeleton key={i} className="h-12 w-full" />
            ))}
          </>
        )}
        {triggers.isError && (
          <p className="text-sm text-muted-foreground">
            Não foi possível carregar gatilhos.
          </p>
        )}
        {triggers.data?.map((t) => (
          <div
            key={t.type}
            className="flex flex-col gap-1 rounded-lg ring-1 ring-border px-3 py-2"
          >
            <span className="text-sm font-medium">
              {triggerLabel[t.type] ?? t.label}
            </span>
            <span className="text-xs leading-snug text-muted-foreground">
              {t.description}
            </span>
          </div>
        ))}
        {!triggers.isLoading && !triggers.isError && triggers.data?.length === 0 && (
          <p className="text-sm text-muted-foreground">
            Nenhum gatilho registrado.
          </p>
        )}
      </CardContent>
    </Card>
  )
}

function ActionsCard() {
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
  return (
    <Card>
      <CardHeader>
        <CardTitle>Ações e LGPD</CardTitle>
        <CardDescription>
          Operações administrativas e conformidade.
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4">
        <div className="flex flex-col gap-2 rounded-xl bg-secondary/60 p-3 ring-1 ring-primary/10">
          <div className="flex items-center gap-2">
            <ShieldCheck className="size-4 text-primary" />
            <span className="text-sm font-semibold text-primary">LGPD</span>
          </div>
          <p className="text-xs leading-snug text-muted-foreground">
            Opt-in coletado presencialmente. Dado mínimo necessário, finalidade
            declarada, consentimento registrado por perfil.
          </p>
        </div>
        <Button
          onClick={() => evaluate.mutate()}
          disabled={evaluate.isPending}
          className="w-full"
        >
          <Sparkles data-icon="inline-start" />
          {evaluate.isPending
            ? "Avaliando…"
            : "Reavaliar gatilhos para todos os perfis"}
        </Button>
        <p className="text-xs text-muted-foreground">
          Roda o motor de regras sobre todos os perfis com opt-in ativo e
          dispara mensagens para gatilhos novos.
        </p>
      </CardContent>
    </Card>
  )
}
