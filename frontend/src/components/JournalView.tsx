import React, { useState } from "react";
import {
  Alert,
  Button,
  Input,
  List,
  Popconfirm,
  Radio,
  Space,
  Spin,
  Tag,
  Typography,
} from "antd";
import type { JournalEntry, JournalEntryInput, Mood } from "../types";
import { useJournal } from "../hooks/useJournal";

const { Text } = Typography;
const { TextArea } = Input;

/**
 * Ánimo de cada valor del conjunto cerrado. El emoji y el texto van siempre
 * juntos: el color del `Tag` no puede ser la única señal.
 */
const MOODS: Record<Mood, { emoji: string; label: string; color: string }> = {
  great: { emoji: "😄", label: "Muy bien", color: "green" },
  good: { emoji: "🙂", label: "Bien", color: "blue" },
  neutral: { emoji: "😐", label: "Normal", color: "default" },
  bad: { emoji: "🙁", label: "Mal", color: "orange" },
  terrible: { emoji: "😣", label: "Muy mal", color: "red" },
};

/** Orden de los ánimos en el formulario: de mejor a peor. */
const MOOD_ORDER: Mood[] = ["great", "good", "neutral", "bad", "terrible"];

/** Valor del formulario para «sin ánimo»: `Radio.Group` necesita un valor. */
const NO_MOOD = "none";

const MONTHS = [
  "enero",
  "febrero",
  "marzo",
  "abril",
  "mayo",
  "junio",
  "julio",
  "agosto",
  "septiembre",
  "octubre",
  "noviembre",
  "diciembre",
];

/**
 * Fecha visible a partir del `YYYY-MM-DD` que ya entrega el backend: la cadena
 * se **parte**, nunca se pasa por `new Date()` —en una zona al oeste de UTC eso
 * movería la entrada al día anterior—.
 */
const formatDate = (date: string): string => {
  const [year, month, day] = date.split("-");
  const monthName = MONTHS[Number(month) - 1];
  if (!year || !day || !monthName) return date;
  return `${Number(day)} de ${monthName} de ${year}`;
};

/** Etiquetas de texto separadas por comas; los huecos se descartan. */
const parseTags = (raw: string): string[] =>
  raw
    .split(",")
    .map((tag) => tag.trim())
    .filter((tag) => tag.length > 0);

const errorMessage = (err: unknown): string =>
  err instanceof Error ? err.message : String(err);

const MoodTag: React.FC<{ mood: Mood | null }> = ({ mood }) => {
  if (mood === null) {
    return <Text type="secondary">Sin ánimo</Text>;
  }
  const { emoji, label, color } = MOODS[mood];
  return (
    <Tag color={color} aria-label={`Ánimo: ${label}`}>
      {emoji} {label}
    </Tag>
  );
};

const EntrySummary: React.FC<{ entry: JournalEntry }> = ({ entry }) => (
  <Space direction="vertical" size={4} style={{ width: "100%" }}>
    <Space align="center" wrap>
      <Text strong>
        <time dateTime={entry.date}>{formatDate(entry.date)}</time>
      </Text>
      <MoodTag mood={entry.mood} />
    </Space>

    <Text>{entry.content}</Text>

    {entry.tags.length > 0 && (
      <Space wrap>
        <Text type="secondary">Etiquetas:</Text>
        {entry.tags.map((tag) => (
          <Tag key={tag}>{tag}</Tag>
        ))}
      </Space>
    )}

    {entry.achievements.length > 0 && (
      <Space wrap>
        <Text type="secondary">Logros:</Text>
        {entry.achievements.map((achievement) => (
          <Text key={achievement}>{achievement}</Text>
        ))}
      </Space>
    )}

    {entry.struggles.length > 0 && (
      <Space wrap>
        <Text type="secondary">Dificultades:</Text>
        {entry.struggles.map((struggle) => (
          <Text key={struggle}>{struggle}</Text>
        ))}
      </Space>
    )}
  </Space>
);

interface EntryEditorProps {
  entry: JournalEntry;
  saving: boolean;
  onSave: (body: JournalEntryInput) => void;
  onCancel: () => void;
}

/**
 * Edición en línea de una entrada: solo el ánimo, el relato y las etiquetas. El
 * formulario vive dentro de la entrada a la que pertenece, así que su borrador
 * no se mezcla con el de otra.
 */
const EntryEditor: React.FC<EntryEditorProps> = ({
  entry,
  saving,
  onSave,
  onCancel,
}) => {
  const [mood, setMood] = useState<Mood | null>(entry.mood);
  const [content, setContent] = useState(entry.content);
  const [tags, setTags] = useState(entry.tags.join(", "));

  const contentId = `journal-content-${entry.date}`;
  const tagsId = `journal-tags-${entry.date}`;

  return (
    <Space direction="vertical" size="small" style={{ width: "100%" }}>
      <Text strong>
        <time dateTime={entry.date}>{formatDate(entry.date)}</time>
      </Text>

      <fieldset style={{ border: 0, margin: 0, padding: 0 }}>
        <legend style={{ padding: 0 }}>
          <Text type="secondary">Ánimo</Text>
        </legend>
        {/* El nombre accesible del grupo vive en el `legend`: `Radio.Group` de
            antd solo reenvía atributos `aria-*` al `<div>` que envuelve los
            radios (no tiene rol), así que un `aria-label` ahí no agruparía
            nada. El `fieldset` sí anuncia grupo y nombre al lector. */}
        <Radio.Group
          value={mood ?? NO_MOOD}
          onChange={(event) => {
            const value: string = event.target.value;
            setMood(value === NO_MOOD ? null : (value as Mood));
          }}
          options={[
            ...MOOD_ORDER.map((value) => ({
              label: MOODS[value].label,
              value,
            })),
            { label: "Sin ánimo", value: NO_MOOD },
          ]}
        />
      </fieldset>

      <div>
        <label htmlFor={contentId}>
          <Text type="secondary">Relato</Text>
        </label>
        <TextArea
          id={contentId}
          rows={3}
          value={content}
          onChange={(event) => setContent(event.target.value)}
        />
      </div>

      <div>
        <label htmlFor={tagsId}>
          <Text type="secondary">Etiquetas</Text>
        </label>
        <Input
          id={tagsId}
          value={tags}
          placeholder="trabajo, casa"
          onChange={(event) => setTags(event.target.value)}
        />
      </div>

      <Space>
        <Button
          type="primary"
          loading={saving}
          onClick={() => {
            onSave({
              content,
              mood,
              tags: parseTags(tags),
              // `PUT` reemplaza la entrada entera: los logros y las dificultades
              // no se editan aquí, así que viajan tal cual para no perderlos.
              achievements: entry.achievements,
              struggles: entry.struggles,
            });
          }}
        >
          Guardar
        </Button>
        <Button onClick={onCancel} disabled={saving}>
          Cancelar
        </Button>
      </Space>
    </Space>
  );
};

/** Bitácora: el listado de las entradas, editable y borrable. */
export const JournalView: React.FC = () => {
  const { entries, loading, error, save, remove } = useJournal();
  const [editingDate, setEditingDate] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  // Los fallos de edición y borrado no son fallos de carga: se avisan sin
  // llevarse por delante la lista.
  const [actionError, setActionError] = useState<string | null>(null);

  const handleSave = async (entry: JournalEntry, body: JournalEntryInput) => {
    setSaving(true);
    setActionError(null);
    try {
      await save(entry.date, body);
      setEditingDate(null);
    } catch (err: unknown) {
      // El formulario sigue abierto: lo escrito está ahí para reintentar.
      setActionError(errorMessage(err));
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async (entry: JournalEntry) => {
    setActionError(null);
    try {
      await remove(entry.date);
    } catch (err: unknown) {
      setActionError(errorMessage(err));
    }
  };

  if (loading) {
    return (
      <div
        role="status"
        aria-live="polite"
        style={{ textAlign: "center", padding: "24px 0" }}
      >
        <Spin /> <Text>Cargando la bitácora…</Text>
      </div>
    );
  }

  if (error) {
    return (
      <Alert
        type="error"
        showIcon
        message="No se pudo cargar la bitácora"
        description={error}
      />
    );
  }

  return (
    <Space direction="vertical" size="middle" style={{ width: "100%" }}>
      {actionError && <Alert type="error" showIcon message={actionError} />}

      {entries.length === 0 ? (
        <div
          role="status"
          style={{
            textAlign: "center",
            padding: "24px 0",
            color: "rgba(255,255,255,0.45)",
          }}
        >
          Todavía no hay entradas. Aparecerán a medida que el agente vaya
          apuntando tus días.
        </div>
      ) : (
        <List
          dataSource={entries}
          rowKey={(entry) => entry.id}
          renderItem={(entry) => (
            <List.Item
              actions={
                editingDate === entry.date
                  ? []
                  : [
                      <Button
                        key="edit"
                        size="small"
                        onClick={() => {
                          setActionError(null);
                          setEditingDate(entry.date);
                        }}
                      >
                        Editar
                      </Button>,
                      <Popconfirm
                        key="delete"
                        title={`¿Borrar la entrada del ${formatDate(entry.date)}?`}
                        description="Esta acción no se puede deshacer."
                        okText="Sí"
                        cancelText="No"
                        onConfirm={() => {
                          void handleDelete(entry);
                        }}
                      >
                        <Button size="small" danger>
                          Borrar
                        </Button>
                      </Popconfirm>,
                    ]
              }
            >
              {editingDate === entry.date ? (
                <EntryEditor
                  entry={entry}
                  saving={saving}
                  onSave={(body) => {
                    void handleSave(entry, body);
                  }}
                  onCancel={() => setEditingDate(null)}
                />
              ) : (
                <EntrySummary entry={entry} />
              )}
            </List.Item>
          )}
        />
      )}
    </Space>
  );
};
