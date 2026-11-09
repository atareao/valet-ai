import { useEffect, useRef, useState } from "react";
import { App as AntdApp, Button, Form, Input, Space, Spin, Typography } from "antd";
import { useGithub } from "../hooks/useGithub";
import type { UseGithubReturn } from "../hooks/useGithub";
import { UNAVAILABLE_LABEL } from "./integrationStatus";
import type { GithubCheckResult } from "../types";

const { Text, Title } = Typography;

export interface GithubSectionProps {
  /**
   * Estado ya cargado por el contenedor («Integraciones»). Si llega, la sección
   * **no** llama a `useGithub()`: el indicador de la sub-pestaña y el panel
   * comparten una única lectura de `GET /api/github/status`. Si falta, el
   * envoltorio la hace por su cuenta (compatibilidad con quien monte la sección
   * suelta).
   */
  githubState?: UseGithubReturn;
  /**
   * `true` mientras se persisten los ajustes del formulario compartido. Se
   * reenvía al botón «Guardar» (submit de ese formulario) para impedir un doble
   * `PUT /api/settings`; igual que hace `BlogSection`.
   */
  saving?: boolean;
}

/**
 * Sección «GitHub» de la undécima sub-pestaña de **Integraciones** del diálogo
 * de ajustes.
 *
 * Combina dos vías de persistencia bien separadas:
 *
 * - El **repositorio por defecto** (`github_default_repo`) y los **labels por
 *   defecto** (`github_default_labels`, lista separada por comas) son ajustes
 *   normales: se declaran con `Form.Item` sobre el formulario de ajustes del
 *   padre y se guardan con el `PUT /api/settings` de su botón «Guardar» (submit
 *   del formulario).
 * - El **token** (`github_token`) es una clave sensible: se excluye de
 *   `GET /api/settings`, `PUT /api/settings` lo ignora y se guarda por su
 *   endpoint dedicado (`PUT /api/github/token`). El campo **nunca** muestra el
 *   valor guardado: su estado arranca vacío y solo refleja lo que teclea el
 *   usuario. El estado de configuración sale de `GET /api/github/status` (que
 *   jamás devuelve el token).
 *
 * «Probar conexión» **no** relee el estado local: sondea la API remota con
 * `GET /api/github/check` (que responde siempre `200`) y muestra el `detail`
 * del resultado en la línea de estado —además del toast—, porque el resultado de
 * una acción no puede quedar solo en un aviso efímero.
 *
 * Envoltorio: con `githubState` renderiza la vista sin consultar el endpoint;
 * sin él, delega en `GithubSectionConnected`, que llama al hook.
 */
export function GithubSection(props: GithubSectionProps) {
  if (props.githubState) {
    return (
      <GithubSectionView
        state={props.githubState}
        saving={props.saving ?? false}
      />
    );
  }
  return <GithubSectionConnected saving={props.saving ?? false} />;
}

function GithubSectionConnected({ saving }: { saving: boolean }) {
  const state = useGithub();
  return <GithubSectionView state={state} saving={saving} />;
}

function GithubSectionView({
  state,
  saving,
}: {
  state: UseGithubReturn;
  saving: boolean;
}) {
  const { message } = AntdApp.useApp();
  const { status, loading, error, saving: savingToken, saveToken, check, checking } =
    state;
  const [token, setToken] = useState("");
  const [checkResult, setCheckResult] = useState<GithubCheckResult | null>(null);

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
      message.warning("Escribe el token de GitHub antes de guardar");
      return;
    }
    try {
      await saveToken(trimmed);
      // El token guardado no se relee: el campo se limpia tras guardar.
      if (mountedRef.current) setToken("");
      message.success("Token de GitHub guardado");
    } catch {
      message.error("Error al guardar el token de GitHub");
    }
  };

  /**
   * «Probar conexión» sondea la API remota (`GET /api/github/check`) y publica
   * el resultado en la línea de estado. La ruta responde siempre `200`, así que
   * el diagnóstico viaja en `detail`; un rechazo solo ocurre si la propia
   * petición HTTP falla.
   */
  const handleCheck = async () => {
    setCheckResult(null);
    try {
      const result = await check();
      if (mountedRef.current) setCheckResult(result);
      if (result.ok) {
        message.success("Conexión con GitHub correcta");
      } else {
        message.error("La comprobación con GitHub ha fallado");
      }
    } catch {
      if (mountedRef.current) {
        setCheckResult({
          ok: false,
          detail: "No se pudo comprobar la conexión con GitHub",
        });
      }
      message.error("No se pudo comprobar la conexión con GitHub");
    }
  };

  const renderStatus = () => {
    if (loading && !status) {
      return (
        <div role="status" aria-label="Cargando estado de GitHub">
          <Spin />
        </div>
      );
    }
    if (status) {
      return (
        <>
          <Text type="secondary">
            {status.configured ? "Configurado" : "Sin configurar"}
          </Text>
          {status.default_repo && (
            <Text type="secondary">{`Repositorio: ${status.default_repo}`}</Text>
          )}
        </>
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
    <section aria-label="GitHub">
      <Title level={5} style={{ marginTop: 0 }}>
        GitHub
      </Title>
      <Text type="secondary">
        Repositorio por defecto, labels por defecto y token de un Personal Access
        Token clásico para crear issues por conversación. Por seguridad, el token
        guardado nunca se muestra: para cambiarlo, escribe uno nuevo y guarda.
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

        {/* El repositorio y los labels son ajustes normales del formulario
            compartido: viajan en el `PUT /api/settings` del botón «Guardar». */}
        <Form.Item
          label="GitHub · Repositorio por defecto"
          name="github_default_repo"
        >
          <Input placeholder="atareao/valet-ai" />
        </Form.Item>

        <Form.Item
          label="GitHub · Labels por defecto"
          name="github_default_labels"
        >
          <Input placeholder="feature, valet" />
        </Form.Item>

        {/* El token es sensible y se guarda por su endpoint dedicado. */}
        <Space.Compact style={{ width: "100%" }}>
          <Input.Password
            aria-label="GitHub · Token"
            placeholder="Escribe el token de GitHub para configurar la integración"
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
