import { useEffect, useState } from "react";
import {
  Alert,
  App as AntdApp,
  Button,
  Divider,
  Empty,
  Input,
  InputNumber,
  List,
  Space,
  Spin,
  Switch,
  Typography,
} from "antd";
import { api } from "../api/client";
import type { SkillInfo, SkillsResponse } from "../types";
import {
  DEFAULT_THRESHOLD,
  formatThreshold,
  parseEnabled,
  parseThreshold,
  ROUTER_ENABLED_LABEL,
  ROUTER_MODEL_LABEL,
  ROUTER_THRESHOLD_LABEL,
  skillThresholdKey,
} from "./skillRouter";

const { Text, Title } = Typography;

export interface RouterControlProps {
  /** Catálogo de skills (`GET /api/skills`); lo posee `SettingsDialog`. */
  skills: SkillInfo[];
  /** Herramientas núcleo, siempre expuestas, fuera del enrutado. */
  coreTools: string[];
  /** Estado de carga del catálogo. */
  loading: boolean;
  /** Error de la consulta del catálogo (si lo hubo). */
  error: string | null;
  /** Relee el catálogo; tras guardar refresca los umbrales efectivos. */
  refetch: () => Promise<SkillsResponse>;
}

/**
 * Control del enrutador de skills dentro de la pestaña «Herramientas»: sección
 * autocontenida (como `ToolsTab`). Recibe el catálogo de skills por props —lo
 * posee `SettingsDialog`, para no repetir `GET /api/skills` en cada apertura—
 * y, al montar, pide él mismo sus tres claves (`GET /settings`) y, al guardar,
 * envía únicamente `ROUTER_ENABLED`, `ROUTER_THRESHOLD` y `ROUTER_MODEL`
 * (`PUT /settings`), sin tocar el formulario compartido del diálogo: guardar el
 * enrutador no reescribe la identidad de `settings` ni revierte ediciones
 * pendientes de otras pestañas. El diálogo lo remonta en cada apertura (`key`),
 * de modo que siempre relee lo persistido y el borrador nace limpio. Los avisos
 * son informativos: nunca deshabilitan el guardado.
 */
export function RouterControl({
  skills,
  coreTools,
  loading,
  error,
  refetch,
}: RouterControlProps) {
  const { message: messageApi } = AntdApp.useApp();

  const [loadingSettings, setLoadingSettings] = useState(true);
  const [saving, setSaving] = useState(false);
  const [enabled, setEnabled] = useState(false);
  const [threshold, setThreshold] = useState<number | null>(DEFAULT_THRESHOLD);
  const [model, setModel] = useState("");
  // Overrides de umbral por skill en edición (id → valor). Solo guarda los
  // skills que el usuario ha tocado; el valor mostrado cae al efectivo del
  // catálogo si no hay entrada, de modo que no hace falta sincronizar estado
  // al cargar las skills (evita `setState` dentro de un efecto).
  const [skillThresholds, setSkillThresholds] = useState<Record<string, number>>(
    {},
  );

  // Lectura propia de las claves del enrutador. El `setState` ocurre en callbacks
  // asíncronos (nunca síncrono dentro del efecto) y el componente se remonta por
  // `key` en cada apertura del diálogo, así que no hace falta sincronizar props.
  useEffect(() => {
    let cancelled = false;
    api
      .getSettings()
      .then((data) => {
        if (cancelled) return;
        setEnabled(parseEnabled(data.ROUTER_ENABLED));
        setThreshold(parseThreshold(data.ROUTER_THRESHOLD));
        setModel(data.ROUTER_MODEL ?? "");
        setLoadingSettings(false);
      })
      .catch(() => {
        if (cancelled) return;
        messageApi.error("Error al cargar los ajustes del enrutador");
        setLoadingSettings(false);
      });
    return () => {
      cancelled = true;
    };
  }, [messageApi]);

  const extremeThreshold = threshold === 0 || threshold === 1;

  const handleSkillThresholdChange = (
    skillId: string,
    value: number | null,
  ) => {
    setSkillThresholds((prev) => {
      if (value === null) {
        if (!(skillId in prev)) return prev;
        const next = { ...prev };
        delete next[skillId];
        return next;
      }
      return { ...prev, [skillId]: value };
    });
  };

  const handleSave = async () => {
    setSaving(true);
    // Siempre las tres claves del enrutador, y además el umbral propio solo de
    // las skills cuyo valor editado difiere de su efectivo (sin sobrescrituras
    // redundantes: con el borrador limpio el payload vuelve a ser solo tres).
    const payload: Record<string, string> = {
      ROUTER_ENABLED: enabled ? "true" : "false",
      ROUTER_THRESHOLD: String(threshold ?? DEFAULT_THRESHOLD),
      ROUTER_MODEL: model,
    };
    for (const skill of skills) {
      const edited = skillThresholds[skill.id];
      if (edited !== undefined && edited !== skill.threshold) {
        payload[skillThresholdKey(skill.id)] = String(edited);
      }
    }
    try {
      await api.updateSettings(payload);
      // El estado local refleja lo realmente enviado.
      setEnabled(payload.ROUTER_ENABLED === "true");
      setThreshold(Number(payload.ROUTER_THRESHOLD));
      setModel(payload.ROUTER_MODEL);
      messageApi.success("Ajustes del enrutador guardados");
      // El catálogo lo posee el diálogo: se relee para que los umbrales
      // efectivos y las marcas de override no queden obsoletos tras persistir.
      // El fallo de la relectura no revierte el guardado ya confirmado.
      refetch().catch(() => undefined);
    } catch {
      messageApi.error("Error al guardar los ajustes del enrutador");
    } finally {
      setSaving(false);
    }
  };

  return (
    <section role="region" aria-label="Enrutador de skills">
      <Title level={5} style={{ marginTop: 0 }}>
        Enrutador de skills
      </Title>

      {loadingSettings && (
        <Spin style={{ display: "block", marginBottom: 12 }} />
      )}

      {!enabled && (
        <Alert
          type="info"
          showIcon
          style={{ marginBottom: 12 }}
          message="El enrutador está apagado: no se filtran las herramientas ni se inyectan los fragmentos de prompt. El guardado sigue disponible."
        />
      )}
      {extremeThreshold && (
        <Alert
          type="warning"
          showIcon
          style={{ marginBottom: 12 }}
          message={
            threshold === 0
              ? "Un umbral de 0 desactiva el filtrado: ninguna skill alcanza la selección. El guardado sigue disponible."
              : "Un umbral de 1 vuelve inalcanzable cualquier selección. El guardado sigue disponible."
          }
        />
      )}

      <Space direction="vertical" size="middle" style={{ width: "100%" }}>
        <div>
          <Switch
            id="router-enabled"
            checked={enabled}
            onChange={(checked) => setEnabled(checked)}
          />
          <label htmlFor="router-enabled" style={{ marginLeft: 8 }}>
            {ROUTER_ENABLED_LABEL}
          </label>
        </div>

        <div>
          <label
            htmlFor="router-threshold"
            style={{ display: "block", marginBottom: 4 }}
          >
            {ROUTER_THRESHOLD_LABEL}
          </label>
          <InputNumber
            id="router-threshold"
            min={0}
            max={1}
            step={0.05}
            value={threshold}
            onChange={(value) => setThreshold(value)}
            style={{ width: "100%" }}
          />
        </div>

        <div>
          <label
            htmlFor="router-model"
            style={{ display: "block", marginBottom: 4 }}
          >
            {ROUTER_MODEL_LABEL}
          </label>
          <Input
            id="router-model"
            value={model}
            onChange={(e) => setModel(e.target.value)}
          />
          <Text type="secondary" style={{ fontSize: 12 }}>
            Modelo de decisiones del enrutador (no un modelo generativo): solo
            necesita responder preguntas tipadas, no producir texto.
          </Text>
        </div>
      </Space>

      <Divider />

      <Title level={5}>Relación de skills</Title>
      {loading ? (
        <Spin />
      ) : skills.length === 0 ? (
        <Empty
          description={
            error
              ? "No se pudieron cargar las skills"
              : "No hay skills enrutables"
          }
        />
      ) : (
        <List
          size="small"
          dataSource={skills}
          rowKey="id"
          renderItem={(skill) => {
            // Solo las skills cuyo umbral efectivo difiere del global tienen
            // umbral propio: el resto hereda el global y no lleva campo.
            //
            // Deuda conocida (no resuelta a propósito): un
            // `ROUTER_THRESHOLD_<ID>` persistido con el mismo valor que el
            // global no difiere y, por tanto, no muestra campo ni forma de
            // limpiarlo desde aquí. Se deja así: no se añade acción de limpiar.
            const hasOwnThreshold =
              threshold !== null && skill.threshold !== threshold;
            const currentThreshold =
              skillThresholds[skill.id] ?? skill.threshold;
            return (
              <List.Item>
                <Space
                  direction="vertical"
                  size={4}
                  style={{ width: "100%" }}
                >
                  <Space wrap size="small">
                    <Text strong>{skill.id}</Text>
                    <Text>{skill.tools.join(", ")}</Text>
                    <Text type="secondary">
                      {`Umbral: ${formatThreshold(skill.threshold)}`}
                    </Text>
                  </Space>
                  {hasOwnThreshold && (
                    <InputNumber
                      aria-label={`Umbral de ${skill.id}`}
                      min={0}
                      max={1}
                      step={0.05}
                      value={currentThreshold}
                      onChange={(value) =>
                        handleSkillThresholdChange(skill.id, value)
                      }
                      style={{ width: "100%" }}
                    />
                  )}
                </Space>
              </List.Item>
            );
          }}
        />
      )}

      <Alert
        type="info"
        showIcon
        style={{ marginTop: 12, marginBottom: 12 }}
        message="Herramientas núcleo: siempre expuestas, fuera del enrutado"
        description={
          coreTools.length > 0
            ? `Nunca se filtran: ${coreTools.join(", ")}`
            : "Nunca se filtran."
        }
      />

      <Button type="primary" loading={saving} onClick={() => void handleSave()}>
        Guardar
      </Button>
    </section>
  );
}
