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
import { useSkills } from "../hooks/useSkills";
import {
  DEFAULT_THRESHOLD,
  parseEnabled,
  parseThreshold,
  ROUTER_ENABLED_LABEL,
  ROUTER_MODEL_LABEL,
  ROUTER_THRESHOLD_LABEL,
} from "./skillRouter";

const { Text, Title } = Typography;

/**
 * Control del enrutador de skills dentro de la pestaña «Herramientas»: sección
 * autocontenida (como `ToolsTab`). Al montar pide él mismo sus tres claves
 * (`GET /settings`) y el catálogo de skills (`GET /api/skills`) y, al guardar,
 * envía únicamente `ROUTER_ENABLED`, `ROUTER_THRESHOLD` y `ROUTER_MODEL`
 * (`PUT /settings`), sin tocar el formulario compartido del diálogo: guardar el
 * enrutador no reescribe la identidad de `settings` ni revierte ediciones
 * pendientes de otras pestañas. El diálogo lo remonta en cada apertura (`key`),
 * de modo que siempre relee lo persistido y el borrador nace limpio. Los avisos
 * son informativos: nunca deshabilitan el guardado.
 */
export function RouterControl() {
  const { message: messageApi } = AntdApp.useApp();
  const { skills, coreTools, loading, error } = useSkills();

  const [loadingSettings, setLoadingSettings] = useState(true);
  const [saving, setSaving] = useState(false);
  const [enabled, setEnabled] = useState(false);
  const [threshold, setThreshold] = useState<number | null>(DEFAULT_THRESHOLD);
  const [model, setModel] = useState("");

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

  const handleSave = async () => {
    setSaving(true);
    // Solo las tres claves del enrutador, siempre como cadenas.
    const payload: Record<string, string> = {
      ROUTER_ENABLED: enabled ? "true" : "false",
      ROUTER_THRESHOLD: String(threshold ?? DEFAULT_THRESHOLD),
      ROUTER_MODEL: model,
    };
    try {
      await api.updateSettings(payload);
      // El estado local refleja lo realmente enviado.
      setEnabled(payload.ROUTER_ENABLED === "true");
      setThreshold(Number(payload.ROUTER_THRESHOLD));
      setModel(payload.ROUTER_MODEL);
      messageApi.success("Ajustes del enrutador guardados");
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
          renderItem={(skill) => (
            <List.Item>
              <Text strong style={{ marginRight: 8 }}>
                {skill.id}
              </Text>
              <Text>{skill.tools.join(", ")}</Text>
            </List.Item>
          )}
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
