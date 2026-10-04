import { useId, useMemo, useState, type FC } from "react";
import { Button, Checkbox, Input, Select, Slider, Typography } from "antd";
import type { QuickFormData, QuickFormField, WidgetProps } from "./types";

const { Text } = Typography;

function initialValue(field: QuickFormField): unknown {
  if (field.type === "checkbox") return false;
  if (field.type === "slider") return field.min ?? 0;
  return "";
}

/**
 * Formulario compacto generado a partir de `QuickFormData`. Presenta un título,
 * un campo por entrada (antd) y un botón de envío que emite los valores
 * introducidos mediante `onAction("submit", valores)`.
 */
export const QuickFormWidget: FC<WidgetProps<QuickFormData>> = ({
  data,
  onAction,
  disabled,
}) => {
  const baseId = useId();
  const fields = useMemo(
    () => (Array.isArray(data?.fields) ? data.fields : []),
    [data],
  );

  const [values, setValues] = useState<Record<string, unknown>>(() =>
    Object.fromEntries(fields.map((f) => [f.name, initialValue(f)])),
  );

  const setValue = (name: string, value: unknown) =>
    setValues((prev) => ({ ...prev, [name]: value }));

  const handleSubmit = () => {
    const payload: Record<string, unknown> = {};
    for (const field of fields) {
      payload[field.name] = values[field.name];
    }
    onAction("submit", payload);
  };

  const renderField = (field: QuickFormField) => {
    const id = `${baseId}-${field.name}`;
    const value = values[field.name];

    switch (field.type) {
      case "select":
        return (
          <Select
            id={id}
            style={{ width: "100%" }}
            value={typeof value === "string" ? value : undefined}
            options={(Array.isArray(field.options) ? field.options : []).map((option) => ({
              label: option,
              value: option,
            }))}
            onChange={(next) => setValue(field.name, next)}
          />
        );
      case "checkbox":
        return (
          <Checkbox
            id={id}
            checked={Boolean(value)}
            onChange={(e) => setValue(field.name, e.target.checked)}
          >
            {field.label}
          </Checkbox>
        );
      case "slider":
        return (
          <Slider
            id={id}
            min={field.min}
            max={field.max}
            value={typeof value === "number" ? value : field.min ?? 0}
            onChange={(next) => setValue(field.name, next)}
          />
        );
      default:
        return (
          <Input
            id={id}
            value={typeof value === "string" ? value : ""}
            onChange={(e) => setValue(field.name, e.target.value)}
          />
        );
    }
  };

  return (
    <div
      style={{
        padding: 12,
        border: "1px solid rgba(255,255,255,0.12)",
        borderRadius: 8,
        background: "rgba(255,255,255,0.03)",
        maxWidth: 420,
      }}
    >
      {data?.title && (
        <Text strong style={{ display: "block", marginBottom: 12 }}>
          {data.title}
        </Text>
      )}
      <div style={{ display: "flex", flexDirection: "column", gap: 12 }}>
        {fields.map((field) => {
          const id = `${baseId}-${field.name}`;
          if (field.type === "checkbox") {
            return <div key={field.name}>{renderField(field)}</div>;
          }
          return (
            <div key={field.name}>
              <label htmlFor={id} style={{ display: "block", marginBottom: 4 }}>
                {field.label}
              </label>
              {renderField(field)}
            </div>
          );
        })}
      </div>
      <Button
        type="primary"
        onClick={handleSubmit}
        disabled={disabled}
        style={{ marginTop: 16 }}
      >
        {data?.submit_label ?? "Enviar"}
      </Button>
    </div>
  );
};