import { useEffect, useRef, useState } from "react";
import { App as AntdApp, Button, Form, Input, Space, Spin, Typography } from "antd";
import { useBlog } from "../hooks/useBlog";
import type { UseBlogReturn } from "../hooks/useBlog";
import { UNAVAILABLE_LABEL } from "./integrationStatus";
import type { BlogCheckResult } from "../types";

const { Text, Title } = Typography;

export interface BlogSectionProps {
  /**
   * Estado ya cargado por el contenedor («Integraciones»). Si llega, la sección
   * **no** llama a `useBlog()`: el indicador de la sub-pestaña y el panel
   * comparten una única lectura de `GET /api/blog/status`. Si falta, el
   * envoltorio la hace por su cuenta (compatibilidad con quien monte la sección
   * suelta).
   */
  blogState?: UseBlogReturn;
  /**
   * `true` mientras se persisten los ajustes del formulario compartido. Se
   * reenvía al botón «Guardar» (submit de ese formulario) para impedir un doble
   * `PUT /api/settings`; igual que hace `OpentodoSection`.
   */
  saving?: boolean;
}

/**
 * Sección «Blog» de la décima sub-pestaña de **Integraciones** del diálogo de
 * ajustes.
 *
 * Combina dos vías de persistencia bien separadas:
 *
 * - La **URL base** (`blog_base_url`) y el **usuario del servicio**
 *   (`blog_service_user`) son ajustes normales: se declaran con `Form.Item`
 *   sobre el formulario de ajustes del padre y se guardan con el
 *   `PUT /api/settings` de su botón «Guardar» (submit del formulario).
 * - La **contraseña de aplicación** (`blog_app_password`) es una clave
 *   sensible: se excluye de `GET /api/settings`, `PUT /api/settings` la ignora
 *   y se guarda por su endpoint dedicado (`PUT /api/blog/password`). El campo
 *   **nunca** muestra el valor guardado: su estado arranca vacío y solo refleja
 *   lo que teclea el usuario. El estado de configuración sale de
 *   `GET /api/blog/status` (que jamás devuelve la contraseña).
 *
 * «Probar conexión» **no** relee el estado local: sondea la instancia remota con
 * `GET /api/blog/check` (que responde siempre `200`) y muestra el `detail` del
 * resultado en la línea de estado —además del toast—, porque el resultado de
 * una acción no puede quedar solo en un aviso efímero.
 *
 * Envoltorio: con `blogState` renderiza la vista sin consultar el endpoint; sin
 * él, delega en `BlogSectionConnected`, que llama al hook.
 */
export function BlogSection(props: BlogSectionProps) {
  if (props.blogState) {
    return (
      <BlogSectionView state={props.blogState} saving={props.saving ?? false} />
    );
  }
  return <BlogSectionConnected saving={props.saving ?? false} />;
}

function BlogSectionConnected({ saving }: { saving: boolean }) {
  const state = useBlog();
  return <BlogSectionView state={state} saving={saving} />;
}

function BlogSectionView({
  state,
  saving,
}: {
  state: UseBlogReturn;
  saving: boolean;
}) {
  const { message } = AntdApp.useApp();
  const {
    status,
    loading,
    error,
    saving: savingPassword,
    savePassword,
    check,
    checking,
  } = state;
  const [password, setPassword] = useState("");
  const [checkResult, setCheckResult] = useState<BlogCheckResult | null>(null);

  // Los manejadores resuelven de forma asíncrona: solo aplican su `setState`
  // si el componente sigue montado. Mismo patrón que `mountedRef` en el hook.
  const mountedRef = useRef(true);
  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const handleSavePassword = async () => {
    const trimmed = password.trim();
    if (trimmed === "") {
      message.warning("Escribe la contraseña de aplicación del Blog antes de guardar");
      return;
    }
    try {
      await savePassword(trimmed);
      // La contraseña guardada no se relee: el campo se limpia tras guardar.
      if (mountedRef.current) setPassword("");
      message.success("Contraseña del Blog guardada");
    } catch {
      message.error("Error al guardar la contraseña del Blog");
    }
  };

  /**
   * «Probar conexión» sondea la instancia remota (`GET /api/blog/check`) y
   * publica el resultado en la línea de estado. La ruta responde siempre `200`,
   * así que el diagnóstico viaja en `detail`; un rechazo solo ocurre si la
   * propia petición HTTP falla.
   */
  const handleCheck = async () => {
    setCheckResult(null);
    try {
      const result = await check();
      if (mountedRef.current) setCheckResult(result);
      if (result.ok) {
        message.success("Conexión con el Blog correcta");
      } else {
        message.error("La comprobación con el Blog ha fallado");
      }
    } catch {
      if (mountedRef.current) {
        setCheckResult({
          ok: false,
          detail: "No se pudo comprobar la conexión con el Blog",
        });
      }
      message.error("No se pudo comprobar la conexión con el Blog");
    }
  };

  const renderStatus = () => {
    if (loading && !status) {
      return (
        <div role="status" aria-label="Cargando estado del Blog">
          <Spin />
        </div>
      );
    }
    if (status) {
      return (
        <Text type="secondary">
          {status.configured ? "Configurado" : "Sin configurar"}
        </Text>
      );
    }
    if (error) {
      // La etiqueta de la sub-pestaña usa la misma etiqueta compartida cuando la
      // lectura falla: el panel no puede decir «Sin configurar» en su lugar.
      return <Text type="warning">{UNAVAILABLE_LABEL}</Text>;
    }
    return <Text type="secondary">{UNAVAILABLE_LABEL}</Text>;
  };

  return (
    <section aria-label="Blog">
      <Title level={5} style={{ marginTop: 0 }}>
        Blog
      </Title>
      <Text type="secondary">
        URL base, usuario del servicio y contraseña de aplicación de WordPress
        para responder a los comentarios del blog. Por seguridad, la contraseña
        guardada nunca se muestra: para cambiarla, escribe una nueva y guarda.
      </Text>

      <Space
        orientation="vertical"
        size="middle"
        style={{ width: "100%", marginTop: 16 }}
      >
        <Space orientation="vertical" size={4} style={{ width: "100%" }}>
          <Text>Estado</Text>
          {/* La misma región viva reúne el estado de configuración y el
              resultado del sondeo: ambos son información de estado. */}
          <div aria-live="polite">
            <Space orientation="vertical" size={2} style={{ width: "100%" }}>
              {renderStatus()}
              {checkResult && (
                <Text type={checkResult.ok ? "success" : "warning"}>
                  {checkResult.detail}
                </Text>
              )}
            </Space>
          </div>
        </Space>

        {/* La URL base y el usuario son ajustes normales del formulario
            compartido: viajan en el `PUT /api/settings` del botón «Guardar». */}
        <Form.Item label="Blog · URL base" name="blog_base_url">
          <Input placeholder="https://atareao.es" />
        </Form.Item>

        <Form.Item label="Blog · Usuario del servicio" name="blog_service_user">
          <Input placeholder="valet" />
        </Form.Item>

        {/* La contraseña es sensible y se guarda por su endpoint dedicado. */}
        <Space.Compact style={{ width: "100%" }}>
          <Input.Password
            aria-label="Blog · Application Password"
            placeholder="Escribe la contraseña de aplicación para configurar el Blog"
            autoComplete="off"
            value={password}
            onChange={(event) => setPassword(event.target.value)}
            onPressEnter={(event) => {
              // El input cuelga del formulario de ajustes del padre: sin esto,
              // Intro dispararía además el submit implícito de ese form
              // (`PUT /api/settings` + cierre del diálogo) y la contraseña se
              // guardaría por dos vías a la vez.
              event.preventDefault();
              void handleSavePassword();
            }}
          />
          <Button
            type="primary"
            loading={savingPassword}
            aria-busy={savingPassword}
            onClick={() => void handleSavePassword()}
          >
            Guardar contraseña
          </Button>
        </Space.Compact>

        <Space>
          {/* Botón propio de la sub-pestaña: dispara el submit del formulario
              de ajustes del padre (`PUT /api/settings`), como las demás. */}
          <Button htmlType="submit" loading={saving} aria-busy={saving}>
            Guardar
          </Button>
          <Button
            loading={checking}
            aria-busy={checking}
            onClick={() => void handleCheck()}
          >
            Probar conexión
          </Button>
        </Space>
      </Space>
    </section>
  );
}
