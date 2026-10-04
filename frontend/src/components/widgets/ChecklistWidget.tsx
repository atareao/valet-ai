import { useMemo, useState, type FC } from "react";
import { Button, Checkbox, Typography } from "antd";
import type { ChecklistData, WidgetProps } from "./types";

const { Text } = Typography;

/**
 * Lista de comprobación generada a partir de `ChecklistData`. Presenta un título,
 * una casilla por ítem y un botón que emite los identificadores marcados mediante
 * `onAction("submit", { checkedIds })`.
 */
export const ChecklistWidget: FC<WidgetProps<ChecklistData>> = ({
  data,
  onAction,
  disabled,
}) => {
  const items = useMemo(
    () => (Array.isArray(data?.items) ? data.items : []),
    [data],
  );
  const [checked, setChecked] = useState<Record<string, boolean>>({});

  const handleSubmit = () => {
    const checkedIds = items
      .filter((item) => checked[item.id])
      .map((item) => item.id);
    onAction("submit", { checkedIds });
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
      <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
        {items.map((item) => (
          <Checkbox
            key={item.id}
            checked={Boolean(checked[item.id])}
            onChange={(e) =>
              setChecked((prev) => ({ ...prev, [item.id]: e.target.checked }))
            }
          >
            {item.label}
          </Checkbox>
        ))}
      </div>
      <Button
        type="primary"
        onClick={handleSubmit}
        disabled={disabled}
        style={{ marginTop: 16 }}
      >
        Enviar
      </Button>
    </div>
  );
};