import React from "react";
import { Card, Skeleton, Table, Tag } from "antd";
import type { TableColumnsType } from "antd";
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

const formatLatency = (ms: number | null): string =>
  ms != null ? `${Math.round(ms)} ms` : "N/A";

const headerStyle = { color: "#ffffff" };

const columns: TableColumnsType<BackgroundStats> = [
  {
    title: "Origin",
    dataIndex: "kind",
    key: "kind",
    onHeaderCell: () => ({ style: headerStyle }),
    render: (kind: string) => (
      <>
        <span style={{ fontWeight: 600 }}>{originLabel(kind)}</span>{" "}
        <Tag>{kind}</Tag>
      </>
    ),
  },
  {
    title: "Calls",
    dataIndex: "calls",
    key: "calls",
    onHeaderCell: () => ({ style: headerStyle }),
  },
  {
    title: "Tokens",
    dataIndex: "total_tokens",
    key: "total_tokens",
    onHeaderCell: () => ({ style: headerStyle }),
    render: (tokens: number) => tokens.toLocaleString("en-US"),
  },
  {
    title: "Cost",
    dataIndex: "total_cost",
    key: "total_cost",
    onHeaderCell: () => ({ style: headerStyle }),
    render: (cost: number) => `$${cost.toFixed(6)}`,
  },
  {
    title: "Avg Duration",
    dataIndex: "avg_duration_ms",
    key: "avg_duration_ms",
    onHeaderCell: () => ({ style: headerStyle }),
    render: (ms: number | null) => formatLatency(ms),
  },
  {
    title: "Errors",
    dataIndex: "total_errors",
    key: "total_errors",
    onHeaderCell: () => ({ style: headerStyle }),
  },
];

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
      <Table
        dataSource={data}
        columns={columns}
        rowKey="kind"
        pagination={false}
        size="small"
      />
    </Card>
  );
};
