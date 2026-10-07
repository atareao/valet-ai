import { Form, Input } from "antd";
import { useSkills } from "../hooks/useSkills";
import { collectSkillPromptKeys, orderSkillFields } from "./skillRouter";

const { TextArea } = Input;

export interface SkillPromptFieldsProps {
  /** Settings vigentes; de aquí salen las claves `SKILL_*_PROMPT`. */
  settings: Record<string, string> | null;
}

/**
 * Sub-pestaña «Skills» de la pestaña «Prompts»: un `TextArea` por fragmento,
 * integrado en el `settingsForm` compartido (los campos viajan en el submit
 * que ya existe). Las etiquetas y el orden vienen de `GET /api/skills`; si esa
 * llamada falla, se cae a las claves de settings sin bloquear el formulario.
 */
export function SkillPromptFields({ settings }: SkillPromptFieldsProps) {
  const { skills } = useSkills();
  const fields = orderSkillFields(collectSkillPromptKeys(settings), skills);

  return (
    <>
      {fields.map((field) => (
        <Form.Item key={field.key} label={field.label} name={field.key}>
          <TextArea rows={10} />
        </Form.Item>
      ))}
    </>
  );
}
