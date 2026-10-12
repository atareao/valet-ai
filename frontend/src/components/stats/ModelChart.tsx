import React from "react";
import { Card, Table, Skeleton } from "antd";
import {
  Chart as ChartJS,
  CategoryScale,
  LinearScale,
  BarElement,
  Title,
  Tooltip,
  Legend,
} from "chart.js";
import { Bar } from "react-chartjs-2";
import type { ModelStats } from "../../types";

ChartJS.register(CategoryScale, LinearScale, BarElement, Title, Tooltip, Legend);

interface ModelChartProps {
  data: ModelStats[];
  loading: boolean;
}

export const ModelChart: React.FC<ModelChartProps> = ({ data, loading }) => {
  if (loading) {
    return (
      <Card title="Cost by Model (all origins)" style={{ marginBottom: 16 }}>
        <Skeleton active paragraph={{ rows: 6 }} />
      </Card>
    );
  }

  if (!data || data.length === 0) {
    return (
      <Card title="Cost by Model (all origins)" style={{ marginBottom: 16 }}>
        <div style={{ textAlign: "center", padding: "24px 0", color: "rgba(255,255,255,0.45)" }}>
          No data yet
        </div>
      </Card>
    );
  }

  const chartData = {
    labels: data.map((m) => m.model),
    datasets: [
      {
        label: "Cost ($)",
        data: data.map((m) => m.total_cost),
        backgroundColor: "rgba(22, 119, 255, 0.6)",
        borderColor: "rgba(22, 119, 255, 1)",
        borderWidth: 1,
      },
    ],
  };

  const options = {
    indexAxis: "y" as const,
    responsive: true,
    plugins: {
      legend: { display: false },
      title: { display: false },
    },
    scales: {
      x: {
        ticks: { color: "rgba(255,255,255,0.65)" },
        grid: { color: "rgba(255,255,255,0.1)" },
      },
      y: {
        ticks: { color: "rgba(255,255,255,0.65)" },
        grid: { color: "rgba(255,255,255,0.1)" },
      },
    },
  };

  const headerStyle = { color: "#ffffff" };
  const columns = [
    { title: "Model", dataIndex: "model", key: "model", onHeaderCell: () => ({ style: headerStyle }) },
    { title: "Calls", dataIndex: "calls", key: "calls", onHeaderCell: () => ({ style: headerStyle }) },
    { title: "Tokens", dataIndex: "total_tokens", key: "total_tokens", onHeaderCell: () => ({ style: headerStyle }) },
    {
      title: "Cost ($)",
      dataIndex: "total_cost",
      key: "total_cost",
      onHeaderCell: () => ({ style: headerStyle }),
      render: (val: number) => val.toFixed(6),
    },
    {
      title: "Avg Duration",
      dataIndex: "avg_duration_ms",
      key: "avg_duration_ms",
      onHeaderCell: () => ({ style: headerStyle }),
      render: (val: number | null) =>
        val != null ? `${val.toFixed(0)} ms` : "N/A",
    },
  ];

  return (
    <Card title="Cost by Model (all origins)" style={{ marginBottom: 16 }}>
      <div style={{ maxHeight: 300, marginBottom: 16 }}>
        <Bar data={chartData} options={options} />
      </div>
      <Table
        dataSource={data}
        columns={columns}
        rowKey="model"
        pagination={false}
        size="small"
      />
    </Card>
  );
};