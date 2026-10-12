import { useCallback, useEffect, useRef, useState } from "react";
import { App as AntdApp, Space, Spin, Switch, Typography } from "antd";
import {
  disablePushNotifications,
  enablePushNotifications,
  getPushStatus,
  isPushSupported,
  type PushStatus,
} from "../push/pushManager";

const { Text, Title } = Typography;

/**
 * Pestaña «Notificaciones» del diálogo de ajustes. Se extrae a un componente
 * propio —en vez de vivir dentro de `SettingsDialog`— para poder probarla sin
 * montar el diálogo entero y su `Modal`.
 *
 * Al montar refleja el estado real del navegador (`getPushStatus`) y ofrece un
 * único `Switch`. El permiso se pide **aquí y solo aquí**, al pulsar el
 * interruptor: no hay diálogos previos ni avisos intermedios. Tras cada acción
 * se relee el estado. Sin soporte del navegador, el interruptor queda
 * deshabilitado y se explica el motivo.
 */
export function NotificationsTab() {
  const { message } = AntdApp.useApp();
  const supported = isPushSupported();
  // Estado inicial: «sin soporte» se conoce de forma síncrona, así que se
  // deriva en la inicialización y el efecto solo relee el estado del navegador
  // cuando este sí soporta push.
  const [status, setStatus] = useState<PushStatus | null>(() =>
    supported ? null : "unsupported",
  );
  const [busy, setBusy] = useState(false);

  // Los manejadores resuelven de forma asíncrona: solo aplican su `setState`
  // si el componente sigue montado. Mismo patrón que `StravaIntegration`.
  const mountedRef = useRef(true);
  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const refresh = useCallback((): Promise<void> => {
    return getPushStatus()
      .then((next) => {
        if (mountedRef.current) setStatus(next);
      })
      .catch(() => {
        if (mountedRef.current) {
          message.error("No se pudo consultar el estado de las notificaciones");
        }
      });
  }, [message]);

  useEffect(() => {
    if (!supported) return;
    void refresh();
  }, [supported, refresh]);

  const handleToggle = async (checked: boolean) => {
    setBusy(true);
    try {
      if (checked) {
        await enablePushNotifications();
        message.success("Notificaciones activadas");
      } else {
        await disablePushNotifications();
        message.success("Notificaciones desactivadas");
      }
    } catch (error) {
      message.error(
        error instanceof Error
          ? error.message
          : "No se pudo cambiar el estado de las notificaciones",
      );
    } finally {
      if (mountedRef.current) setBusy(false);
      // Relectura del estado real tras la acción (acierte o falle).
      await refresh();
    }
  };

  const checked = status === "subscribed";
  const disabled = !supported || status === null;

  const renderStatus = () => {
    if (status === null) {
      return (
        <div role="status" aria-label="Cargando estado de las notificaciones">
          <Spin />
        </div>
      );
    }
    switch (status) {
      case "unsupported":
        return (
          <Text type="secondary">
            Este navegador no soporta notificaciones push.
          </Text>
        );
      case "denied":
        return (
          <Text type="secondary">
            El permiso de notificaciones está denegado; actívalo en los ajustes
            del navegador.
          </Text>
        );
      case "granted-unsubscribed":
        return (
          <Text type="secondary">
            Permiso concedido; activa el interruptor para suscribir este
            navegador.
          </Text>
        );
      case "subscribed":
        return <Text type="secondary">Notificaciones activadas en este navegador.</Text>;
    }
  };

  return (
    <section aria-label="Notificaciones">
      <Title level={5} style={{ marginTop: 0 }}>
        Notificaciones
      </Title>
      <Space direction="vertical" size="middle" style={{ width: "100%" }}>
        <Space align="center">
          <Switch
            aria-label="Notificaciones"
            checked={checked}
            disabled={disabled}
            loading={busy}
            onChange={(value) => void handleToggle(value)}
          />
          {renderStatus()}
        </Space>
      </Space>
    </section>
  );
}
