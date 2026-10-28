import { Tabs, Tag } from "antd";

import { useStrava } from "../hooks/useStrava";
import { useZennotes } from "../hooks/useZennotes";
import { useOpentodo } from "../hooks/useOpentodo";
import type { SettingsMap } from "../types";
import { KeyIntegration } from "./KeyIntegration";
import { StravaIntegration } from "./StravaIntegration";
import { ZennotesSection } from "./ZennotesSection";
import { OpentodoSection } from "./OpentodoSection";
import { isKeyConfigured, keyStatusLabel, UNAVAILABLE_LABEL } from "./integrationStatus";
import { KEY_INTEGRATIONS } from "./keyIntegrations";

export interface IntegrationsTabProps {
  /** Ajustes vigentes; de aquí se deriva el estado de las claves. */
  settings: SettingsMap;
  /** Persiste cambios (hook `useSettings`); lo usan Strava y las claves. */
  updateSettings: (data: Record<string, string>) => Promise<void>;
  /** `true` mientras se persisten los ajustes; para los botones. */
  saving?: boolean;
}

/** Texto del indicador mientras el estado remoto aún no ha llegado. */
const PENDING_LABEL = "Consultando…";

interface StatusTagProps {
  label: string;
  /** `true` si la integración está operativa (verde) o pendiente (neutro). */
  ok: boolean;
}

function StatusTag({ label, ok }: StatusTagProps) {
  // El texto viaja SIEMPRE junto al color: el estado nunca se transmite solo
  // por el color del indicador.
  return (
    <Tag color={ok ? "green" : "default"} style={{ marginInlineEnd: 0 }}>
      {label}
    </Tag>
  );
}

/**
 * Etiqueta de una sub-pestaña: su nombre seguido del indicador de estado. El
 * texto del indicador forma parte del nombre accesible de la pestaña, así que
 * un lector de pantalla anuncia «OpenWeatherMap Configurada» sin depender del
 * color.
 */
function tabLabel(name: string, label: string, ok: boolean) {
  return (
    <span style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>
      {name}
      <StatusTag label={label} ok={ok} />
    </span>
  );
}

/** Estado del indicador de una integración cuyo estado se lee de un endpoint. */
interface RemoteState {
  /** `true`/`false` cuando el estado ya se conoce; `null` si no. */
  ok: boolean | null;
  /** Error de la última lectura del endpoint, si lo hubo. */
  error: string | null;
}

/**
 * Texto del indicador de una integración remota, con los tres estados
 * posibles: positivo, negativo y —cuando la lectura falló y no hay dato—
 * «Sin datos».
 */
function remoteLabel(state: RemoteState, yes: string, no: string): string {
  if (state.ok !== null) return state.ok ? yes : no;
  return state.error ? UNAVAILABLE_LABEL : PENDING_LABEL;
}

/**
 * Pestaña «Integraciones» del diálogo de ajustes: un `Tabs` **anidado
 * horizontal** con una sub-pestaña por servicio, en el orden cerrado Strava,
 * OpenWeatherMap, Google Places, Brave Search, apimail, Miniflux, ZenNotes y
 * OpenTodo.
 *
 * Los `Form.Item` de las cinco integraciones de clave cuelgan del formulario de
 * ajustes del padre (`SettingsForm`), que envuelve a este componente; por eso
 * cada sub-pestaña tiene su propio botón «Guardar» con `htmlType="submit"`.
 * Las cinco se pintan con el mismo `KeyIntegration`, guiado por la tabla
 * `KEY_INTEGRATIONS`. La URL base de OpenTodo cuelga igualmente del formulario
 * compartido; su token, en cambio, se guarda por su endpoint dedicado.
 *
 * **Estado de los indicadores.** Los de Strava, ZenNotes y OpenTodo salen de sus
 * endpoints dedicados: este contenedor llama a `useStrava()`, `useZennotes()` y
 * `useOpentodo()` una sola vez y pasa el resultado a los componentes, que en ese
 * caso no vuelven a consultar (patrón envoltorio `*State`). Así se evita
 * duplicar `GET /api/strava/status`, `GET /api/zennotes/status` y
 * `GET /api/opentodo/status`. El de las cinco integraciones de clave se deriva
 * de `settings` en cada render, de modo que el indicador cambia en cuanto el
 * padre refresca `settings` tras guardar.
 *
 * Los hooks se leen una vez al montar, y el montaje ocurre por apertura: el
 * `Modal` del diálogo usa `destroyOnHidden`, así que cerrar desmonta este panel
 * y abrir lo vuelve a montar. No hacen falta señales de visibilidad ni
 * contadores de aperturas.
 *
 * Todas las sub-pestañas se montan con `forceRender` para que el indicador de
 * la etiqueta sea correcto **desde el primer render** del panel, sin depender
 * de que el usuario abra cada una (el montaje perezoso de antd dejaría las no
 * visitadas sin calcular).
 */
export function IntegrationsTab({
  settings,
  updateSettings,
  saving = false,
}: IntegrationsTabProps) {
  // Una única lectura por endpoint: los componentes reciben el estado ya
  // cargado y no vuelven a llamar al hook.
  const stravaState = useStrava();
  const zennotesState = useZennotes();
  const opentodoState = useOpentodo();

  const strava: RemoteState = {
    ok: stravaState.status ? stravaState.status.connected : null,
    error: stravaState.error,
  };
  const zennotes: RemoteState = {
    ok: zennotesState.status ? zennotesState.status.configured : null,
    error: zennotesState.error,
  };
  const opentodo: RemoteState = {
    ok: opentodoState.status ? opentodoState.status.configured : null,
    error: opentodoState.error,
  };

  const stravaLabel = remoteLabel(strava, "Conectada", "No conectada");
  const zennotesLabel = remoteLabel(zennotes, "Configurado", "Sin configurar");
  const opentodoLabel = remoteLabel(opentodo, "Configurado", "Sin configurar");

  return (
    <Tabs
      tabPosition="left"
      aria-label="Integraciones"
      role="group"
      items={[
        {
          key: "strava",
          label: tabLabel("Strava", stravaLabel, strava.ok === true),
          forceRender: true,
          children: (
            <StravaIntegration
              settings={settings}
              updateSettings={updateSettings}
              stravaState={stravaState}
            />
          ),
        },
        ...KEY_INTEGRATIONS.map((definition) => ({
          key: definition.statusKey,
          label: tabLabel(
            definition.name,
            keyStatusLabel(settings, definition.statusKey),
            isKeyConfigured(settings, definition.statusKey),
          ),
          forceRender: true,
          children: (
            <KeyIntegration
              definition={definition}
              settings={settings}
              saving={saving}
            />
          ),
        })),
        {
          key: "zennotes",
          label: tabLabel("ZenNotes", zennotesLabel, zennotes.ok === true),
          forceRender: true,
          children: <ZennotesSection zennotesState={zennotesState} />,
        },
        {
          key: "opentodo",
          label: tabLabel("OpenTodo", opentodoLabel, opentodo.ok === true),
          forceRender: true,
          children: <OpentodoSection opentodoState={opentodoState} saving={saving} />,
        },
      ]}
    />
  );
}
