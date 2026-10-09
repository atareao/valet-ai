import { lazy } from "react";
import type { WidgetComponent } from "./types";
import { QuickFormWidget } from "./QuickFormWidget";
import { ChecklistWidget } from "./ChecklistWidget";

const LocationWidget = lazy(() =>
  import("./LocationWidget").then((m) => ({ default: m.LocationWidget })),
);

/**
 * Mapea el nombre del widget a su componente React. Un nombre que no figure
 * aquí es degradado a un aviso visible por `WidgetRenderer`.
 */
export const WIDGET_REGISTRY: Record<string, WidgetComponent> = {
  QuickForm: QuickFormWidget,
  Checklist: ChecklistWidget,
  LocationWidget,
};