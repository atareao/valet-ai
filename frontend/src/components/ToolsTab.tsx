import { useEffect, type CSSProperties } from "react";
import { App as AntdApp, Empty, List, Spin, Switch } from "antd";
import { useTools } from "../hooks/useTools";

const TOGGLE_ERROR_MESSAGE = "Error al cambiar la herramienta";

// Limita la lista a 60vh para que, con muchas tools, las filas hagan scroll
// vertical en lugar de desbordar la ventana del diálogo.
const TOOLS_LIST_SCROLL_STYLE: CSSProperties = {
  maxHeight: "60vh",
  overflowY: "auto",
  overflowX: "hidden",
  paddingRight: 8,
};

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
    <div
      role="region"
      aria-label="Lista de herramientas"
      tabIndex={0}
      className="tools-scroll-region"
      style={TOOLS_LIST_SCROLL_STYLE}
    >
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
    </div>
  );
}