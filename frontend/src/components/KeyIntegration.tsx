import { Button, Form, Input, Typography } from "antd";

import type { SettingsMap } from "../types";
import type { KeyIntegrationDefinition } from "./keyIntegrations";
import { keyStatusLabel } from "./integrationStatus";

const { Text, Title } = Typography;

export interface KeyIntegrationProps {
  /** Definición declarativa de la integración a pintar. */
  definition: KeyIntegrationDefinition;
  /** Ajustes vigentes; de aquí sale el estado del indicador. */
  settings: SettingsMap;
  /** `true` mientras se persisten los ajustes; para el botón. */
  saving?: boolean;
}

/**
 * Panel de una integración basada en claves (sub-pestaña de «Integraciones»).
 *
 * El `Form` es el del padre (formulario de ajustes compartido), así que aquí
 * solo van los `Form.Item` y el botón de guardado, que dispara el submit de ese
 * formulario: cada sub-pestaña conserva su propio botón «Guardar».
 *
 * Toda la variación entre las cinco integraciones vive en la definición
 * (la tabla `KEY_INTEGRATIONS` de `./keyIntegrations`), no en el JSX: así no
 * pueden divergir sus etiquetas, placeholders ni claves de ajuste.
 */
export function KeyIntegration({
  definition,
  settings,
  saving = false,
}: KeyIntegrationProps) {
  const status = keyStatusLabel(settings, definition.statusKey);

  return (
    <section aria-label={definition.name}>
      <Title level={5} style={{ marginTop: 0 }}>
        {definition.name}
      </Title>
      <Text type="secondary">{definition.description}</Text>

      <div aria-live="polite" style={{ marginTop: 12, marginBottom: 12 }}>
        <Text>{`Estado: ${status}`}</Text>
      </div>

      {definition.fields.map((field) => (
        <Form.Item key={field.name} label={field.label} name={field.name}>
          {field.password ? (
            <Input.Password placeholder={field.placeholder} />
          ) : (
            <Input placeholder={field.placeholder} />
          )}
        </Form.Item>
      ))}

      <Button type="primary" htmlType="submit" loading={saving}>
        Guardar
      </Button>
    </section>
  );
}
