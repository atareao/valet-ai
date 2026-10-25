import React, { useState } from "react";
import {
  Alert,
  Button,
  Input,
  List,
  Popconfirm,
  Select,
  Space,
  Spin,
  Tag,
  Typography,
} from "antd";
import type { Note, NoteCategory, NoteInput } from "../types";
import { useNotes } from "../hooks/useNotes";

const { Text } = Typography;
const { TextArea } = Input;

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
 * Fecha visible a partir del crudo de SQLite (`"YYYY-MM-DD HH:MM:SS"`): la parte
 * de la fecha se **parte**, nunca se pasa por `new Date()` —en una zona al oeste
 * de UTC eso movería la nota al día anterior—.
 */
const formatDate = (value: string): string => {
  const [datePart] = value.split(" ");
  const [year, month, day] = (datePart ?? "").split("-");
  const monthName = MONTHS[Number(month) - 1];
  if (!year || !day || !monthName) return value;
  return `${Number(day)} de ${monthName} de ${year}`;
};

/** Etiquetas de texto separadas por comas; los huecos se descartan. */
const parseTags = (raw: string): string[] =>
  raw
    .split(",")
    .map((tag) => tag.trim())
    .filter((tag) => tag.length > 0);

/** Las tres categorías permitidas, con su etiqueta legible. */
const CATEGORY_LABELS: Record<NoteCategory, string> = {
  idea: "Idea",
  journal: "Diario",
  fact: "Dato",
};

const CATEGORY_COLORS: Record<NoteCategory, string> = {
  idea: "blue",
  journal: "green",
  fact: "gold",
};

const CATEGORY_OPTIONS: { label: string; value: NoteCategory }[] = (
  Object.keys(CATEGORY_LABELS) as NoteCategory[]
).map((value) => ({ label: CATEGORY_LABELS[value], value }));

/** ¿Es una de las tres categorías válidas? El dato legado `todo` no lo es. */
const isNoteCategory = (value: string): value is NoteCategory =>
  Object.prototype.hasOwnProperty.call(CATEGORY_LABELS, value);

/** Etiqueta visible de la categoría; un valor legado se muestra tal cual. */
const categoryLabel = (category: string): string =>
  isNoteCategory(category) ? CATEGORY_LABELS[category] : category;

const categoryColor = (category: string): string =>
  isNoteCategory(category) ? CATEGORY_COLORS[category] : "default";

const errorMessage = (err: unknown): string =>
  err instanceof Error ? err.message : String(err);

const NoteSummary: React.FC<{ note: Note }> = ({ note }) => {
  const tags = parseTags(note.tags ?? "");
  return (
    <Space direction="vertical" size={4} style={{ width: "100%" }}>
      <Space align="center" wrap>
        <Tag color={categoryColor(note.category)}>
          {categoryLabel(note.category)}
        </Tag>
        <Text type="secondary">
          <time dateTime={note.created_at}>
            {formatDate(note.created_at)}
          </time>
        </Text>
      </Space>

      <Text>{note.content}</Text>

      {tags.length > 0 && (
        <Space wrap>
          {tags.map((tag, index) => (
            <Tag key={`${tag}-${index}`}>{tag}</Tag>
          ))}
        </Space>
      )}
    </Space>
  );
};

interface NoteEditorProps {
  note: Note;
  saving: boolean;
  onSave: (body: NoteInput) => void;
  onCancel: () => void;
}

/**
 * Edición en línea de una nota: contenido, categoría y etiquetas. El formulario
 * vive dentro de la nota a la que pertenece, así que su borrador no se mezcla
 * con el de otra. Una categoría legada (`todo`) no es una opción válida, así que
 * el desplegable arranca vacío y guardar exige elegir una de las tres.
 */
const NoteEditor: React.FC<NoteEditorProps> = ({
  note,
  saving,
  onSave,
  onCancel,
}) => {
  const [content, setContent] = useState(note.content);
  const [category, setCategory] = useState<NoteCategory | null>(
    isNoteCategory(note.category) ? note.category : null,
  );
  const [tags, setTags] = useState(note.tags ?? "");

  const contentId = `notes-content-${note.id}`;
  const categoryId = `notes-category-${note.id}`;
  const tagsId = `notes-tags-${note.id}`;

  const canSave = content.trim().length > 0 && category !== null;

  return (
    <Space direction="vertical" size="small" style={{ width: "100%" }}>
      <Text type="secondary">
        <time dateTime={note.created_at}>
          {formatDate(note.created_at)}
        </time>
      </Text>

      <div>
        <label htmlFor={contentId}>
          <Text type="secondary">Contenido</Text>
        </label>
        <TextArea
          id={contentId}
          rows={3}
          value={content}
          onChange={(event) => setContent(event.target.value)}
        />
      </div>

      <div>
        <label htmlFor={categoryId}>
          <Text type="secondary">Categoría</Text>
        </label>
        {!isNoteCategory(note.category) && (
          // El valor previo era legado (`todo`): no es una opción válida, así que
          // se anuncia junto al desplegable para que el cambio no parezca casual.
          <Tag style={{ marginLeft: 8 }}>{`${note.category} (legado)`}</Tag>
        )}
        <Select<NoteCategory>
          id={categoryId}
          style={{ width: "100%" }}
          value={category ?? undefined}
          placeholder="Elige una categoría"
          options={CATEGORY_OPTIONS}
          onChange={(value) => setCategory(value)}
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
          disabled={!canSave}
          onClick={() => {
            if (category === null) return;
            onSave({
              content,
              category,
              // En la edición `tags` viaja **siempre** (incluida la cadena
              // vacía): `""` es un valor, no una ausencia, y es lo que vacía las
              // etiquetas en la API. Mandar `undefined` las dejaría intactas.
              tags: tags.trim(),
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

/** Notas: el listado del perfil, con alta, edición en línea y borrado. */
export const NotesView: React.FC = () => {
  const { notes, loading, error, create, update, remove } = useNotes();

  // Borrador del formulario de alta.
  const [draftContent, setDraftContent] = useState("");
  const [draftCategory, setDraftCategory] = useState<NoteCategory>("idea");
  const [draftTags, setDraftTags] = useState("");

  const [editingId, setEditingId] = useState<string | null>(null);
  const [creating, setCreating] = useState(false);
  const [saving, setSaving] = useState(false);
  // Los fallos de alta, edición y borrado no son fallos de carga: se avisan sin
  // llevarse por delante la lista.
  const [actionError, setActionError] = useState<string | null>(null);

  const handleCreate = async () => {
    setCreating(true);
    setActionError(null);
    try {
      await create({
        content: draftContent,
        category: draftCategory,
        tags: draftTags.trim() || undefined,
      });
      // El borrador se limpia: lo creado ya vive en la lista.
      setDraftContent("");
      setDraftCategory("idea");
      setDraftTags("");
    } catch (err: unknown) {
      setActionError(errorMessage(err));
    } finally {
      setCreating(false);
    }
  };

  const handleSave = async (note: Note, body: NoteInput) => {
    setSaving(true);
    setActionError(null);
    try {
      await update(note.id, body);
      setEditingId(null);
    } catch (err: unknown) {
      // El formulario sigue abierto: lo escrito está ahí para reintentar.
      setActionError(errorMessage(err));
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async (note: Note) => {
    setActionError(null);
    try {
      await remove(note.id);
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
        <Spin /> <Text>Cargando las notas…</Text>
      </div>
    );
  }

  if (error) {
    return (
      <Alert
        type="error"
        showIcon
        message="No se pudieron cargar las notas"
        description={error}
      />
    );
  }

  return (
    <Space direction="vertical" size="middle" style={{ width: "100%" }}>
      {actionError && <Alert type="error" showIcon message={actionError} />}

      <Space direction="vertical" size="small" style={{ width: "100%" }}>
        <div>
          <label htmlFor="notes-new-content">
            <Text type="secondary">Contenido</Text>
          </label>
          <TextArea
            id="notes-new-content"
            rows={2}
            value={draftContent}
            placeholder="Escribe una nota…"
            onChange={(event) => setDraftContent(event.target.value)}
          />
        </div>

        <div>
          <label htmlFor="notes-new-category">
            <Text type="secondary">Categoría</Text>
          </label>
          <Select<NoteCategory>
            id="notes-new-category"
            style={{ width: "100%" }}
            value={draftCategory}
            options={CATEGORY_OPTIONS}
            onChange={(value) => setDraftCategory(value)}
          />
        </div>

        <div>
          <label htmlFor="notes-new-tags">
            <Text type="secondary">Etiquetas</Text>
          </label>
          <Input
            id="notes-new-tags"
            value={draftTags}
            placeholder="trabajo, casa"
            onChange={(event) => setDraftTags(event.target.value)}
          />
        </div>

        <Button
          type="primary"
          loading={creating}
          disabled={draftContent.trim().length === 0}
          onClick={() => {
            void handleCreate();
          }}
        >
          Crear
        </Button>
      </Space>

      {notes.length === 0 ? (
        <div
          role="status"
          style={{
            textAlign: "center",
            padding: "24px 0",
            color: "rgba(255,255,255,0.45)",
          }}
        >
          Todavía no hay notas. Crea una desde el formulario de arriba.
        </div>
      ) : (
        <List
          dataSource={notes}
          rowKey={(note) => note.id}
          renderItem={(note) => (
            <List.Item
              actions={
                editingId === note.id
                  ? []
                  : [
                      <Button
                        key="edit"
                        size="small"
                        onClick={() => {
                          setActionError(null);
                          setEditingId(note.id);
                        }}
                      >
                        Editar
                      </Button>,
                      <Popconfirm
                        key="delete"
                        title="¿Borrar esta nota?"
                        description="Esta acción no se puede deshacer."
                        okText="Sí"
                        cancelText="No"
                        onConfirm={() => {
                          void handleDelete(note);
                        }}
                      >
                        <Button size="small" danger>
                          Borrar
                        </Button>
                      </Popconfirm>,
                    ]
              }
            >
              {editingId === note.id ? (
                <NoteEditor
                  note={note}
                  saving={saving}
                  onSave={(body) => {
                    void handleSave(note, body);
                  }}
                  onCancel={() => setEditingId(null)}
                />
              ) : (
                <NoteSummary note={note} />
              )}
            </List.Item>
          )}
        />
      )}
    </Space>
  );
};
