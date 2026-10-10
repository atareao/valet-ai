import { useState } from "react";
import {
  Alert,
  App as AntdApp,
  Button,
  Form,
  Input,
  Space,
  Spin,
  Typography,
} from "antd";
import { BASE_URL } from "../api/client";
import { useStrava } from "../hooks/useStrava";

const { Text, Title } = Typography;

export interface StravaIntegrationProps {
  /** Settings vigentes; de aquí salen `strava_client_id`/`strava_client_secret`. */
  settings: Record<string, string> | null;
  /** Persiste cambios (hook `useSettings`). */
  updateSettings: (data: Record<string, string>) => Promise<void>;
}

interface StravaCredentialsForm {
  strava_client_id?: string;
  strava_client_secret?: string;
}

/**
 * Sección «Integraciones» del diálogo de ajustes: credenciales OAuth de Strava,
 * estado de la conexión, inicio del flujo OAuth y desconexión.
 *
 * Los **tokens nunca se muestran**: la interfaz solo lee `GET /api/strava/status`
 * (que devuelve el atleta y el scope, sin tokens) y las credenciales que el
 * usuario teclea. «Connect with Strava» es un enlace de navegación al endpoint
 * `authorize` (el backend responde 302 hacia Strava), no una llamada `fetch`.
 */
export function StravaIntegration({
  settings,
  updateSettings,
}: StravaIntegrationProps) {
  const { message } = AntdApp.useApp();
  const { status, loading, error, disconnect } = useStrava();
  const [saving, setSaving] = useState(false);
  const [disconnecting, setDisconnecting] = useState(false);

  const handleSave = async (values: StravaCredentialsForm) => {
    setSaving(true);
    try {
      await updateSettings({
        strava_client_id: values.strava_client_id ?? "",
        strava_client_secret: values.strava_client_secret ?? "",
      });
      message.success("Credenciales de Strava guardadas");
    } catch {
      message.error("Error al guardar las credenciales de Strava");
    } finally {
      setSaving(false);
    }
  };

  const handleDisconnect = async () => {
    setDisconnecting(true);
    try {
      // `disconnect` solo rechaza si falla la propia desconexión; un fallo del
      // refresco del estado se muestra por separado (aviso de estado).
      await disconnect();
      message.success("Cuenta de Strava desconectada");
    } catch {
      message.error("Error al desconectar la cuenta de Strava");
    } finally {
      setDisconnecting(false);
    }
  };

  const connected = status?.connected === true;
  const athleteLabel = status?.athlete_name ?? status?.athlete_id ?? "atleta";

  const renderStatus = () => {
    if (loading && !status) {
      return (
        <div role="status" aria-label="Cargando estado de Strava">
          <Spin />
        </div>
      );
    }
    if (error) {
      return (
        <Alert
          type="warning"
          showIcon
          message="No se pudo consultar el estado de Strava"
          description={error}
        />
      );
    }
    return (
      <Text>
        {connected ? `Conectada como ${athleteLabel}` : "No conectada"}
      </Text>
    );
  };

  return (
    <section role="region" aria-label="Integraciones">
      <Title level={5} style={{ marginTop: 0 }}>
        Integraciones
      </Title>
      <Text type="secondary">Cuenta de Strava (solo lectura)</Text>

      <Form<StravaCredentialsForm>
        layout="vertical"
        onFinish={handleSave}
        initialValues={{
          strava_client_id: settings?.strava_client_id ?? "",
          strava_client_secret: settings?.strava_client_secret ?? "",
        }}
      >
        <Form.Item label="Strava Client ID" name="strava_client_id">
          <Input autoComplete="off" />
        </Form.Item>
        <Form.Item label="Strava Client Secret" name="strava_client_secret">
          <Input.Password
            autoComplete="off"
            placeholder="Dejar vacío para usar variable de entorno"
          />
        </Form.Item>
        <Button type="primary" htmlType="submit" loading={saving}>
          Guardar credenciales
        </Button>
      </Form>

      <Space direction="vertical" size="middle" style={{ width: "100%", marginTop: 16 }}>
        <div aria-live="polite">{renderStatus()}</div>

        <Space>
          {/* `Button href` renderiza un `<a>` con el estilo del botón: navegación
              completa al endpoint `authorize` (el backend responde 302 hacia
              Strava) y activable con teclado. */}
          <Button href={`${BASE_URL}/strava/authorize`}>Connect with Strava</Button>
          {connected && (
            <Button danger loading={disconnecting} onClick={() => void handleDisconnect()}>
              Desconectar
            </Button>
          )}
        </Space>
      </Space>
    </section>
  );
}
