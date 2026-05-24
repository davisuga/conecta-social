import { NavLink, useLocation } from "react-router"
import {
  CalendarDays,
  Home,
  Send,
  Settings,
  ShieldCheck,
  Users,
} from "lucide-react"

import {
  Sidebar,
  SidebarContent,
  SidebarFooter,
  SidebarGroup,
  SidebarGroupContent,
  SidebarHeader,
  SidebarInset,
  SidebarMenu,
  SidebarMenuButton,
  SidebarMenuItem,
  SidebarProvider,
  SidebarTrigger,
} from "~/components/ui/sidebar"
import { Badge } from "~/components/ui/badge"
import { Separator } from "~/components/ui/separator"
import { BrandMark } from "~/components/brand-mark"

type NavItem = {
  to: string
  label: string
  icon: typeof Home
  end?: boolean
}

const NAV: NavItem[] = [
  { to: "/", label: "Resumo", icon: Home, end: true },
  { to: "/alertas", label: "Alertas e Mensagens", icon: Send },
  { to: "/agendamentos", label: "Agendamentos", icon: CalendarDays },
  { to: "/perfis", label: "Perfis (Mock)", icon: Users },
  { to: "/configuracoes", label: "Configurações", icon: Settings },
]

function pageTitle(pathname: string) {
  if (pathname === "/") return "Painel Administrativo"
  const item = NAV.find((n) => pathname.startsWith(n.to) && n.to !== "/")
  return item?.label ?? "Painel Administrativo"
}

export function AdminShell({ children }: { children: React.ReactNode }) {
  const location = useLocation()
  const title = pageTitle(location.pathname)

  return (
    <SidebarProvider>
      <Sidebar>
        <SidebarHeader className="gap-3 p-4">
          <div className="flex items-center gap-3">
            <BrandMark className="size-9" />
            <div className="flex min-w-0 flex-col">
              <span className="truncate text-sm leading-tight font-semibold text-primary">
                Comunicação Proativa
              </span>
              <span className="flex items-center gap-2 text-xs text-muted-foreground">
                e Triagem Digital
                <Badge
                  variant="secondary"
                  className="bg-primary/10 px-1.5 py-0 text-[10px] font-semibold tracking-wide text-primary uppercase"
                >
                  MVP
                </Badge>
              </span>
            </div>
          </div>
        </SidebarHeader>

        <SidebarContent className="px-2">
          <SidebarGroup>
            <SidebarGroupContent>
              <SidebarMenu>
                {NAV.map((item) => {
                  const isActive = item.end
                    ? location.pathname === item.to
                    : location.pathname.startsWith(item.to)
                  return (
                    <SidebarMenuItem key={item.to}>
                      <SidebarMenuButton
                        asChild
                        isActive={isActive}
                        size="lg"
                        className="data-[active=true]:bg-primary/15 data-[active=true]:font-semibold data-[active=true]:text-primary data-[active=true]:hover:bg-primary/20 data-[active=true]:[&_svg]:text-primary"
                      >
                        <NavLink to={item.to} end={item.end}>
                          <item.icon />
                          <span>{item.label}</span>
                        </NavLink>
                      </SidebarMenuButton>
                    </SidebarMenuItem>
                  )
                })}
              </SidebarMenu>
            </SidebarGroupContent>
          </SidebarGroup>
        </SidebarContent>

        <SidebarFooter className="p-3">
          <div className="flex flex-col gap-2 rounded-xl bg-secondary/60 p-3 ring-1 ring-primary/10">
            <div className="flex items-center gap-2">
              <ShieldCheck className="size-4 text-primary" />
              <span className="text-sm font-semibold text-primary">LGPD</span>
            </div>
            <p className="text-xs leading-snug text-muted-foreground">
              Consentimento e finalidade registrados conforme LGPD.
            </p>
            <a
              href="#"
              className="text-xs font-medium text-primary underline underline-offset-2"
            >
              Saiba mais
            </a>
          </div>
        </SidebarFooter>
      </Sidebar>

      <SidebarInset>
        <header className="sticky top-0 z-10 flex h-14 items-center gap-3 border-b bg-background/90 px-6 backdrop-blur">
          <SidebarTrigger className="-ml-1" />
          <Separator orientation="vertical" className="h-5" />
          <h1 className="text-lg font-semibold tracking-tight">{title}</h1>
          <div className="ml-auto flex items-center gap-2">
            <div className="flex size-8 items-center justify-center rounded-full bg-muted text-muted-foreground">
              <UserGlyph />
            </div>
            <span className="text-sm font-medium">Admin</span>
          </div>
        </header>
        <div className="flex-1 px-6 py-6">{children}</div>
      </SidebarInset>
    </SidebarProvider>
  )
}

function UserGlyph() {
  return (
    <svg
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      className="size-4"
    >
      <circle cx="12" cy="8" r="4" />
      <path d="M4 21a8 8 0 0 1 16 0" />
    </svg>
  )
}
