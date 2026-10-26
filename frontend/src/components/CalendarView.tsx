import React, { useState, useMemo, type ReactNode } from "react";
import {
  App as AntdApp,
  Calendar,
  Badge,
  Select,
  Button,
  Space,
  Typography,
  Spin,
  Alert,
} from "antd";
import { PlusOutlined } from "@ant-design/icons";
import dayjs from "dayjs";
import type { Dayjs } from "dayjs";
import type { CalendarEvent } from "../types";
import { useEvents, useDeleteEvent } from "../hooks/useEvents";
import { useMediaQuery } from "../hooks/useMediaQuery";
import { EventModal } from "./EventModal";
import { EventDetail } from "./EventDetail";
import { CATEGORY_COLORS } from "./calendar.constants";

const { Text, Title } = Typography;

const CATEGORY_LABELS: Record<string, string> = {
  default: "Default",
  work: "Work",
  personal: "Personal",
  health: "Health",
  birthday: "Birthday",
  holiday: "Holiday",
};

interface CalendarViewProps {
  onClose?: () => void;
}

export const CalendarView: React.FC<CalendarViewProps> = ({
  onClose: _onClose,
}) => {
  const { message } = AntdApp.useApp();
  const [currentDate, setCurrentDate] = useState(dayjs());
  const [categoryFilter, setCategoryFilter] = useState<string | undefined>(
    undefined,
  );
  const [selectedDate, setSelectedDate] = useState<Dayjs | null>(null);
  const [eventModalOpen, setEventModalOpen] = useState(false);
  const [editingEvent, setEditingEvent] = useState<CalendarEvent | null>(null);
  const [detailEvent, setDetailEvent] = useState<CalendarEvent | null>(null);
  const [detailOpen, setDetailOpen] = useState(false);

  const isMobile = useMediaQuery("(max-width: 767px)");

  // Fetch events for the visible month (expand to full weeks around it).
  // El rango se pide en hora local del navegador y como RFC 3339 con su offset
  // (`format()` sin argumentos), en un intervalo semiabierto: desde el inicio
  // del primer día visible hasta el **inicio del día siguiente** al último
  // visible. Formatear como `YYYY-MM-DD` (lo anterior) perdía la hora y dejaba
  // fuera todo evento que terminase el último día visible cuando este caía en
  // sábado (octubre de 2026); `endOf("week")` tampoco vale porque devuelve
  // `23:59:59.999` y al formatear se pierden los milisegundos.
  // `startOf("week")` ya cae en el inicio del día, así que el límite inferior
  // es el inicio del primer día visible sin más ajuste.
  const monthStart = currentDate.startOf("month").startOf("week").format();
  const monthEnd = currentDate
    .endOf("month")
    .endOf("week")
    .startOf("day")
    .add(1, "day")
    .format();

  const { events, loading, error, refetch } = useEvents(monthStart, monthEnd);
  const { delete: deleteEvent } = useDeleteEvent();

  const filteredEvents = useMemo(() => {
    if (!categoryFilter) return events;
    return events.filter((e) => e.category === categoryFilter);
  }, [events, categoryFilter]);

  const dayEvents = useMemo(() => {
    if (!selectedDate) return [];
    const dateStr = selectedDate.format("YYYY-MM-DD");
    return filteredEvents.filter(
      (e) => dayjs(e.start_time).format("YYYY-MM-DD") === dateStr,
    );
  }, [filteredEvents, selectedDate]);

  const getListData = (value: Dayjs) => {
    const dateStr = value.format("YYYY-MM-DD");
    return filteredEvents.filter(
      (e) => dayjs(e.start_time).format("YYYY-MM-DD") === dateStr,
    );
  };

  const cellRender = (
    current: Dayjs,
    info: { type: string; originNode: ReactNode },
  ): ReactNode => {
    if (info.type !== "date") return info.originNode;
    const listData = getListData(current);
    return (
      <ul style={{ listStyle: "none", padding: 0, margin: 0 }}>
        {listData.slice(0, 3).map((item) => (
          <li key={item.id} style={{ marginBottom: 1 }}>
            <Badge
              color={CATEGORY_COLORS[item.category] || CATEGORY_COLORS.default}
              text={
                <Text
                  style={{
                    fontSize: 11,
                    cursor: "pointer",
                    color: "rgba(255,255,255,0.85)",
                  }}
                  onClick={(e) => {
                    e.stopPropagation();
                    setDetailEvent(item);
                    setDetailOpen(true);
                  }}
                >
                  {item.title}
                </Text>
              }
            />
          </li>
        ))}
        {listData.length > 3 && (
          <li>
            <Text style={{ fontSize: 11, color: "rgba(255,255,255,0.45)" }}>
              +{listData.length - 3} more
            </Text>
          </li>
        )}
      </ul>
    );
  };

  const handleCreateEvent = () => {
    setEditingEvent(null);
    setEventModalOpen(true);
  };

  const handleEditEvent = (event: CalendarEvent) => {
    setEditingEvent(event);
    setEventModalOpen(true);
    setDetailOpen(false);
  };

  const handleModalClose = () => {
    setEventModalOpen(false);
    setEditingEvent(null);
  };

  const handleEventSaved = () => {
    handleModalClose();
    setDetailOpen(false);
    refetch();
  };

  const handleEventDeleted = async (id: string) => {
    try {
      await deleteEvent(id);
      setDetailOpen(false);
      refetch();
    } catch (err) {
      message.error(err instanceof Error ? err.message : String(err));
    }
  };

  if (error) {
    return (
      <Alert type="error" message={error} showIcon style={{ margin: 16 }} />
    );
  }

  return (
    <div style={{ padding: 16, minHeight: 400 }}>
      <Space direction="vertical" style={{ width: "100%" }} size="middle">
        <div
          style={{
            display: "flex",
            justifyContent: "space-between",
            alignItems: "center",
            flexWrap: "wrap",
            gap: 8,
          }}
        >
          <Title
            level={4}
            style={{ margin: 0, color: "rgba(255,255,255,0.85)" }}
          >
            Calendar
          </Title>
          <Space
            direction={isMobile ? "vertical" : "horizontal"}
            style={{ width: isMobile ? "100%" : undefined }}
          >
            <Select
              allowClear
              placeholder="Filter by category"
              style={{ width: isMobile ? "100%" : 160 }}
              value={categoryFilter}
              onChange={setCategoryFilter}
              options={Object.entries(CATEGORY_LABELS).map(
                ([value, label]) => ({
                  value,
                  label: (
                    <span>
                      <Badge color={CATEGORY_COLORS[value]} /> {label}
                    </span>
                  ),
                }),
              )}
            />
            <Button
              type="primary"
              icon={<PlusOutlined />}
              onClick={handleCreateEvent}
              block={isMobile}
            >
              New Event
            </Button>
          </Space>
        </div>

        {loading ? (
          <div style={{ textAlign: "center", padding: 40 }}>
            <Spin size="large" />
          </div>
        ) : (
          <Calendar
            value={currentDate}
            onSelect={(date) => setSelectedDate(date)}
            onPanelChange={(date) => setCurrentDate(date)}
            cellRender={cellRender}
            fullscreen={!isMobile}
          />
        )}

        {selectedDate && dayEvents.length > 0 && (
          <div>
            <Title level={5} style={{ color: "rgba(255,255,255,0.85)" }}>
              Events for {selectedDate.format("dddd, MMMM D, YYYY")}
            </Title>
            {dayEvents.map((event) => (
              <div
                key={event.id}
                style={{
                  padding: "8px 12px",
                  marginBottom: 6,
                  borderRadius: 6,
                  background: "#1f1f1f",
                  borderLeft: `4px solid ${CATEGORY_COLORS[event.category] || CATEGORY_COLORS.default}`,
                  cursor: "pointer",
                }}
                onClick={() => {
                  setDetailEvent(event);
                  setDetailOpen(true);
                }}
              >
                {isMobile ? (
                  <div>
                    <div
                      style={{
                        color: "rgba(255,255,255,0.85)",
                        fontWeight: 600,
                        marginBottom: 4,
                      }}
                    >
                      <Badge
                        color={
                          CATEGORY_COLORS[event.category] ||
                          CATEGORY_COLORS.default
                        }
                      />{" "}
                      {event.title}
                    </div>
                    <div
                      style={{ color: "rgba(255,255,255,0.45)", fontSize: 12 }}
                    >
                      {dayjs(event.start_time).format("HH:mm")} -{" "}
                      {dayjs(event.end_time).format("HH:mm")}
                    </div>
                  </div>
                ) : (
                  <Space>
                    <Badge
                      color={
                        CATEGORY_COLORS[event.category] ||
                        CATEGORY_COLORS.default
                      }
                    />
                    <Text strong style={{ color: "rgba(255,255,255,0.85)" }}>
                      {event.title}
                    </Text>
                    <Text
                      style={{ color: "rgba(255,255,255,0.45)", fontSize: 12 }}
                    >
                      {dayjs(event.start_time).format("HH:mm")} -{" "}
                      {dayjs(event.end_time).format("HH:mm")}
                    </Text>
                  </Space>
                )}
              </div>
            ))}
          </div>
        )}
      </Space>

      <EventModal
        open={eventModalOpen}
        event={editingEvent}
        defaultDate={selectedDate}
        onClose={handleModalClose}
        onSaved={handleEventSaved}
      />

      <EventDetail
        event={detailEvent}
        open={detailOpen}
        onClose={() => setDetailOpen(false)}
        onEdit={handleEditEvent}
        onDelete={handleEventDeleted}
      />
    </div>
  );
};
