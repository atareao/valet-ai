import React from "react";
import { Card, Row, Col, Skeleton, Tag } from "antd";
import type { BackgroundStats } from "../../types";

interface BackgroundCardProps {
  data: BackgroundStats[];
  loading: boolean;
}

/** Nombre legible de cada origen de fondo. El `kind` crudo se muestra aparte. */
const ORIGIN_LABELS: Record<string, string> = {
  router: "Clasificador",
  archivist: "Archivista",
  consolidator: "Consolidador",
  collapse: "Colapso",
};

const originLabel = (kind: string): string => ORIGIN_LABELS[kind] ?? kind;

const formatCost = (cost: number): string => `$${cost.toFixed(6)}`;

const formatLatency = (ms: number | null): string =>
  ms != null ? `${ms.toFixed(0)} ms` : "N/A";

const Metric: React.FC<{ title: string; value: React.ReactNode }> = ({
  title,
  value,
}) => (
  <div>
    <div style={{ fontSize: 12, color: "rgba(255,255,255,0.45)" }}>{title}</div>
    <div style={{ fontSize: 14 }}>{value}</div>
  </div>
);

export const BackgroundCard: React.FC<BackgroundCardProps> = ({
  data,
  loading,
}) => {
  if (loading) {
    return (
      <Card title="Procesos de fondo" style={{ marginBottom: 16 }}>
        <Skeleton active paragraph={{ rows: 4 }} />
      </Card>
    );
  }

  const hasActivity = data.some((entry) => entry.calls > 0);

  if (!hasActivity) {
    return (
      <Card title="Procesos de fondo" style={{ marginBottom: 16 }}>
        <div style={{ textAlign: "center", padding: "24px 0", color: "rgba(255,255,255,0.45)" }}>
          No data yet
        </div>
      </Card>
    );
  }

  return (
    <Card title="Procesos de fondo" style={{ marginBottom: 16 }}>
      {data.map((entry) => (
        <Row
          key={entry.kind}
          gutter={[16, 16]}
          align="middle"
          style={{
            borderBottom: "1px solid rgba(255,255,255,0.08)",
            padding: "8px 0",
          }}
        >
          <Col xs={24} sm={8} md={6}>
            <span style={{ fontWeight: 600 }}>{originLabel(entry.kind)}</span>{" "}
            <Tag>{entry.kind}</Tag>
          </Col>
          <Col xs={8} sm={4} md={4}>
            <Metric title="Calls" value={entry.calls} />
          </Col>
          <Col xs={8} sm={4} md={4}>
            <Metric title="Tokens" value={entry.total_tokens} />
          </Col>
          <Col xs={8} sm={4} md={4}>
            <Metric title="Cost" value={formatCost(entry.total_cost)} />
          </Col>
          <Col xs={12} sm={4} md={3}>
            <Metric
              title="Avg Duration"
              value={formatLatency(entry.avg_duration_ms)}
            />
          </Col>
          <Col xs={12} sm={4} md={3}>
            <Metric title="Errors" value={entry.total_errors} />
          </Col>
        </Row>
      ))}
    </Card>
  );
};
