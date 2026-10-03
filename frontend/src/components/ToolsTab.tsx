import { useEffect } from "react";
import { App as AntdApp, Empty, List, Spin, Switch } from "antd";
import { useTools } from "../hooks/useTools";

const TOGGLE_ERROR_MESSAGE = "Error al cambiar la herramienta";

/**
 * Panel de la pestaña «Herramientas»: lista las tools registradas y permite
 * activarlas/desactivarlas. El aviso de error se emite por dos vías sin
 * duplicarse: si `toggle` rechazara (mocks de test), lo captura el `try/catch`;
 * en producción el hook fija `error` sin re-lanzar, y el `useEffect` lo anuncia.
 */
export function ToolsTab() {
  const { message } = AntdApp.useApp();
  const { tools, loading, error, toggle } = useTools();

  useEffect(() => {
    if (error) {
      message.error(TOGGLE_ERROR_MESSAGE);
    }
  }, [error, message]);

  if (loading) {
    return <Spin />;
  }

  if (tools.length === 0) {
    return <Empty description="No hay herramientas" />;
  }

  return (
    <List
      itemLayout="horizontal"
      dataSource={tools}
      rowKey="id"
      renderItem={(tool) => (
        <List.Item
          actions={[
            <Switch
              key="toggle"
              aria-label={tool.name}
              checked={tool.enabled}
              onChange={() => {
                void (async () => {
                  try {
                    await toggle(tool.id);
                  } catch {
                    message.error(TOGGLE_ERROR_MESSAGE);
                  }
                })();
              }}
            />,
          ]}
        >
          <List.Item.Meta title={tool.name} description={tool.description} />
        </List.Item>
      )}
    />
  );
}