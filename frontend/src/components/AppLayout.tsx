import React, { lazy, Suspense, useState, useEffect, useContext } from "react";
import { Layout, Typography, Button, Space, Modal, Spin } from "antd";
import {
  SettingOutlined,
  CalendarOutlined,
  CheckSquareOutlined,
  BarChartOutlined,
  LogoutOutlined,
} from "@ant-design/icons";
import { ChatView } from "./ChatView";
import { useMainChat } from "../hooks/useMainChat";
import { useSettings } from "../hooks/useSettings";
import { useProfileContext } from "../contexts/ProfileContext";
import { AuthContext } from "../contexts/AuthContext";

// Vistas que se abren bajo demanda: se cargan con `React.lazy` para que no
// formen parte del bundle inicial. `ChatView` (vista por defecto) es estático.
const CalendarView = lazy(() =>
  import("./CalendarView").then((m) => ({ default: m.CalendarView })),
);
const TaskView = lazy(() =>
  import("./TaskView").then((m) => ({ default: m.TaskView })),
);
const StatsDashboard = lazy(() =>
  import("../pages/StatsDashboard").then((m) => ({
    default: m.StatsDashboard,
  })),
);
const SettingsDialog = lazy(() =>
  import("./SettingsDialog").then((m) => ({ default: m.SettingsDialog })),
);

// Fallback de carga de las vistas diferidas. antd ya está cargado en este punto
// (AppLayout vive en el chunk de la app autenticada), así que un `Spin` centrado
// da señal visible mientras se descarga el chunk de turno —p. ej. `vendor-charts`
// (~64 kB gzip) la primera vez que se abre Stats— sin coste extra de bundle.
const lazyViewFallback = (
  <div style={{ display: "flex", justifyContent: "center", padding: 24 }}>
    <Spin />
  </div>
);

const { Header, Content } = Layout;
const { Text } = Typography;

interface AppLayoutProps {
  children?: React.ReactNode;
}

export const AppLayout: React.FC<AppLayoutProps> = ({ children }) => {
  const mainChat = useMainChat();
  const { settings } = useSettings();
  const { profile } = useProfileContext();
  // Se consume el contexto directamente (en vez de `useAuth()`) para que
  // `AppLayout` siga siendo renderizable sin `<AuthProvider>` —hay tests de
  // layout que no lo montan— y solo muestre el logout si hay sesión.
  const auth = useContext(AuthContext);
  const logout = auth?.logout;
  const authenticated = auth?.user != null;
  const loggingOut = auth?.loggingOut ?? false;

  // Apply font-size as CSS variable on root element
  const fontSize = settings?.font_size ? parseInt(settings.font_size, 10) : 16;
  useEffect(() => {
    document.documentElement.style.setProperty(
      "--font-size-base",
      `${fontSize}px`,
    );
  }, [fontSize]);
  const [settingsVisible, setSettingsVisible] = useState(false);
  // `SettingsDialog` se renderiza siempre en el árbol, así que un `lazy` a secas
  // lo cargaría al montar. Se difiere hasta el primer uso: al pulsar el
  // engranaje se marca `settingsLoaded` y, a partir de ahí, queda montado (se
  // conserva la animación de cierre del modal).
  const [settingsLoaded, setSettingsLoaded] = useState(false);
  const [calendarVisible, setCalendarVisible] = useState(false);
  const [tasksVisible, setTasksVisible] = useState(false);
  const [statsVisible, setStatsVisible] = useState(false);
  const [statsOpenKey, setStatsOpenKey] = useState(0);

  return (
    <Layout style={{ minHeight: "100vh" }}>
      <Header
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          padding: "0 24px",
          background: "#1a1a2e",
          borderBottom: "1px solid rgba(255,255,255,0.1)",
        }}
      >
        <Text strong style={{ color: "#fff", fontSize: 18 }}>
          💬 Valet
        </Text>
        <Space>
          <Button
            type="text"
            icon={<CheckSquareOutlined />}
            onClick={() => setTasksVisible(true)}
            style={{ color: "rgba(255,255,255,0.65)" }}
          />
          <Button
            type="text"
            icon={<CalendarOutlined />}
            onClick={() => setCalendarVisible(true)}
            style={{ color: "rgba(255,255,255,0.65)" }}
          />
          <Button
            type="text"
            icon={<BarChartOutlined />}
            onClick={() => setStatsVisible(true)}
            style={{ color: "rgba(255,255,255,0.65)" }}
          />
          <Button
            type="text"
            icon={<SettingOutlined />}
            onClick={() => {
              setSettingsLoaded(true);
              setSettingsVisible(true);
            }}
            style={{ color: "rgba(255,255,255,0.65)" }}
          />
          {authenticated && logout && (
            <Button
              type="text"
              icon={<LogoutOutlined />}
              aria-label="Cerrar sesión"
              loading={loggingOut}
              disabled={loggingOut}
              onClick={() => {
                void logout();
              }}
              style={{ color: "rgba(255,255,255,0.65)" }}
            />
          )}
        </Space>
      </Header>
      <Content
        style={{
          padding: 0,
          background: "#000",
          height: "calc(100vh - 64px)",
          overflow: "hidden",
        }}
      >
        {children || (
          <ChatView
            messages={mainChat.messages}
            loading={mainChat.loading}
            onSendMessage={mainChat.sendMessage}
            streaming={mainChat.streaming}
            streamingContent={mainChat.streamingContent}
            activeTools={mainChat.activeTools}
            settings={settings}
            userAvatarUrl={profile?.avatar_url ?? null}
            pendingApproval={mainChat.pendingApproval}
            onResolveApproval={mainChat.resolveApproval}
            widgetsByMessage={mainChat.widgetsByMessage}
            onWidgetAction={mainChat.sendWidgetAction}
          />
        )}
      </Content>
      <Modal
        title="📅 Agenda"
        open={calendarVisible}
        onCancel={() => setCalendarVisible(false)}
        footer={null}
        width={900}
      >
        <Suspense fallback={lazyViewFallback}>
          <CalendarView />
        </Suspense>
      </Modal>
      <Modal
        title="✅ Tasks"
        open={tasksVisible}
        onCancel={() => setTasksVisible(false)}
        footer={null}
        width={1000}
      >
        <Suspense fallback={lazyViewFallback}>
          <TaskView onClose={() => setTasksVisible(false)} />
        </Suspense>
      </Modal>
      <Modal
        title="📊 Stats"
        open={statsVisible}
        onCancel={() => setStatsVisible(false)}
        footer={null}
        width={1000}
        afterOpenChange={(open) => {
          if (open) setStatsOpenKey((k) => k + 1);
        }}
      >
        <Suspense fallback={lazyViewFallback}>
          <StatsDashboard key={statsOpenKey} />
        </Suspense>
      </Modal>
      {settingsLoaded && (
        <Suspense fallback={lazyViewFallback}>
          <SettingsDialog
            visible={settingsVisible}
            onClose={() => setSettingsVisible(false)}
          />
        </Suspense>
      )}
    </Layout>
  );
};
