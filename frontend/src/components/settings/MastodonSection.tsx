import { useEffect, useRef, useState } from "react";
import { App as AntdApp, Button, Form, Input, Space, Spin, Typography } from "antd";

import { useMastodon } from "../../hooks/useMastodon";
import type { UseMastodonReturn } from "../../hooks/useMastodon";
import { UNAVAILABLE_LABEL } from "../integrationStatus";
import type { MastodonCheckResult } from "../../types";

const { Text, Title } = Typography;

export interface MastodonSectionProps {
  /**
   * Estado ya cargado por el contenedor («Integraciones»). Si llega, la sección
   * **no** llama a `useMastodon()`: el indicador de la sub-pestaña y el panel
   * comparten una única lectura de `GET /api/mastodon/status`. Si falta, el
   * envoltorio la hace por su cuenta (compatibilidad con quien monte la sección
   * suelta).
   */
  mastodonState?: UseMastodonReturn;
  /**
   * `true` mientras se persisten los ajustes del formulario compartido. Se
   * reenvía al botón «Guardar» (submit de ese formulario) para impedir un doble
   * `PUT /api/settings`.
   */
  saving?: boolean;
}

/**
 * Sección «Mastodon» de la sub-pestaña de **Integraciones** del diálogo de
 * ajustes.
 *
 * Combina dos vías de persistencia bien separadas:
 *
 * - La **URL base** (`mastodon_base_url`) es un ajuste normal: se declara con
 *   `Form.Item` sobre el formulario de ajustes del padre y se guarda con el
 *   `PUT /api/settings` de su botón «Guardar». No tiene valor por defecto: se
 *   exige una URL explícita para no enviar el token a una instancia equivocada.
 * - El **token** (`mastodon_token`) es una clave sensible: se excluye de
 *   `GET /api/settings`, `PUT /api/settings` lo ignora y se guarda por su
 *   endpoint dedicado (`PUT /api/mastodon/token`). El campo **nunca** muestra el
 *   valor guardado: su estado arranca vacío y solo refleja lo que teclea el
 *   usuario. El estado de configuración sale de `GET /api/mastodon/status`
 *   (que jamás devuelve el token).
 *
 * «Probar conexión» **no** relee el estado local: sondea la API remota con
 * `GET /api/mastodon/check` (que responde siempre `200`) y muestra el `detail`
 * del resultado en la línea de estado —además del toast—, porque el resultado
 * de una acción no puede quedar solo en un aviso efímero.
 */
export function MastodonSection(props: MastodonSectionProps) {
  if (props.mastodonState) {
    return (
      <MastodonSectionView
        state={props.mastodonState}
        saving={props.saving ?? false}
      />
    );
  }
  return <MastodonSectionConnected saving={props.saving ?? false} />;
}

function MastodonSectionConnected({ saving }: { saving: boolean }) {
  const state = useMastodon();
  return <MastodonSectionView state={state} saving={saving} />;
}

function MastodonSectionView({
  state,
  saving,
}: {
  state: UseMastodonReturn;
  saving: boolean;
}) {
  const { message } = AntdApp.useApp();
  const {
    status,
    loading,
    error,
    saving: savingToken,
    saveToken,
    check,
    checking,
  } = state;
  // El token NUNCA se precarga: este estado solo refleja lo que teclea el
  // usuario, jamás el valor guardado (que el endpoint de estado no devuelve).
  const [token, setToken] = useState("");
  const [checkResult, setCheckResult] = useState<MastodonCheckResult | null>(null);

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
      message.warning("Escribe el token de Mastodon antes de guardar");
      return;
    }
    try {
      await saveToken(trimmed);
      // El token guardado no se relee: el campo se limpia tras guardar.
      if (mountedRef.current) setToken("");
      message.success("Token de Mastodon guardado");
    } catch {
      message.error("Error al guardar el token de Mastodon");
    }
  };

  const handleCheck = async () => {
    setCheckResult(null);
    try {
      const result = await check();
      if (mountedRef.current) setCheckResult(result);
      if (result.ok) {
        message.success("Conexión con Mastodon correcta");
      } else {
        message.error("La comprobación con Mastodon ha fallado");
      }
    } catch {
      if (mountedRef.current) {
        setCheckResult({
          ok: false,
          detail: "No se pudo comprobar la conexión con Mastodon",
        });
      }
      message.error("No se pudo comprobar la conexión con Mastodon");
    }
  };

  const renderStatus = () => {
    if (loading && !status) {
      return (
        <div role="status" aria-label="Cargando estado de Mastodon">
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
          {status.base_url && (
            <Text type="secondary">{`Instancia: ${status.base_url}`}</Text>
          )}
        </>
      );
    }
    if (error) {
      // La etiqueta de la sub-pestaña usa la misma etiqueta compartida cuando
      // la lectura falla: el panel no puede decir «Sin configurar» en su lugar.
      return <Text type="warning">{UNAVAILABLE_LABEL}</Text>;
    }
    return <Text type="secondary">{UNAVAILABLE_LABEL}</Text>;
  };

  return (
    <section aria-label="Mastodon">
      <Title level={5} style={{ marginTop: 0 }}>
        Mastodon
      </Title>
      <Text type="secondary">
        URL de tu instancia y token de una app OAuth con scopes
        <code> read:notifications</code> y <code>write:statuses</code> para leer
        menciones y contestarlas desde el chat. Por seguridad, el token guardado
        nunca se muestra: para cambiarlo, escribe uno nuevo y guarda.
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

        {/* La URL base es un ajuste normal: viaja en el `PUT /api/settings` del
            botón «Guardar». No hay valor por defecto a propósito. */}
        <Form.Item
          label="Mastodon · URL base"
          name="mastodon_base_url"
        >
          <Input placeholder="https://mastodon.social" />
        </Form.Item>

        {/* El token es sensible y se guarda por su endpoint dedicado. */}
        <Space.Compact style={{ width: "100%" }}>
          <Input.Password
            aria-label="Mastodon · Token"
            placeholder="Escribe el token de Mastodon para configurar la integración"
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
