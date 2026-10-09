import { useState, useEffect, useCallback } from "react";
import { Row, Col, Spin, Alert, Empty, Tabs } from "antd";
import { api } from "../api/client";
import type { StatsSummary, ModelStats, DayStats, ToolStats, TableSize, MemoryStats, LastApiCall, BackgroundStats } from "../types";
import { SummaryCard } from "../components/stats/SummaryCard";
import { MemoryCard } from "../components/stats/MemoryCard";
import { BackgroundCard } from "../components/stats/BackgroundCard";
import { ModelChart } from "../components/stats/ModelChart";
import { DailyChart } from "../components/stats/DailyChart";
import { ToolsChart } from "../components/stats/ToolsChart";
import { DbSizesTable } from "../components/stats/DbSizesTable";
import { RetentionConfig } from "../components/stats/RetentionConfig";
import { LastApiCallCard } from "../components/stats/LastApiCallCard";

export const StatsDashboard: React.FC = () => {
  const [summary, setSummary] = useState<StatsSummary | null>(null);
  const [byModel, setByModel] = useState<ModelStats[]>([]);
  const [byDay, setByDay] = useState<DayStats[]>([]);
  const [tools, setTools] = useState<ToolStats[]>([]);
  const [dbSizes, setDbSizes] = useState<TableSize[]>([]);
  const [memory, setMemory] = useState<MemoryStats | null>(null);
  const [background, setBackground] = useState<BackgroundStats[]>([]);
  const [lastCall, setLastCall] = useState<LastApiCall | null>(null);
  const [loading, setLoading] = useState(true);
  const [selectedDays, setSelectedDays] = useState(30);
  const [error, setError] = useState<string | null>(null);

  const fetchData = useCallback((days: number) => {
    return Promise.all([
      api.getStatsSummary(),
      api.getStatsByModel(),
      api.getStatsByDay(days),
      api.getStatsTools(),
      api.getDbSizes(),
      api.getMemoryStats(),
      api.getStatsBackground(),
      api.getLastApiCall(),
    ])
      .then(
        ([summaryData, modelData, dayData, toolsData, dbData, memoryData, backgroundData, lastCallData]) => {
          setSummary(summaryData);
          setByModel(modelData);
          setByDay(dayData);
          setTools(toolsData);
          setDbSizes(dbData);
          setMemory(memoryData);
          setBackground(backgroundData);
          setLastCall(lastCallData);
        },
      )
      .catch((err: unknown) => {
        setError(err instanceof Error ? err.message : "Failed to load stats");
      })
      .finally(() => setLoading(false));
  }, []);

  const loadData = (days: number) => {
    setLoading(true);
    setError(null);
    void fetchData(days);
  };

  useEffect(() => {
    // La marca de "petición en curso" debe fijarse fuera del camino síncrono
    // del efecto (spec frontend-lint-zero), pero en el mismo turno, para que
    // al cambiar el rango de días las tarjetas vuelvan a mostrar su indicador
    // de carga. Programarla en un microtask resuelto deja el estado fuera del
    // cuerpo síncrono del efecto, sin necesidad de suprimir la regla.
    let cancelled = false;
    void Promise.resolve().then(() => {
      if (cancelled) return;
      setLoading(true);
      setError(null);
    });
    void fetchData(selectedDays);
    return () => {
      cancelled = true;
    };
  }, [selectedDays, fetchData]);

  const handleRangeChange = (days: number) => {
    setSelectedDays(days);
  };

  if (error) {
    return (
      <div style={{ padding: 24 }}>
        <Alert
          message="Error loading stats"
          description={error}
          type="error"
          showIcon
          action={
            <a onClick={() => loadData(selectedDays)} style={{ cursor: "pointer" }}>
              Retry
            </a>
          }
        />
      </div>
    );
  }

  if (loading && !summary && !byModel.length && !byDay.length) {
    return (
      <div
        style={{
          display: "flex",
          justifyContent: "center",
          alignItems: "center",
          minHeight: "60vh",
        }}
      >
        <Spin size="large" tip="Loading stats..." />
      </div>
    );
  }

  const hasData =
    (summary && summary.total_calls > 0) ||
    byModel.length > 0 ||
    byDay.length > 0 ||
    tools.length > 0 ||
    dbSizes.length > 0 ||
    background.some((entry) => entry.calls > 0) ||
    (memory && memory.total_memories > 0);

  if (!loading && !hasData) {
    return (
      <div style={{ padding: 24 }}>
        <Empty description="No stats data available yet" />
      </div>
    );
  }

  return (
    <div style={{ padding: 24 }}>
      <Tabs
        items={[
          {
            key: "resumen",
            label: "📊 Resumen",
            children: (
              <Row gutter={[16, 16]}>
                <Col span={24}>
                  <SummaryCard data={summary} loading={loading} />
                </Col>
                <Col span={24}>
                  <MemoryCard data={memory} loading={loading} />
                </Col>
              </Row>
            ),
          },
          {
            key: "modelos",
            label: "🤖 Modelos",
            children: (
              <Row gutter={[16, 16]}>
                <Col xs={24} lg={12}>
                  <ModelChart data={byModel} loading={loading} />
                </Col>
                <Col xs={24} lg={12}>
                  <DailyChart
                    data={byDay}
                    loading={loading}
                    onRangeChange={handleRangeChange}
                    selectedDays={selectedDays}
                  />
                </Col>
                <Col span={24}>
                  <BackgroundCard data={background} loading={loading} />
                </Col>
              </Row>
            ),
          },
          {
            key: "sistema",
            label: "⚙️ Sistema",
            children: (
              <Row gutter={[16, 16]}>
                <Col xs={24} lg={8}>
                  <ToolsChart data={tools} loading={loading} />
                </Col>
                <Col xs={24} lg={8}>
                  <DbSizesTable data={dbSizes} loading={loading} />
                </Col>
                <Col xs={24} lg={8}>
                  <RetentionConfig />
                </Col>
              </Row>
            ),
          },
          {
            key: "ultima-llamada",
            label: "📡 Última llamada",
            children: (
              <Row gutter={[16, 16]}>
                <Col span={24}>
                  <LastApiCallCard data={lastCall} loading={loading} />
                </Col>
              </Row>
            ),
          },
        ]}
      />
    </div>
  );
};