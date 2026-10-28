# Tasks — settings-tabs-vertical

## 1. Planificación (SDD)

- [x] 1.1 Crear el change `openspec/changes/settings-tabs-vertical/`.
- [x] 1.2 Escribir `proposal.md` (why / what changes / impact).
- [x] 1.3 Escribir el delta `specs/frontend/spec.md`: requisito añadido de orientación vertical,
      requisito de «Integraciones» modificado y requisito de desbordamiento horizontal retirado.
- [x] 1.4 Validar con `openspec validate settings-tabs-vertical --strict`.
- [x] 1.5 **Aprobación explícita del usuario** antes de escribir código o tests.

## 2. RED — tests de la orientación vertical

- [x] 2.1 En `frontend/src/test/SettingsDialog.test.tsx`, invertir la aserción del test
      «organiza «Integraciones» en ocho sub-pestañas horizontales en el orden acordado»: renombrarlo
      a «verticales» y exigir `aria-orientation="vertical"` en el `tablist` anidado (hoy exige
      `not.toHaveAttribute("aria-orientation", "vertical")`).
- [x] 2.2 Añadir el escenario de las **diez pestañas superiores**: el `tablist` que las aloja tiene
      `aria-orientation="vertical"` y sigue conteniendo las diez, en orden. No debe convivir con el
      test existente `shows exactly ten top-level tabs and no Herramientas tab`.
- [x] 2.3 Añadir el escenario de las sub-pestañas de «Prompts» (6), «Memoria» (2) y «Generación»
      (5): cada `tablist` anidado tiene `aria-orientation="vertical"`.
- [x] 2.4 Añadir el escenario de las sub-pestañas de «Skills» (una por skill del catálogo mockeado):
      su `tablist` tiene `aria-orientation="vertical"`.
- [x] 2.5 Ejecutar `npx vitest run src/test/SettingsDialog.test.tsx` y **confirmar el fallo** de los
      tests nuevos (el resto en verde).

## 3. GREEN — `tabPosition="left"` en los seis `Tabs`

- [x] 3.1 `SettingsDialog.tsx`: `tabPosition="left"` en el `Tabs` superior (Perfil … Notificaciones).
- [x] 3.2 `SettingsDialog.tsx`: `tabPosition="left"` en los `Tabs` anidados de «Prompts», «Memoria»
      y «Generación».
- [x] 3.3 `SkillsTab.tsx`: `tabPosition="left"` en el `Tabs` de skills.
- [x] 3.4 `IntegrationsTab.tsx`: `tabPosition="left"` en el `Tabs` de integraciones.
- [x] 3.5 Ejecutar `npx vitest run` (toda la suite) y `npx tsc --noEmit` sin errores. Confirmar que
      NO cambia ninguna clave, orden, `forceRender` ni `aria-label`.

## 4. REFACTOR y verificación

- [x] 4.1 Revisar con `npx eslint .` y el formateador del proyecto; sin `style` nuevos ni
      sobrescrituras de CSS de antd (la lista lateral se dimensiona sola).
- [ ] 4.2 Comprobación manual del render: `just dev` (Podman) y abrir Ajustes en escritorio y en
      ventana estrecha; verificar que la lista superior de diez pestañas cabe en alto y que el
      panel mantiene su scroll propio en «Generación» y «Skills».
      **Diferida por decisión del usuario (2026-10-28)**: no se levanta el stack de desarrollo; el
      aspecto se confirma sobre el despliegue del release. Los tests fijan la orientación
      (`aria-orientation="vertical"` en los seis `tablist`), no el aspecto.
- [x] 4.3 Ejecutar `just check-all`.
- [ ] 4.4 Actualizar `openspec/specs/frontend` con `openspec archive settings-tabs-vertical` una vez
      aprobado el resultado.
