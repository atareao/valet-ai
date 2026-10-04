import type { FC } from "react";
import { Alert } from "antd";
import { WIDGET_REGISTRY } from "./registry";
import { WidgetErrorBoundary } from "./WidgetErrorBoundary";

export interface WidgetRendererProps {
  id: string;
  name: string;
  data: unknown;
  onAction: (action: string, payload?: unknown) => void;
  /** Reenviado a los widgets para bloquear sus acciones. */
  disabled?: boolean;
}

/**
 * Renderiza un `WidgetInstance` a partir del `WIDGET_REGISTRY`. Si el nombre no
 * existe en el registry, muestra un aviso visible y no lanza. Cualquier fallo de
 * render del widget queda contenido por `WidgetErrorBoundary`.
 */
export const WidgetRenderer: FC<WidgetRendererProps> = ({
  name,
  data,
  onAction,
  disabled,
}) => {
  const Component = WIDGET_REGISTRY[name];

  if (!Component) {
    return (
      <Alert
        type="warning"
        showIcon
        message={`Widget desconocido: ${name}`}
        style={{ marginTop: 8 }}
      />
    );
  }

  return (
    <WidgetErrorBoundary name={name}>
      <div style={{ marginTop: 8 }}>
        <Component data={data} onAction={onAction} disabled={disabled} />
      </div>
    </WidgetErrorBoundary>
  );
};