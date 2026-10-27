import { useEffect, useRef, useState } from "react";
import { App as AntdApp, Button, Input, Space, Spin, Typography } from "antd";
import { useZennotes } from "../hooks/useZennotes";

const { Text, Title } = Typography;

/**
 * Sección «ZenNotes» de la pestaña **API Keys** del diálogo de ajustes.
 *
 * Es independiente del formulario de ajustes (no usa `updateSettings`): el
 * token es una clave sensible, se excluye de `GET /api/settings` y
 * `PUT /api/settings` lo ignora, así que se guarda por su endpoint dedicado
 * (`PUT /api/zennotes/token`). El campo **nunca** muestra el valor guardado: su
 * estado arranca vacío y solo refleja lo que teclea el usuario. El estado de
 * configuración sale de `GET /api/zennotes/status` (que jamás devuelve el token).
 */
export function ZennotesSection() {
  const { message } = AntdApp.useApp();
  const { status, loading, error, saving, saveToken } = useZennotes();
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
        {status?.configured ? "Configurado" : "No configurado"}
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
            onPressEnter={() => void handleSave()}
          />
          <Button type="primary" loading={saving} onClick={() => void handleSave()}>
            Guardar token
          </Button>
        </Space.Compact>
      </Space>
    </section>
  );
}
