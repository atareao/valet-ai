import { useEffect, useRef, useState } from "react";
import type { KeyboardEvent } from "react";
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
import type { UseStravaReturn } from "../hooks/useStrava";
import type { SettingsMap, StravaCheckResult } from "../types";

const { Text, Title } = Typography;

export interface StravaIntegrationProps {
  /** Settings vigentes; de aquí salen `strava_client_id`/`strava_client_secret`. */
  settings: SettingsMap;
  /** Persiste cambios (hook `useSettings`). */
  updateSettings: (data: Record<string, string>) => Promise<void>;
  /**
   * Estado de la conexión ya cargado por el contenedor («Integraciones»). Si
   * llega, la sección **no** llama a `useStrava()`: así el indicador de la
   * sub-pestaña y el panel comparten una única lectura de
   * `GET /api/strava/status`. Si falta, el envoltorio la hace por su cuenta
   * (compatibilidad con quien monte la sección suelta).
   */
  stravaState?: UseStravaReturn;
}

interface StravaCredentialsForm {
  strava_client_id?: string;
  strava_client_secret?: string;
}

/**
 * Sección de Strava dentro de la sub-pestaña «Strava» de «Integraciones»:
 * credenciales OAuth, estado de la conexión, inicio del flujo OAuth y
 * desconexión.
 *
 * Los **tokens nunca se muestran**: la interfaz solo lee `GET /api/strava/status`
 * (que devuelve el atleta y el scope, sin tokens) y las credenciales que el
 * usuario teclea. «Connect with Strava» es un enlace de navegación al endpoint
 * `authorize` (el backend responde 302 hacia Strava), no una llamada `fetch`.
 *
 * Envoltorio: con `stravaState` renderiza la vista sin consultar el endpoint;
 * sin él, delega en `StravaIntegrationConnected`, que llama al hook.
 */
export function StravaIntegration(props: StravaIntegrationProps) {
  if (props.stravaState) {
    return <StravaIntegrationView {...props} state={props.stravaState} />;
  }
  return <StravaIntegrationConnected {...props} />;
}

function StravaIntegrationConnected(props: StravaIntegrationProps) {
  const state = useStrava();
  return <StravaIntegrationView {...props} state={state} />;
}

interface StravaIntegrationViewProps extends StravaIntegrationProps {
  /** Estado ya resuelto (propio o heredado del contenedor). */
  state: UseStravaReturn;
}

function StravaIntegrationView({
  settings,
  updateSettings,
  state,
}: StravaIntegrationViewProps) {
  const { message } = AntdApp.useApp();
  const { status, loading, checking, error, check, disconnect } = state;
  // Instancia propia para el formulario de credenciales: se envía con
  // `submit()` explícito porque el `Form` se renderiza con `component={false}`.
  const [credentialsForm] = Form.useForm<StravaCredentialsForm>();
  const [saving, setSaving] = useState(false);
  const [disconnecting, setDisconnecting] = useState(false);
  const [checkResult, setCheckResult] = useState<StravaCheckResult | null>(null);
  const [disconnectWarning, setDisconnectWarning] = useState<string | null>(null);

  // Los manejadores resuelven de forma asíncrona: solo aplican su `setState`
  // si el componente sigue montado. Mismo patrón que `mountedRef` en el hook.
  const mountedRef = useRef(true);
  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

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
      if (mountedRef.current) setSaving(false);
    }
  };

  const handleDisconnect = async () => {
    setDisconnecting(true);
    setDisconnectWarning(null);
    try {
      // `disconnect` solo rechaza si falla la propia desconexión; un fallo del
      // refresco del estado se muestra por separado (aviso de estado).
      const { warning } = await disconnect();
      message.success("Cuenta de Strava desconectada");
      if (mountedRef.current) setDisconnectWarning(warning);
    } catch {
      message.error("Error al desconectar la cuenta de Strava");
    } finally {
      if (mountedRef.current) setDisconnecting(false);
    }
  };

  /**
   * Intro en cualquiera de los dos campos de credenciales.
   *
   * Como el `Form` va con `component={false}`, sus inputs cuelgan del `<form>`
   * real del formulario de ajustes del padre: sin interceptar la tecla, el
   * navegador dispara el **submit implícito** de ese form
   * (`handleSettingsSubmit` → `PUT /api/settings` **sin** las credenciales) y
   * el diálogo se cierra sin guardar nada. `preventDefault()` corta ese camino
   * y `submit()` envía el formulario correcto.
   */
  const handleCredentialsEnter = (event: KeyboardEvent<HTMLInputElement>) => {
    event.preventDefault();
    credentialsForm.submit();
  };

  const handleCheck = async () => {
    setCheckResult(null);
    try {
      const result = await check();
      if (mountedRef.current) setCheckResult(result);
    } catch {
      // Solo se llega aquí si la propia petición HTTP falla (`check` siempre
      // responde 200 cuando Strava contesta). Se muestra como error de sondeo.
      if (mountedRef.current) {
        setCheckResult({
          ok: false,
          athlete_id: null,
          athlete_name: null,
          error: "No se pudo comprobar la conexión con Strava",
        });
      }
    }
  };

  const connected = status?.connected === true;
  const athleteLabel = status?.athlete_name ?? status?.athlete_id ?? "atleta";
  const scopeHasReadAll = status?.scope?.includes("activity:read_all") === true;

  const renderCheckResult = () => {
    if (!checkResult) return null;
    if (checkResult.ok) {
      const athlete =
        checkResult.athlete_name ?? checkResult.athlete_id ?? "atleta";
      return (
        <Alert
          type="success"
          showIcon
          message="Conexión con Strava correcta"
          description={`Atleta: ${athlete}`}
        />
      );
    }
    return (
      <Alert
        type="error"
        showIcon
        message="La comprobación con Strava ha fallado"
        description={checkResult.error ?? "Error desconocido"}
      />
    );
  };

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
    if (!connected) {
      return <Text>No conectada</Text>;
    }
    return (
      <Space direction="vertical" size={4} style={{ width: "100%" }}>
        <Text>{`Conectada como ${athleteLabel}`}</Text>
        <Text type="secondary">
          {`Permisos concedidos: ${status?.scope ?? "desconocidos"}`}
        </Text>
        {!scopeHasReadAll && (
          <Alert
            type="warning"
            showIcon
            message="Permisos insuficientes"
            description="La cuenta no concedió activity:read_all; vuelve a conectar para autorizar la lectura de actividades."
          />
        )}
      </Space>
    );
  };

  return (
    <section role="region" aria-label="Strava">
      <Title level={5} style={{ marginTop: 0 }}>
        Strava
      </Title>
      <Text type="secondary">Cuenta de Strava (solo lectura)</Text>

      {/* `component={false}`: la sección vive dentro del `Form` de ajustes del
          padre («Integraciones»), y un `<form>` anidado sería HTML inválido. El
          envío se dispara con `submit()` sobre la instancia propia —también al
          pulsar Intro en los campos, que si no dispararía el submit implícito
          del formulario del padre—. */}
      <Form<StravaCredentialsForm>
        form={credentialsForm}
        component={false}
        layout="vertical"
        onFinish={handleSave}
        initialValues={{
          strava_client_id: settings?.strava_client_id ?? "",
          strava_client_secret: settings?.strava_client_secret ?? "",
        }}
      >
        <Form.Item label="Strava Client ID" name="strava_client_id">
          <Input autoComplete="off" onPressEnter={handleCredentialsEnter} />
        </Form.Item>
        <Form.Item label="Strava Client Secret" name="strava_client_secret">
          <Input.Password
            autoComplete="off"
            placeholder="Dejar vacío para usar variable de entorno"
            onPressEnter={handleCredentialsEnter}
          />
        </Form.Item>
        <Button
          type="primary"
          loading={saving}
          onClick={() => credentialsForm.submit()}
        >
          Guardar credenciales
        </Button>
      </Form>

      <Space direction="vertical" size="middle" style={{ width: "100%", marginTop: 16 }}>
        <div aria-live="polite">{renderStatus()}</div>

        {(checkResult || disconnectWarning) && (
          // El `Alert` de antd ya es su propia región viva (`role="alert"`):
          // envolverlo en un contenedor anunciado provocaría doble anuncio y
          // politeness en conflicto. Se deja que el propio `Alert` anuncie.
          <div>
            {renderCheckResult()}
            {disconnectWarning && (
              <Alert
                type="warning"
                showIcon
                message="Revisa el acceso en Strava"
                description={disconnectWarning}
              />
            )}
          </div>
        )}

        <Space>
          {/* `Button href` renderiza un `<a>` con el estilo del botón: navegación
              completa al endpoint `authorize` (el backend responde 302 hacia
              Strava) y activable con teclado. */}
          <Button href={`${BASE_URL}/strava/authorize`}>Connect with Strava</Button>
          {connected && (
            <>
              <Button loading={checking} onClick={() => void handleCheck()}>
                Probar conexión
              </Button>
              <Button danger loading={disconnecting} onClick={() => void handleDisconnect()}>
                Desconectar
              </Button>
            </>
          )}
        </Space>
      </Space>
    </section>
  );
}
