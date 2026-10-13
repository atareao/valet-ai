import React from "react";
import { Card, Skeleton, Table, Tag } from "antd";
import type { TableColumnsType } from "antd";
import type { BackgroundStats } from "../../types";
import { parseSqliteTimestamp } from "./sqliteTimestamp";

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
  timeline: "Línea temporal",
};

const originLabel = (kind: string): string => ORIGIN_LABELS[kind] ?? kind;

/** Texto legible para una pareja sin modelo registrado (origen sin filas). */
const NO_MODEL_LABEL = "Sin modelo";

const formatLatency = (ms: number | null): string =>
  ms != null ? `${Math.round(ms)} ms` : "N/A";

/**
 * Fecha de la última llamada. `null` (o una fecha no parseable) se muestra como
 * ausencia —`—`—, nunca como «Invalid Date» ni una celda vacía. El parseo del
 * crudo de SQLite vive en `./sqliteTimestamp` (`parseSqliteTimestamp`).
 */
const formatLastCall = (value: string | null): string => {
  const dt = parseSqliteTimestamp(value);
  return dt ? dt.toLocaleString() : "—";
};

const headerStyle = { color: "#ffffff" };

/** Clave compuesta: hay varias filas por `kind` (una por modelo). */
const rowKey = (entry: BackgroundStats): string =>
  `${entry.kind}::${entry.model ?? "__none__"}`;

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
    title: "Model",
    dataIndex: "model",
    key: "model",
    onHeaderCell: () => ({ style: headerStyle }),
    render: (model: string | null) =>
      model ? (
        <span data-testid="bg-model">{model}</span>
      ) : (
        <span
          data-testid="bg-model"
          style={{ color: "rgba(255,255,255,0.45)", fontStyle: "italic" }}
        >
          {NO_MODEL_LABEL}
        </span>
      ),
  },
  {
    title: "Calls",
    dataIndex: "calls",
    key: "calls",
    onHeaderCell: () => ({ style: headerStyle }),
    render: (calls: number) => <span data-testid="bg-calls">{calls}</span>,
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
    render: (errors: number) =>
      errors > 0 ? (
        // Marca no cromática, además del color: quien no distinga el rojo
        // percibe igualmente el icono y, con lector de pantalla, el `aria-label`.
        <span
          data-testid="bg-errors"
          data-highlighted="true"
          aria-label={`${errors} errores`}
          style={{ color: "#ff4d4f", fontWeight: 600 }}
        >
          <span aria-hidden="true">⛔ </span>
          <span>{errors}</span>
        </span>
      ) : (
        <span data-testid="bg-errors" data-highlighted="false">
          {errors}
        </span>
      ),
  },
  {
    title: "Suspicious",
    dataIndex: "suspicious_calls",
    key: "suspicious_calls",
    onHeaderCell: () => ({ style: headerStyle }),
    render: (suspicious: number) =>
      suspicious > 0 ? (
        <Tag color="warning" data-testid="bg-suspicious">
          ⚠ {suspicious}
        </Tag>
      ) : (
        <span style={{ color: "rgba(255,255,255,0.45)" }}>0</span>
      ),
  },
  {
    title: "Last Call",
    dataIndex: "last_call_at",
    key: "last_call_at",
    onHeaderCell: () => ({ style: headerStyle }),
    render: (value: string | null) => (
      <span data-testid="bg-last-call">{formatLastCall(value)}</span>
    ),
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
        rowKey={rowKey}
        pagination={false}
        size="small"
      />
    </Card>
  );
};
