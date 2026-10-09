import { useEffect, useState } from "react";
import {
  Alert,
  App as AntdApp,
  Button,
  Input,
  Space,
  Spin,
  Switch,
  Typography,
} from "antd";
import { api } from "../api/client";
import {
  parseEnabled,
  ROUTER_ENABLED_LABEL,
  ROUTER_MODEL_LABEL,
} from "./skillRouter";

const { Text, Title } = Typography;

/**
 * Control global del enrutador de skills (pestaña «Enrutador de skills»):
 * interruptor para **activar el enrutado** y campo del **modelo de decisiones**.
 *
 * Sección autocontenida: al montar pide ella misma sus claves (`GET /settings`)
 * y, al guardar, envía únicamente `ROUTER_ENABLED` y `ROUTER_MODEL`
 * (`PUT /settings`), sin tocar el formulario compartido del diálogo. El diálogo
 * lo remonta en cada apertura (`key`), de modo que relee lo persistido y el
 * borrador nace limpio. El aviso de enrutador apagado es informativo: nunca
 * deshabilita el guardado.
 */
export function RouterControl() {
  const { message: messageApi } = AntdApp.useApp();

  const [loadingSettings, setLoadingSettings] = useState(true);
  const [saving, setSaving] = useState(false);
  const [enabled, setEnabled] = useState(false);
  const [model, setModel] = useState("");

  // Lectura propia de las claves del enrutador. El `setState` ocurre en
  // callbacks asíncronos (nunca síncrono dentro del efecto) y el componente se
  // remonta por `key` en cada apertura del diálogo, así que no hace falta
  // sincronizar props.
  useEffect(() => {
    let cancelled = false;
    api
      .getSettings()
      .then((data) => {
        if (cancelled) return;
        setEnabled(parseEnabled(data.ROUTER_ENABLED));
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

  const handleSave = async () => {
    setSaving(true);
    const payload: Record<string, string> = {
      ROUTER_ENABLED: enabled ? "true" : "false",
      ROUTER_MODEL: model,
    };
    try {
      await api.updateSettings(payload);
      // El estado local refleja lo realmente enviado.
      setEnabled(payload.ROUTER_ENABLED === "true");
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

      <Button
        type="primary"
        loading={saving}
        onClick={() => void handleSave()}
        style={{ marginTop: 16 }}
      >
        Guardar
      </Button>
    </section>
  );
}
