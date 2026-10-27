import { useEffect, useRef, useState } from "react";
import { App as AntdApp, Button, Form, Input, Space, Spin, Typography } from "antd";
import { useOpentodo } from "../hooks/useOpentodo";
import type { UseOpentodoReturn } from "../hooks/useOpentodo";
import { UNAVAILABLE_LABEL } from "./integrationStatus";
import type { OpentodoCheckResult } from "../types";

const { Text, Title } = Typography;

export interface OpentodoSectionProps {
  /**
   * Estado ya cargado por el contenedor («Integraciones»). Si llega, la sección
   * **no** llama a `useOpentodo()`: el indicador de la sub-pestaña y el panel
   * comparten una única lectura de `GET /api/opentodo/status`. Si falta, el
   * envoltorio la hace por su cuenta (compatibilidad con quien monte la
   * sección suelta).
   */
  opentodoState?: UseOpentodoReturn;
  /**
   * `true` mientras se persisten los ajustes del formulario compartido. Se
   * reenvía al botón «Guardar» (submit de ese formulario) para impedir un doble
   * `PUT /api/settings`; igual que hace `KeyIntegration`.
   */
  saving?: boolean;
}

/**
 * Sección «OpenTodo» de la octava sub-pestaña de **Integraciones** del diálogo
 * de ajustes.
 *
 * Combina dos vías de persistencia bien separadas:
 *
 * - La **URL base** (`opentodo_base_url`) es un ajuste normal: se declara con
 *   `Form.Item` sobre el formulario de ajustes del padre y se guarda con el
 *   `PUT /api/settings` de su botón «Guardar» (submit del formulario), igual
 *   que las URL base de apimail y Miniflux.
 * - El **token** (`opentodo_token`) es una clave sensible: se excluye de
 *   `GET /api/settings`, `PUT /api/settings` lo ignora y se guarda por su
 *   endpoint dedicado (`PUT /api/opentodo/token`). El campo **nunca** muestra el
 *   valor guardado: su estado arranca vacío y solo refleja lo que teclea el
 *   usuario. El estado de configuración sale de `GET /api/opentodo/status` (que
 *   jamás devuelve el token).
 *
 * «Probar conexión» **no** relee el estado local: sondea la instancia remota con
 * `GET /api/opentodo/check` (que responde siempre `200`) y muestra el `detail`
 * del resultado en la línea de estado —además del toast—, porque el resultado de
 * una acción no puede quedar solo en un aviso efímero.
 *
 * Envoltorio: con `opentodoState` renderiza la vista sin consultar el endpoint;
 * sin él, delega en `OpentodoSectionConnected`, que llama al hook.
 */
export function OpentodoSection(props: OpentodoSectionProps) {
  if (props.opentodoState) {
    return (
      <OpentodoSectionView
        state={props.opentodoState}
        saving={props.saving ?? false}
      />
    );
  }
  return <OpentodoSectionConnected saving={props.saving ?? false} />;
}

function OpentodoSectionConnected({ saving }: { saving: boolean }) {
  const state = useOpentodo();
  return <OpentodoSectionView state={state} saving={saving} />;
}

function OpentodoSectionView({
  state,
  saving,
}: {
  state: UseOpentodoReturn;
  saving: boolean;
}) {
  const { message } = AntdApp.useApp();
  const { status, loading, error, saving: savingToken, saveToken, check, checking } =
    state;
  const [token, setToken] = useState("");
  const [checkResult, setCheckResult] = useState<OpentodoCheckResult | null>(null);

  // Los manejadores resuelven de forma asíncrona: solo aplican su `setState`
  // si el componente sigue montado. Mismo patrón que `mountedRef` en el hook.
  const mountedRef = useRef(true);
  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const handleSaveToken = async () => {
    const trimmed = token.trim();
    if (trimmed === "") {
      message.warning("Escribe un token de OpenTodo antes de guardar");
      return;
    }
    try {
      await saveToken(trimmed);
      // El token guardado no se relee: el campo se limpia siempre tras guardar.
      if (mountedRef.current) setToken("");
      message.success("Token de OpenTodo guardado");
    } catch {
      message.error("Error al guardar el token de OpenTodo");
    }
  };

  /**
   * «Probar conexión» sondea la instancia remota (`GET /api/opentodo/check`) y
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
        message.success("Conexión con OpenTodo correcta");
      } else {
        message.error("La comprobación con OpenTodo ha fallado");
      }
    } catch {
      if (mountedRef.current) {
        setCheckResult({
          ok: false,
          detail: "No se pudo comprobar la conexión con OpenTodo",
        });
      }
      message.error("No se pudo comprobar la conexión con OpenTodo");
    }
  };

  const renderStatus = () => {
    if (loading && !status) {
      return (
        <div role="status" aria-label="Cargando estado de OpenTodo">
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
      return (
        <Text type="warning">{UNAVAILABLE_LABEL}</Text>
      );
    }
    return <Text type="secondary">{UNAVAILABLE_LABEL}</Text>;
  };

  return (
    <section aria-label="OpenTodo">
      <Title level={5} style={{ marginTop: 0 }}>
        OpenTodo
      </Title>
      <Text type="secondary">
        URL base y token de la API de OpenTodo para gestionar las tareas. Por
        seguridad, el token guardado nunca se muestra: para cambiarlo, escribe
        uno nuevo y guarda.
      </Text>

      <Space
        direction="vertical"
        size="middle"
        style={{ width: "100%", marginTop: 16 }}
      >
        <Space direction="vertical" size={4} style={{ width: "100%" }}>
          <Text>Estado</Text>
          {/* La misma región viva reúne el estado de configuración y el
              resultado del sondeo: ambos son información de estado. */}
          <div aria-live="polite">
            <Space direction="vertical" size={2} style={{ width: "100%" }}>
              {renderStatus()}
              {checkResult && (
                <Text type={checkResult.ok ? "success" : "warning"}>
                  {checkResult.detail}
                </Text>
              )}
            </Space>
          </div>
        </Space>

        {/* La URL base es un ajuste normal del formulario compartido: viaja en
            el `PUT /api/settings` del botón «Guardar». */}
        <Form.Item label="OpenTodo · URL base" name="opentodo_base_url">
          <Input placeholder="https://todo.territoriolinux.es" />
        </Form.Item>

        {/* El token es sensible y se guarda por su endpoint dedicado. */}
        <Space.Compact style={{ width: "100%" }}>
          <Input.Password
            aria-label="OpenTodo · token de API"
            placeholder="Escribe un token para configurar OpenTodo"
            autoComplete="off"
            value={token}
            onChange={(event) => setToken(event.target.value)}
            onPressEnter={(event) => {
              // El input cuelga del formulario de ajustes del padre: sin esto,
              // Intro dispararía además el submit implícito de ese form
              // (`PUT /api/settings` + cierre del diálogo) y el token se
              // guardaría por dos vías a la vez.
              event.preventDefault();
              void handleSaveToken();
            }}
          />
          <Button
            type="primary"
            loading={savingToken}
            aria-busy={savingToken}
            onClick={() => void handleSaveToken()}
          >
            Guardar token
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
