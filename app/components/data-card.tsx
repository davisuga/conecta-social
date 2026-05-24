import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "~/components/ui/card"

export function DataCard({
  title,
  action,
  toolbar,
  children,
  footer,
}: {
  title: string
  action?: React.ReactNode
  toolbar?: React.ReactNode
  children: React.ReactNode
  footer?: React.ReactNode
}) {
  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between gap-4">
        <CardTitle>{title}</CardTitle>
        {action}
      </CardHeader>
      {toolbar && <div className="px-4 pb-2">{toolbar}</div>}
      <CardContent className="p-0">{children}</CardContent>
      {footer && (
        <div className="flex items-center justify-between gap-4 border-t px-4 py-2 text-sm text-muted-foreground">
          {footer}
        </div>
      )}
    </Card>
  )
}

export function EmptyRow({ colSpan, children }: { colSpan: number; children: React.ReactNode }) {
  return (
    <tr>
      <td colSpan={colSpan} className="px-4 py-10 text-center text-muted-foreground">
        {children}
      </td>
    </tr>
  )
}
