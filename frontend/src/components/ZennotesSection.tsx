import { useEffect, useRef, useState } from "react";
import { App as AntdApp, Button, Input, Space, Spin, Typography } from "antd";
import { useZennotes } from "../hooks/useZennotes";
import type { UseZennotesReturn } from "../hooks/useZennotes";

const { Text, Title } = Typography;

export interface ZennotesSectionProps {
  /**
   * Estado ya cargado por el contenedor («Integraciones»). Si llega, la sección
   * **no** llama a `useZennotes()`: el indicador de la sub-pestaña y el panel
   * comparten una única lectura de `GET /api/zennotes/status`. Si falta, el
   * envoltorio la hace por su cuenta (compatibilidad con quien monte la
   * sección suelta).
   */
  zennotesState?: UseZennotesReturn;
}

/**
 * Sección «ZenNotes» de la sub-pestaña «ZenNotes» de **Integraciones** del
 * diálogo de ajustes.
 *
 * Es independiente del formulario de ajustes (no usa `updateSettings`): el
 * token es una clave sensible, se excluye de `GET /api/settings` y
 * `PUT /api/settings` lo ignora, así que se guarda por su endpoint dedicado
 * (`PUT /api/zennotes/token`). El campo **nunca** muestra el valor guardado: su
 * estado arranca vacío y solo refleja lo que teclea el usuario. El estado de
 * configuración sale de `GET /api/zennotes/status` (que jamás devuelve el token).
 *
 * Envoltorio: con `zennotesState` renderiza la vista sin consultar el endpoint;
 * sin él, delega en `ZennotesSectionConnected`, que llama al hook.
 */
export function ZennotesSection(props: ZennotesSectionProps) {
  if (props.zennotesState) {
    return <ZennotesSectionView state={props.zennotesState} />;
  }
  return <ZennotesSectionConnected />;
}

function ZennotesSectionConnected() {
  const state = useZennotes();
  return <ZennotesSectionView state={state} />;
}

function ZennotesSectionView({ state }: { state: UseZennotesReturn }) {
  const { message } = AntdApp.useApp();
  const { status, loading, error, saving, saveToken } = state;
  const [token, setToken] = useState("");

  // Los manejadores resuelven de forma asíncrona: solo aplican su `setState`
  // si el componente sigue montado. Mismo patrón que `mountedRef` en el hook.
  const mountedRef = useRef(true);
  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const handleSave = async () => {
    const trimmed = token.trim();
    if (trimmed === "") {
      message.warning("Escribe un token de ZenNotes antes de guardar");
      return;
    }
    try {
      await saveToken(trimmed);
      // El token guardado no se relee: el campo se limpia siempre tras guardar.
      if (mountedRef.current) setToken("");
      message.success("Token de ZenNotes guardado");
    } catch {
      message.error("Error al guardar el token de ZenNotes");
    }
  };

  const renderStatus = () => {
    if (loading && !status) {
      return (
        <div role="status" aria-label="Cargando estado de ZenNotes">
          <Spin />
        </div>
      );
    }
    if (error) {
      return (
        <Text type="warning">{`No se pudo consultar el estado: ${error}`}</Text>
      );
    }
    return (
      <Text type="secondary">
        {status?.configured ? "Configurado" : "Sin configurar"}
      </Text>
    );
  };

  return (
    <section aria-label="ZenNotes">
      <Title level={5} style={{ marginTop: 0 }}>
        ZenNotes
      </Title>
      <Text type="secondary">
        Token de API para el vault de ZenNotes. Por seguridad, el valor guardado
        nunca se muestra: para cambiarlo, escribe uno nuevo y guarda.
      </Text>

      <Space
        direction="vertical"
        size="middle"
        style={{ width: "100%", marginTop: 16 }}
      >
        <Space direction="vertical" size={4} style={{ width: "100%" }}>
          <Text>Estado</Text>
          <div aria-live="polite">{renderStatus()}</div>
        </Space>

        <Space.Compact style={{ width: "100%" }}>
          <Input.Password
            aria-label="ZenNotes · token de API"
            placeholder="Escribe un token para configurar ZenNotes"
            autoComplete="off"
            value={token}
            onChange={(event) => setToken(event.target.value)}
            onPressEnter={(event) => {
              // El input cuelga del formulario de ajustes del padre: sin esto,
              // Intro dispara además el submit implícito de ese form
              // (`PUT /api/settings` + cierre del diálogo) y el token se
              // guardaría por dos vías a la vez.
              event.preventDefault();
              void handleSave();
            }}
          />
          <Button type="primary" loading={saving} onClick={() => void handleSave()}>
            Guardar token
          </Button>
        </Space.Compact>
      </Space>
    </section>
  );
}
