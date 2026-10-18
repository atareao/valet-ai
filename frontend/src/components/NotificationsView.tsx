import React from "react";
import { Button, Card, List, Space, Tag, Typography } from "antd";
import { useNotifications } from "../hooks/useNotifications";
import { parseSqliteTimestamp } from "../utils/sqliteTimestamp";
import type { Notification } from "../types";

const { Text } = Typography;

/** Color del `Tag` de nivel; coherente con la severidad del aviso. */
const LEVEL_COLORS: Record<Notification["level"], string> = {
  info: "blue",
  warning: "orange",
  critical: "red",
};

/**
 * Fecha de creación: el crudo UTC de SQLite se interpreta como UTC y el texto
 * se pinta en la hora local del usuario dentro de un `<time>` cuyo `dateTime`
 * expone el instante UTC. Un valor no parseable se muestra como ausencia —`—`—,
 * nunca como «Invalid Date» ni el valor crudo.
 */
const formatCreatedAt = (value: string): React.ReactNode => {
  const dt = parseSqliteTimestamp(value);
  if (!dt) return "—";
  return <time dateTime={dt.toISOString()}>{dt.toLocaleString()}</time>;
};

const countLabel = (total: number): string =>
  total === 1 ? `${total} aviso pendiente` : `${total} avisos pendientes`;

/**
 * Bandeja de avisos pendientes (`useNotifications`): recuento, lista con nivel
 * y fecha, acciones de marcado y controles de paginación por cursor.
 */
export const NotificationsView: React.FC = () => {
  const {
    notifications,
    total,
    loading,
    error,
    hasPrev,
    hasNext,
    goNext,
    goPrev,
    markRead,
    markAll,
  } = useNotifications();

  if (loading) {
    return <Card loading />;
  }

  if (error) {
    return (
      <Card>
        <Text type="danger">{error}</Text>
      </Card>
    );
  }

  return (
    <Card>
      <Space direction="vertical" size="middle" style={{ width: "100%" }}>
        <Space
          style={{ width: "100%", justifyContent: "space-between" }}
          align="center"
        >
          <Text strong>{countLabel(total)}</Text>
          <Button
            disabled={notifications.length === 0}
            onClick={() => {
              void markAll();
            }}
          >
            Marcar todos como leídos
          </Button>
        </Space>

        {notifications.length === 0 ? (
          <div
            role="status"
            style={{
              textAlign: "center",
              padding: "24px 0",
              color: "rgba(0,0,0,0.45)",
            }}
          >
            No hay avisos pendientes
          </div>
        ) : (
          <List
            dataSource={notifications}
            rowKey={(n) => n.id}
            renderItem={(n) => (
              <List.Item
                actions={[
                  <Button
                    key="read"
                    size="small"
                    onClick={() => {
                      void markRead(n.id);
                    }}
                  >
                    Marcar como leído
                  </Button>,
                ]}
              >
                <List.Item.Meta
                  title={
                    <Space>
                      <Tag color={LEVEL_COLORS[n.level]}>{n.level}</Tag>
                      <Text strong>{n.origin}</Text>
                      <Text type="secondary">{n.author}</Text>
                    </Space>
                  }
                  description={
                    <Space direction="vertical" size={0}>
                      <Text>{n.message}</Text>
                      <Text type="secondary">
                        {formatCreatedAt(n.created_at)}
                      </Text>
                    </Space>
                  }
                />
              </List.Item>
            )}
          />
        )}

        <Space>
          <Button onClick={goPrev} disabled={!hasPrev || loading}>
            Anterior
          </Button>
          <Button onClick={goNext} disabled={!hasNext || loading}>
            Siguiente
          </Button>
        </Space>
      </Space>
    </Card>
  );
};
