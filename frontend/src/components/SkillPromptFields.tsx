import { useEffect, useRef, useState } from "react";
import {
  Alert,
  App as AntdApp,
  Button,
  Card,
  Form,
  Input,
  Space,
  Spin,
  Tag,
} from "antd";
import { api } from "../api/client";
import type { SkillInfo, SkillsResponse } from "../types";
import {
  isSkillFieldOverridden,
  skillEffectiveValues,
  skillFields,
} from "./skillRouter";

const { TextArea } = Input;

export interface SkillPromptFieldsProps {
  /** Settings vigentes; de aquí sale el valor efectivo de cada fragmento. */
  settings: Record<string, string> | null;
  /** Catálogo de skills (`GET /api/skills`); la lista se deriva de él. */
  skills: SkillInfo[];
  /** Estado de carga del catálogo. */
  loading: boolean;
  /** Error de la consulta del catálogo (si lo hubo). */
  error: string | null;
  /** Relee el catálogo (`GET /api/skills`); lo dispara la acción de restaurar. */
  onRestore: () => Promise<SkillsResponse>;
}

/**
 * Sub-pestaña «Skills» de la pestaña «Prompts»: una tarjeta por skill —listada
 * desde el catálogo, no por patrón de clave— con cuatro campos editables
 * (pregunta, criterios y fragmento), prefijados con el valor efectivo vigente.
 *
 * Los campos viajan en el `settingsForm` compartido (el submit que ya existe), y
 * el padre guarda solo los que cambien respecto al efectivo. Un campo
 * sobrescrito se marca y ofrece restaurar, que envía su clave vacía y relee el
 * catálogo para mostrar el valor por defecto. Si el catálogo falla, la vista
 * degrada a un aviso sin romper el resto del formulario.
 */
export function SkillPromptFields({
  settings,
  skills,
  loading,
  error,
  onRestore,
}: SkillPromptFieldsProps) {
  const { message } = AntdApp.useApp();
  const form = Form.useFormInstance();
  const seededRef = useRef(false);
  const [restoringKey, setRestoringKey] = useState<string | null>(null);

  // Precarga los cuatro campos de cada skill con su valor efectivo. Solo una vez
  // por montaje: en una relectura posterior del catálogo (p. ej. tras restaurar)
  // el valor restaurado se fija aparte, sin pisar ediciones pendientes.
  useEffect(() => {
    if (seededRef.current || skills.length === 0) return;
    seededRef.current = true;
    const values: Record<string, string> = {};
    for (const skill of skills) {
      Object.assign(values, skillEffectiveValues(skill, settings));
    }
    form.setFieldsValue(values);
  }, [skills, settings, form]);

  const handleRestore = async (skill: SkillInfo, key: string) => {
    setRestoringKey(key);
    try {
      // No hay endpoint de borrado: vaciar la clave equivale a restaurarla.
      await api.updateSettings({ [key]: "" });
      const data = await onRestore();
      const updated = data.skills.find((candidate) => candidate.id === skill.id);
      if (updated) {
        const effective = skillEffectiveValues(updated, settings);
        // Precondición: la acción solo se ofrece en campos marcados como
        // sobrescritos y `isSkillFieldOverridden` excluye `"prompt"`, así que
        // este camino nunca refresca un fragmento. Si algún día se marcara el
        // fragmento, este `setFieldsValue` no bastaría: su valor efectivo se
        // recalcula de `settings`, que no se recarga aquí (solo se relee el
        // catálogo) y la UI mostraría un valor obsoleto.
        form.setFieldsValue({ [key]: effective[key] });
      }
      message.success("Valor por defecto restaurado");
    } catch {
      message.error("Error al restaurar el valor");
    } finally {
      setRestoringKey(null);
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
        description="Los campos por skill no están disponibles. El resto del formulario sigue funcionando."
      />
    );
  }

  return (
    <>
      {skills.map((skill) => (
        <div role="group" aria-label={`Skill ${skill.id}`} key={skill.id}>
          <Card size="small" title={skill.id} style={{ marginBottom: 12 }}>
            {skillFields(skill).map((field) => {
              const overridden = isSkillFieldOverridden(skill, field.kind);
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
                          onClick={() => void handleRestore(skill, field.key)}
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
          </Card>
        </div>
      ))}
    </>
  );
}
