import { useEffect, useRef, useState } from "react";
import {
  Alert,
  App as AntdApp,
  Button,
  Form,
  Input,
  InputNumber,
  Space,
  Spin,
  Switch,
  Tabs,
  Tag,
} from "antd";
import type { SkillInfo, SkillsResponse } from "../types";import {
  changedSkillFields,
  isSkillFieldOverridden,
  skillEffectiveValues,
  skillEnabledKey,
  skillFields,
  skillThresholdKey,
} from "./skillRouter";

const { TextArea } = Input;

export interface SkillsTabProps {
  /** Settings vigentes; de aquí sale el valor efectivo del fragmento. */
  settings: Record<string, string> | null;
  /** Catálogo de skills (`GET /api/skills`); una pestaña por skill. */
  skills: SkillInfo[];
  /** Estado de carga del catálogo. */
  loading: boolean;
  /** Error de la consulta del catálogo (si lo hubo). */
  error: string | null;
  /** Persiste cambios y refresca los settings vigentes (hook `useSettings`). */
  updateSettings: (data: Record<string, string>) => Promise<void>;
  /** Relee el catálogo (`GET /api/skills`); lo disparan guardar y restaurar. */
  onRestore: () => Promise<SkillsResponse>;
}

interface SkillPaneProps {
  skill: SkillInfo;
  /** Settings vigentes; deciden la marca de sobrescritura del fragmento. */
  settings: Record<string, string> | null;
  /** Clave de settings cuyo «Restaurar» está en curso (para el spinner). */
  restoringKey: string | null;
  onRestoreField: (key: string) => Promise<void>;
}

/**
 * Contenido de la pestaña de una skill: su interruptor de habilitación, su
 * umbral, el aviso (no bloqueante) si el umbral toca los extremos, y sus cuatro
 * campos editables. Los campos comparten el `Form` de `SkillsTab`.
 */
function SkillPane({
  skill,
  settings,
  restoringKey,
  onRestoreField,
}: SkillPaneProps) {
  const form = Form.useFormInstance();
  const threshold = Form.useWatch<number | null>(
    skillThresholdKey(skill.id),
    form,
  );
  const extremeThreshold = threshold === 0 || threshold === 1;

  return (
    <div>
      <Form.Item
        label="Habilitada"
        name={skillEnabledKey(skill.id)}
        valuePropName="checked"
      >
        <Switch aria-label={`Habilitada ${skill.id}`} />
      </Form.Item>

      <Form.Item
        label={`Umbral de ${skill.id}`}
        name={skillThresholdKey(skill.id)}
      >
        <InputNumber min={0} max={1} step={0.05} style={{ width: "100%" }} />
      </Form.Item>
      {extremeThreshold && (
        <Alert
          type="warning"
          showIcon
          style={{ marginBottom: 16 }}
          message={
            threshold === 0
              ? "Un umbral de 0 desactiva el filtrado de esta skill. El guardado sigue disponible."
              : "Un umbral de 1 vuelve inalcanzable cualquier selección de esta skill. El guardado sigue disponible."
          }
        />
      )}

      {skillFields(skill).map((field) => {
        const overridden = isSkillFieldOverridden(skill, field.kind, settings);
        return (
          <Form.Item
            key={field.key}
            label={`${field.label} (${skill.id})`}
            name={field.key}
            extra={
              overridden ? (
                <Space size={4}>
                  <Tag color="blue">Modificado</Tag>
                  <Button
                    type="link"
                    size="small"
                    loading={restoringKey === field.key}
                    aria-label={`Restaurar ${skill.id} · ${field.label}`}
                    onClick={() => void onRestoreField(field.key)}
                  >
                    Restaurar
                  </Button>
                </Space>
              ) : undefined
            }
          >
            <TextArea rows={field.kind === "prompt" ? 8 : 3} />
          </Form.Item>
        );
      })}
    </div>
  );
}

/**
 * Pestaña superior «Skills»: lista las skills **desde el catálogo** —una pestaña
 * por skill, no por patrón de clave— y permite configurar cada una por separado:
 * habilitación, umbral y los cuatro textos editables (pregunta, criterios y
 * fragmento), mostrando el valor efectivo vigente.
 *
 * Los campos sobrescritos se marcan y ofrecen restaurar, que vacía su clave en
 * settings y relee el catálogo. Un único «Guardar» envía **solo** las claves que
 * cambian respecto al efectivo (incluidas la habilitación y el umbral), sin
 * tocar el resto de settings. Si el catálogo falla, degrada a un aviso sin
 * romper el resto del formulario.
 */
export function SkillsTab({
  settings,
  skills,
  loading,
  error,
  updateSettings,
  onRestore,
}: SkillsTabProps) {
  const { message } = AntdApp.useApp();
  const [form] = Form.useForm();
  const seededRef = useRef(false);
  const [saving, setSaving] = useState(false);
  const [restoringKey, setRestoringKey] = useState<string | null>(null);

  // Precarga cada campo con su valor efectivo (incluidas habilitación y umbral).
  // Solo una vez por montaje: tras guardar o restaurar, una relectura posterior
  // del catálogo vuelve a sembrar (el valor persistido), sin pisar el borrador
  // mientras este sigue vivo. No se siembra hasta que settings estén cargados
  // (`null`): el fragmento depende de ellos.
  useEffect(() => {
    if (seededRef.current || skills.length === 0 || settings === null) return;
    seededRef.current = true;
    const values: Record<string, unknown> = {};
    for (const skill of skills) {
      Object.assign(values, skillEffectiveValues(skill, settings));
      values[skillEnabledKey(skill.id)] = skill.enabled ?? true;
      values[skillThresholdKey(skill.id)] = skill.threshold;
    }
    form.setFieldsValue(values);
  }, [skills, settings, form]);

  const handleRestore = async (key: string) => {
    setRestoringKey(key);
    try {
      // No hay endpoint de borrado: vaciar la clave equivale a restaurarla.
      // `updateSettings` refresca los settings; `onRestore` relee el catálogo.
      await updateSettings({ [key]: "" });
      // Fuerza el re-sembrado desde los settings y el catálogo ya frescos; el
      // efecto toma el valor restaurado (sin `setFieldsValue` manual, que
      // dependería de un `settings` obsoleto).
      seededRef.current = false;
      await onRestore();
      message.success("Valor por defecto restaurado");
    } catch {
      message.error("Error al restaurar el valor");
    } finally {
      setRestoringKey(null);
    }
  };

  const handleSubmit = async (values: Record<string, unknown>) => {
    // Solo lo que difiere del efectivo: ni sobrescrituras redundantes ni el
    // resto de settings del diálogo.
    const changes = changedSkillFields(values, skills, settings);
    if (Object.keys(changes).length === 0) {
      message.info("No hay cambios que guardar");
      return;
    }
    if (Object.values(changes).some((value) => value === "")) {
      message.warning(
        "Hay campos vacíos: se usará el valor por defecto del catálogo",
      );
    }
    setSaving(true);
    try {
      await updateSettings(changes);
      message.success("Ajustes de skills guardados");
      // Relee el catálogo para refrescar valores efectivos y marcas de override.
      // El fallo de la relectura no revierte el guardado ya confirmado.
      seededRef.current = false;
      await onRestore().catch(() => undefined);
    } catch {
      message.error("Error al guardar los ajustes de skills");
    } finally {
      setSaving(false);
    }
  };

  if (loading) {
    return <Spin style={{ display: "block", margin: "12px 0" }} />;
  }

  if (error || skills.length === 0) {
    return (
      <Alert
        type="warning"
        showIcon
        message="No se pudieron cargar las skills"
        description="La configuración por skill no está disponible. El resto de los ajustes sigue funcionando."
      />
    );
  }

  return (
    <section role="region" aria-label="Skills">
      <Form form={form} layout="vertical" onFinish={handleSubmit}>
        <Tabs
          aria-label="Configuración por skill"
          items={skills.map((skill) => ({
            key: skill.id,
            label: skill.id,
            forceRender: true,
            children: (
              <SkillPane
                skill={skill}
                settings={settings}
                restoringKey={restoringKey}
                onRestoreField={handleRestore}
              />
            ),
          }))}
        />
        <Button
          type="primary"
          htmlType="submit"
          loading={saving}
          style={{ marginTop: 16 }}
        >
          Guardar
        </Button>
      </Form>
    </section>
  );
}
