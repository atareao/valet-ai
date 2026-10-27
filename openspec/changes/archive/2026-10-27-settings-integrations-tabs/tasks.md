# Tasks — settings-integrations-tabs

## 1. Planificación (SDD)

- [x] 1.1 Crear el change proposal `openspec/changes/settings-integrations-tabs/`.
- [x] 1.2 Escribir `proposal.md` (why / what changes / impact).
- [x] 1.3 Escribir el delta `specs/frontend/spec.md` (REMOVED del requisito «API Keys tab» + ADDED de «Claves», sub-pestañas de Integraciones, indicadores de estado y cobertura de tests).
- [x] 1.4 Validar con `openspec validate settings-integrations-tabs --strict`.
- [x] 1.5 **Aprobación explícita del usuario** antes de escribir código o tests.

## 2. RED — tests de la nueva estructura

- [x] 2.1 Añadir a `frontend/src/test/SettingsDialog.test.tsx` el escenario de sub-pestañas
      horizontales en «Integraciones», en el orden Strava, OpenWeatherMap, Google Places,
      Brave Search, apimail, Miniflux, ZenNotes.
- [x] 2.2 Añadir el escenario de que cada sub-pestaña tiene su propio botón «Guardar» y que
      guardar en una sub-pestaña de claves llama a `updateSettings` con la clave correspondiente.
- [x] 2.3 Añadir el escenario de la URL base con placeholder por defecto
      (`https://apimail.territoriolinux.es`, `https://miniflux.territoriolinux.es`) y el de las
      claves con placeholder «Dejar vacío para usar variable de entorno».
- [x] 2.4 Añadir el escenario de los indicadores de estado: «Conectada»/«No conectada» (Strava),
      «Configurada»/«Sin configurar» (OpenWeatherMap, Google Places, Brave Search, apimail,
      Miniflux) y «Configurado»/«Sin configurar» (ZenNotes), **visibles desde el primer render**
      del panel Integraciones sin abrir cada sub-pestaña (mock de `GET /api/strava/status` y
      `GET /api/zennotes/status`).
- [x] 2.5 Añadir a `frontend/src/components/IngestKeysSection.test.tsx` (o al test del diálogo) el
      escenario de que «Claves» contiene **solo** `IngestKeysSection`: crear, listar con prefijo y
      fecha de uso, y revocar con confirmación; y que el token completo se muestra una única vez.
- [x] 2.6 Ejecutar `npx vitest run` y **confirmar el fallo** de los tests nuevos (el resto en verde).

## 3. GREEN — extracción de componentes y reestructuración

- [x] 3.1 Extraer el panel de integraciones de clave a `frontend/src/components/`: `keyIntegrations.ts`
      (tabla `KEY_INTEGRATIONS`, única fuente de etiquetas, placeholders y claves de ajuste),
      `KeyIntegration.tsx` (componente genérico guiado por esa tabla, props con interfaz exportada) e
      `IntegrationsTab.tsx` (contenedor con el `Tabs` anidado de sub-pestañas).
- [x] 3.2 Reestructurar la pestaña `integrations` de `SettingsDialog.tsx` como `Tabs` anidado
      horizontal con el orden cerrado: Strava, OpenWeatherMap, Google Places, Brave Search,
      apimail, Miniflux, ZenNotes.
- [x] 3.3 Mantener el `Form` (`settingsForm`, `onFinish={handleSettingsSubmit}`) y las claves de
      ajuste sin cambios: `openweather_api_key`, `google_places_api_key`, `brave_search_api_key`,
      `apimail_base_url`, `apimail_api_key`, `miniflux_base_url`, `miniflux_api_key`.
- [x] 3.4 Añadir un botón «Guardar» **por sub-pestaña** que dispare el submit de `settingsForm`;
      Strava y ZenNotes conservan sus flujos actuales (`PUT /api/strava/...`,
      `PUT /api/zennotes/token`).
- [x] 3.5 Ejecutar `npx vitest run` hasta verde y `npx tsc --noEmit` sin errores.

## 4. Indicador de configuración sin montaje perezoso

- [x] 4.1 Resolver el montaje perezoso de las sub-pestañas de antd (p. ej. `forceRender` en las
      sub-pestañas, o derivar el estado fuera de ellas) para que el indicador de la etiqueta sea
      correcto **desde el primer render** del panel Integraciones.
- [x] 4.2 Derivar el estado de Strava de `useStrava()` (`GET /api/strava/status`) y el de ZenNotes
      de `useZennotes()` (`GET /api/zennotes/status`, campo `configured`).
- [x] 4.3 Derivar el estado de OpenWeatherMap, Google Places, Brave Search, apimail y Miniflux del
      valor no vacío de su clave en `settings` (apimail y Miniflux según la API key).
- [x] 4.4 Añadir el indicador en la etiqueta de la sub-pestaña (Tag/punto de color) **con texto
      accesible**, además de una línea de estado dentro del panel; debe actualizarse tras guardar.

## 5. Renombrar «API Keys» a «Claves»

- [x] 5.1 Cambiar la etiqueta de la pestaña de nivel superior de «API Keys» a «Claves».
- [x] 5.2 Dejar en «Claves» únicamente `<IngestKeysSection />` (sin campos de API keys, sin URL
      base de apimail/Miniflux y sin el token de ZenNotes).
- [x] 5.3 Actualizar los comentarios JSDoc de `IngestKeysSection.tsx` y `ZennotesSection.tsx` que
      dicen «pestaña API Keys».

## 6. Actualizar los tests existentes

- [x] 6.1 Reemplazar `screen.getByText("API Keys")` (líneas ~658, 709, 740, 777, 806, 840, 879,
      893) por la navegación Integraciones → sub-pestaña correspondiente.
- [x] 6.2 Actualizar la lista de etiquetas de campos (~1324) para que refleje la nueva ubicación.
- [x] 6.3 Añadir la comprobación explícita de que «Claves» no contiene ningún campo de credencial.

## 7. REFACTOR — verificación

- [x] 7.1 `npx tsc --noEmit` sin errores.
- [x] 7.2 `npm run lint` sin warnings nuevos.
- [x] 7.3 `npx vitest run` en verde.

## 8. Verificación manual

- [x] 8.1 Levantar con `just dev` (Podman) y revisar el diálogo de ajustes. *(Omitida por decisión del usuario: se cierra con la cobertura de tests, 626 en verde.)*
- [x] 8.2 Revisar en pantalla estrecha (móvil) que las 7 sub-pestañas en horizontal no desbordan
      el modal (scroll horizontal del `Tabs` o envoltura aceptable). *(Omitida por decisión del usuario.)*
- [x] 8.3 Comprobar que los indicadores son correctos al abrir el panel, sin abrir cada sub-pestaña,
      y que se actualizan tras guardar. *(Cubierta por los tests: «indicador… desde el primer render» y el de apertura/cierre/reapertura; la comprobación visual queda pendiente para el despliegue.)*

## 9. Cierre

- [ ] 9.1 `openspec archive settings-integrations-tabs`.

## 10. Correcciones de auditoría

- [x] 10.1 `aria-label="Integraciones"` en el `Tabs` anidado, con test.
- [x] 10.2 Revalidar `GET /api/strava/status` y `GET /api/zennotes/status` en cada apertura del diálogo (una lectura por apertura).
- [x] 10.3 Indicador «Sin datos» cuando falla la lectura de estado remoto.
- [x] 10.4 Copy unificado «Configurado» / «Sin configurar» en panel y etiqueta de ZenNotes.
- [x] 10.5 Color del indicador de claves derivado de `isKeyConfigured`, no del texto.
- [x] 10.6 `CONFIGURED_LABEL` / `UNCONFIGURED_LABEL` sin exportar.
- [x] 10.7 Refactor a `KeyIntegration` + tabla `KEY_INTEGRATIONS` (elimina los cinco componentes duplicados).
- [x] 10.8 Tests de no duplicación de peticiones (una por endpoint y apertura).
- [x] 10.9 Tipo `SettingsMap` reutilizado en las interfaces de ajustes.
- [x] 10.10 Cero warnings de lint: datos de `KEY_INTEGRATIONS` fuera del fichero de componentes.
- [x] 10.11 Intro en credenciales de Strava y en el token de ZenNotes guarda su propio formulario (sin submit implícito del formulario de ajustes).
- [x] 10.12 `aria-label="Integraciones"` con rol navegable (`role="group"`) y test sobre el rol, no sobre el atributo.
- [x] 10.13 Frescura de los indicadores: `destroyOnHidden` en el `Modal` desmonta el contenido al cerrar, de modo que cada apertura relee ambos endpoints; test de apertura/cierre/reapertura y aserción de que el contenido no queda en el DOM.
