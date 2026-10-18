import { useCallback, useEffect, useRef, useState } from "react";
import {
  Alert,
  App as AntdApp,
  Button,
  Input,
  Popconfirm,
  Space,
  Spin,
  Table,
  Tag,
  Typography,
} from "antd";
import type { TableProps } from "antd";
import {
  createIngestKey,
  listIngestKeys,
  revokeIngestKey,
} from "../api/ingestKeys";
import type { CreatedIngestKey, IngestKey } from "../types";
import { parseSqliteTimestamp } from "../utils/sqliteTimestamp";

const { Text, Title } = Typography;

/**
 * Formatea una fecha (ISO-8601 o formato crudo de SQLite `"YYYY-MM-DD HH:MM:SS"`)
 * a texto local en hora del usuario. Los valores ausentes o no parseables se
 * muestran como guion largo (`—`).
 */
function formatDate(value: string | null): string {
  const date = parseSqliteTimestamp(value);
  return date ? date.toLocaleString() : "—";
}

/**
 * Sección «Claves de ingesta» de la pestaña **API Keys** del diálogo de ajustes.
 *
 * Es independiente del formulario de ajustes (no usa `updateSettings`): las
 * claves se crean y se revocan con llamadas **inmediatas** a la API. El token
 * solo se muestra una vez, en el alta, junto a un aviso explícito; la lista de
 * claves nunca lo contiene, solo el prefijo. Todos los avisos se emiten por la
 * API contextual de antd (`App.useApp()`).
 */
export function IngestKeysSection() {
  const { message } = AntdApp.useApp();
  const [keys, setKeys] = useState<IngestKey[]>([]);
  const [loading, setLoading] = useState(true);
  const [name, setName] = useState("");
  const [creating, setCreating] = useState(false);
  const [revokingId, setRevokingId] = useState<string | null>(null);
  // El token recién creado vive solo aquí, unos segundos, hasta la siguiente
  // creación: no se guarda en la lista ni se relee del servidor.
  const [created, setCreated] = useState<CreatedIngestKey | null>(null);

  const mountedRef = useRef(true);
  useEffect(() => {
    mountedRef.current = true;
    return () => {
      mountedRef.current = false;
    };
  }, []);

  const refresh = useCallback((): Promise<void> => {
    return listIngestKeys()
      .then((next) => {
        if (mountedRef.current) setKeys(next);
      })
      .catch(() => {
        if (mountedRef.current) {
          message.error("No se pudieron cargar las claves de ingesta");
        }
      })
      .finally(() => {
        if (mountedRef.current) setLoading(false);
      });
  }, [message]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const handleCreate = async () => {
    const trimmed = name.trim();
    if (!trimmed) return;
    setCreating(true);
    try {
      const result = await createIngestKey(trimmed);
      if (!mountedRef.current) return;
      setCreated(result);
      setName("");
      await refresh();
    } catch {
      // Un fallo no deja la clave a medias: no se añade a la lista ni se
      // refresca; solo se comunica el error.
      if (mountedRef.current) {
        message.error("No se pudo crear la clave de ingesta");
      }
    } finally {
      if (mountedRef.current) setCreating(false);
    }
  };

  const handleRevoke = async (id: string) => {
    setRevokingId(id);
    try {
      await revokeIngestKey(id);
      message.success("Clave de ingesta revocada");
      await refresh();
    } catch {
      message.error("No se pudo revocar la clave de ingesta");
    } finally {
      if (mountedRef.current) setRevokingId(null);
    }
  };

  const columns: TableProps<IngestKey>["columns"] = [
    { title: "Nombre", dataIndex: "name", key: "name" },
    { title: "Prefijo", dataIndex: "prefix", key: "prefix" },
    {
      title: "Alta",
      dataIndex: "created_at",
      key: "created_at",
      render: (value: string) => formatDate(value),
    },
    {
      title: "Último uso",
      dataIndex: "last_used_at",
      key: "last_used_at",
      render: (value: string | null) => formatDate(value),
    },
    {
      title: "Estado",
      key: "status",
      render: (_, key) =>
        key.revoked_at ? <Tag color="default">Revocada</Tag> : <Tag color="green">Activa</Tag>,
    },
    {
      title: "Acciones",
      key: "actions",
      render: (_, key) =>
        key.revoked_at ? null : (
          <Popconfirm
            title="¿Revocar esta clave?"
            description="La aplicación que la use dejará de poder enviar avisos."
            okText="Sí"
            cancelText="No"
            onConfirm={() => void handleRevoke(key.id)}
          >
            <Button danger size="small" loading={revokingId === key.id}>
              Revocar
            </Button>
          </Popconfirm>
        ),
    },
  ];

  return (
    <section aria-label="Claves de ingesta">
      <Title level={5} style={{ marginTop: 0 }}>
        Claves de ingesta (entrantes)
      </Title>
      <Text type="secondary">
        Autoriza a otras aplicaciones a enviar avisos a Valet. El token solo se
        muestra una vez, al crearlo.
      </Text>

      <Space direction="vertical" size="middle" style={{ width: "100%", marginTop: 16 }}>
        <Space.Compact style={{ width: "100%" }}>
          <Input
            aria-label="Nombre de la aplicación"
            placeholder="Nombre de la aplicación"
            value={name}
            maxLength={64}
            onChange={(event) => setName(event.target.value)}
            onPressEnter={() => void handleCreate()}
          />
          <Button
            type="primary"
            loading={creating}
            disabled={name.trim() === ""}
            onClick={() => void handleCreate()}
          >
            Crear clave
          </Button>
        </Space.Compact>

        {created && (
          <Alert
            type="warning"
            showIcon
            message="Guarda este token ahora: no se volverá a mostrar"
            description={
              <Space direction="vertical" size={4}>
                <Text copyable={{ text: created.token }} code>
                  {created.token}
                </Text>
                <Text type="secondary">
                  Es la única vez que verás el token. Si lo pierdes, revoca la
                  clave y crea otra.
                </Text>
              </Space>
            }
          />
        )}

        {loading ? (
          <div role="status" aria-label="Cargando claves de ingesta">
            <Spin />
          </div>
        ) : (
          <Table<IngestKey>
            rowKey="id"
            size="small"
            columns={columns}
            dataSource={keys}
            pagination={false}
            locale={{ emptyText: "Todavía no hay claves de ingesta" }}
          />
        )}
      </Space>
    </section>
  );
}
