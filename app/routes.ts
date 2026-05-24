import { type RouteConfig, index, route } from "@react-router/dev/routes"

export default [
  index("routes/home.tsx"),
  route("alertas", "routes/alertas.tsx"),
  route("agendamentos", "routes/agendamentos.tsx"),
  route("perfis", "routes/perfis.tsx"),
  route("configuracoes", "routes/configuracoes.tsx"),
] satisfies RouteConfig
