import type { ReactNode } from "react";

export interface WidgetProps<T = unknown> {
  data: T;
  onAction: (action: string, payload?: unknown) => void;
  /** Bloquea las acciones del widget (p. ej. mientras el asistente streamea). */
  disabled?: boolean;
}

/**
 * Tipo de un componente de widget registrado. Usa el "bivariance hack" para que
 * componentes tipados con `WidgetProps<QuickFormData>` (datos concretos) sean
 * asignables a este tipo sin cast, pese a que el registry entrega `data` como
 * `unknown`. Evita los `as WidgetComponent` que exigiría la contravarianza
 * estricta de `React.ComponentType`.
 */
export type WidgetComponent = {
  bivarianceHack(props: WidgetProps): ReactNode | Promise<ReactNode>;
}["bivarianceHack"];

export interface WidgetInstance {
  id: string;
  name: string;
  data: unknown;
  /**
   * Tramo del mensaje del asistente que este widget **ocupa**, expresado como
   * rango **semiabierto** `[start, end)` en **bytes UTF-8** sobre el `content`
   * persistido (change `widget-projection`, D2 opción (c)).
   *
   * Es **opcional**: sin él el widget se pinta **debajo** del texto, como hoy.
   * Es el caso de los widgets que renderiza el modelo (`render_widget`) y de
   * todas las filas anteriores a este change (sin backfill).
   */
  span?: [number, number];
}

export interface QuickFormField {
  name: string;
  label: string;
  type: "text" | "textarea" | "number" | "select" | "checkbox" | "slider" | "radio";
  options?: string[];
  min?: number;
  max?: number;
}

export interface QuickFormData {
  title?: string;
  description?: string;
  fields: QuickFormField[];
  submit_label?: string;
}

export interface LocationData {
  title?: string;
  description?: string;
  latitude?: number;
  longitude?: number;
  address?: string;
}

export interface ChecklistItem {
  id?: string;
  label?: string;
  text?: string;
}

export interface ChecklistData {
  title?: string;
  items: ChecklistItem[];
}