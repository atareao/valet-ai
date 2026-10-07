import { BrowserRouter, Routes, Route } from "react-router-dom";
import { ConfigProvider, App as AntdApp } from "antd";
import { valetTheme } from "./theme";
import { AppLayout } from "./components/AppLayout";
import { ProfileProvider } from "./contexts/ProfileProvider";

/**
 * Aplicación autenticada. `App.tsx` la importa dinámicamente (React.lazy) para
 * que antd, el router y las vistas no formen parte del bundle inicial.
 */
export default function AuthenticatedApp() {
  return (
    <BrowserRouter>
      <ConfigProvider theme={valetTheme}>
        <AntdApp>
          <ProfileProvider>
            <Routes>
              <Route path="*" element={<AppLayout />} />
            </Routes>
          </ProfileProvider>
        </AntdApp>
      </ConfigProvider>
    </BrowserRouter>
  );
}
