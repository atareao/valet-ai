import { useEffect, useRef, useState } from "react";
import {
  App as AntdApp,
  Button,
  Form,
  Input,
  Space,
  Spin,
  Typography,
} from "antd";
import { useOximeal } from "../hooks/useOximeal";
import type { UseOximealReturn } from "../hooks/useOximeal";
import { UNAVAILABLE_LABEL } from "./integrationStatus";
import type { OximealCheckResult } from "../types";

const { Text, Title } = Typography;

export interface OximealSectionProps {
  /**
   * Estado ya cargado por el contenedor («Integraciones»). Si llega, la sección
   * **no** llama a `useOximeal()`: el indicador de la sub-pestaña y el panel
   * comparten una única lectura de `GET /api/oximeal/status`. Si falta, el
   * envoltorio la hace por su cuenta (compatibilidad con quien monte la
   * sección suelta).
   */
  oximealState?: UseOximealReturn;
  /**
   * `true` mientras se persisten los ajustes del formulario compartido. Se
   * reenvía al botón «Guardar» (submit de ese formulario) para impedir un doble
   * `PUT /api/settings`; igual que hace `OpentodoSection`.
   */
  saving?: boolean;
}

/**
 * Sección «OxiMeal» de la pestaña **Integraciones** del diálogo de ajustes.
 *
 * Combina dos vías de persistencia bien separadas:
 *
 * - La **URL base**, la **URL del emisor** y el **client id** son ajustes
 *   normales (`oximeal_base_url`, `oximeal_issuer_url`, `oximeal_client_id`):
 *   se declaran con `Form.Item` sobre el formulario de ajustes del padre y se
 *   guardan con el `PUT /api/settings` de su botón «Guardar» (submit del
 *   formulario).
 * - El **client secret** (`oximeal_client_secret`) es una clave sensible: se
 *   excluye de `GET /api/settings`, `PUT /api/settings` lo ignora y se guarda
 *   por su endpoint dedicado (`PUT /api/oximeal/secret`). El campo **nunca**
 *   muestra el valor guardado: su estado arranca vacío y solo refleja lo que
 *   teclea el usuario. El estado de configuración sale de
 *   `GET /api/oximeal/status` (que jamás devuelve el secreto).
 *
 * «Probar conexión» **no** relee el estado local: sondea la instancia remota con
 * `GET /api/oximeal/check` (que responde siempre `200`) y muestra el `detail`
 * del resultado en la línea de estado —además del toast—, porque el resultado de
 * una acción no puede quedar solo en un aviso efímero.
 *
 * Envoltorio: con `oximealState` renderiza la vista sin consultar el endpoint;
 * sin él, delega en `OximealSectionConnected`, que llama al hook.
 */
export function OximealSection(props: OximealSectionProps) {
  if (props.oximealState) {
    return (
      <OximealSectionView
        state={props.oximealState}
        saving={props.saving ?? false}
      />
    );
  }
  return <OximealSectionConnected saving={props.saving ?? false} />;
}

function OximealSectionConnected({ saving }: { saving: boolean }) {
  const state = useOximeal();
  return <OximealSectionView state={state} saving={saving} />;
}

function OximealSectionView({
  state,
  saving,
}: {
  state: UseOximealReturn;
  saving: boolean;
}) {
  const { message } = AntdApp.useApp();
  const { status, loading, error, saving: savingSecret, saveSecret, check, checking } =
    state;
  const [secret, setSecret] = useState("");
  const [checkResult, setCheckResult] = useState<OximealCheckResult | null>(null);

  // Los manejadores resuelven de forma asíncrona: solo aplican su `setState`
  // si el componente sigue montado. Mismo patrón que `mountedRef` en el hook.
  const mountedRef = useRef(true);
  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const handleSaveSecret = async () => {
    const trimmed = secret.trim();
    if (trimmed === "") {
      message.warning("Escribe el client secret de OxiMeal antes de guardar");
      return;
    }
    try {
      await saveSecret(trimmed);
      // El secreto guardado no se relee: el campo se limpia siempre tras guardar.
      if (mountedRef.current) setSecret("");
      message.success("Client secret de OxiMeal guardado");
    } catch {
      message.error("Error al guardar el client secret de OxiMeal");
    }
  };

  /**
   * «Probar conexión» sondea la instancia remota (`GET /api/oximeal/check`) y
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
        message.success("Conexión con OxiMeal correcta");
      } else {
        message.error("La comprobación con OxiMeal ha fallado");
      }
    } catch {
      if (mountedRef.current) {
        setCheckResult({
          ok: false,
          detail: "No se pudo comprobar la conexión con OxiMeal",
        });
      }
      message.error("No se pudo comprobar la conexión con OxiMeal");
    }
  };

  const renderStatus = () => {
    if (loading && !status) {
      return (
        <div role="status" aria-label="Cargando estado de OxiMeal">
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
      return <Text type="warning">{UNAVAILABLE_LABEL}</Text>;
    }
    return <Text type="secondary">{UNAVAILABLE_LABEL}</Text>;
  };

  return (
    <section aria-label="OxiMeal">
      <Title level={5} style={{ marginTop: 0 }}>
        OxiMeal
      </Title>
      <Text type="secondary">
        URL base, emisor, client id y client secret del cliente M2M de OxiMeal.
        Por seguridad, el client secret guardado nunca se muestra: para
        cambiarlo, escribe uno nuevo y guarda.
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

        {/* Los ajustes no sensibles cuelgan del formulario compartido: viajan en
            el `PUT /api/settings` del botón «Guardar». */}
        <Form.Item label="OxiMeal · URL base" name="oximeal_base_url">
          <Input placeholder="https://oximeal.territoriolinux.es" />
        </Form.Item>

        <Form.Item label="OxiMeal · URL del emisor" name="oximeal_issuer_url">
          <Input placeholder="https://auth.territoriolinux.es" />
        </Form.Item>

        <Form.Item label="OxiMeal · Client ID" name="oximeal_client_id">
          <Input placeholder="valet-agent" />
        </Form.Item>

        {/* El client secret es sensible y se guarda por su endpoint dedicado. */}
        <Space.Compact style={{ width: "100%" }}>
          <Input.Password
            aria-label="OxiMeal · client secret"
            placeholder="Escribe el client secret para configurar OxiMeal"
            autoComplete="off"
            value={secret}
            onChange={(event) => setSecret(event.target.value)}
            onPressEnter={(event) => {
              // El input cuelga del formulario de ajustes del padre: sin esto,
              // Intro dispararía además el submit implícito de ese form
              // (`PUT /api/settings` + cierre del diálogo) y el secreto se
              // guardaría por dos vías a la vez.
              event.preventDefault();
              void handleSaveSecret();
            }}
          />
          <Button
            type="primary"
            loading={savingSecret}
            aria-busy={savingSecret}
            onClick={() => void handleSaveSecret()}
          >
            Guardar secreto
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
