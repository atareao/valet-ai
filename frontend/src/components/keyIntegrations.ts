/**
 * Datos y tipos de las integraciones basadas en claves de la pestaña
 * «Integraciones».
 *
 * Vive fuera de `KeyIntegration.tsx` a propósito: ese fichero solo exporta
 * componentes, de modo que el refresco en caliente de React (`react-refresh`)
 * puede reemplazarlos sin invalidar el módulo. Una constante exportada desde un
 * fichero de componentes rompe esa regla.
 */

/** Campo de texto de una integración de clave. */
export interface KeyIntegrationField {
  /** Etiqueta visible (y nombre accesible) del `Form.Item`. */
  label: string;
  /** Clave de ajuste en `settings` y `name` del campo en el formulario. */
  name: string;
  /** Placeholder del campo. */
  placeholder: string;
  /** `true` si el valor debe enmascararse (API keys y tokens). */
  password?: boolean;
}

/**
 * Definición declarativa de una integración basada en claves. Es lo único que
 * distingue a OpenWeatherMap, Google Places, Brave Search, apimail y Miniflux:
 * comparten componente, campos y comportamiento de guardado.
 */
export interface KeyIntegrationDefinition {
  /** Nombre visible de la sub-pestaña, del título y del `aria-label`. */
  name: string;
  /** Texto de ayuda bajo el título. */
  description: string;
  /**
   * Clave de ajuste que decide el estado: «Configurada» si tiene valor no
   * vacío. Para apimail y Miniflux es la **API key** (la URL base tiene valor
   * por defecto y no determina el estado).
   */
  statusKey: string;
  /** Campos del panel, en orden de presentación. */
  fields: KeyIntegrationField[];
}

const KEY_PLACEHOLDER = "Dejar vacío para usar variable de entorno";

/**
 * Las cinco integraciones de credenciales, en el orden cerrado de las
 * sub-pestañas de «Integraciones». Es la única fuente de verdad de sus
 * etiquetas, placeholders y claves de ajuste.
 */
export const KEY_INTEGRATIONS: readonly KeyIntegrationDefinition[] = [
  {
    name: "OpenWeatherMap",
    description: "Clave de la API de OpenWeatherMap para la herramienta del tiempo.",
    statusKey: "openweather_api_key",
    fields: [
      {
        label: "OpenWeatherMap API Key",
        name: "openweather_api_key",
        placeholder: KEY_PLACEHOLDER,
        password: true,
      },
    ],
  },
  {
    name: "Google Places",
    description: "Clave de la API de Google Places para buscar lugares.",
    statusKey: "google_places_api_key",
    fields: [
      {
        label: "Google Places API Key",
        name: "google_places_api_key",
        placeholder: KEY_PLACEHOLDER,
        password: true,
      },
    ],
  },
  {
    name: "Brave Search",
    description: "Clave de la API de Brave Search para la búsqueda web.",
    statusKey: "brave_search_api_key",
    fields: [
      {
        label: "Brave Search API Key",
        name: "brave_search_api_key",
        placeholder: KEY_PLACEHOLDER,
        password: true,
      },
    ],
  },
  {
    name: "apimail",
    description: "URL base y clave de la API de apimail para el correo.",
    statusKey: "apimail_api_key",
    fields: [
      {
        label: "apimail · URL base",
        name: "apimail_base_url",
        placeholder: "https://apimail.territoriolinux.es",
      },
      {
        label: "apimail · API Key",
        name: "apimail_api_key",
        placeholder: KEY_PLACEHOLDER,
        password: true,
      },
    ],
  },
  {
    name: "Miniflux",
    description: "URL base y clave de la API de Miniflux para leer los feeds.",
    statusKey: "miniflux_api_key",
    fields: [
      {
        label: "Miniflux · URL base",
        name: "miniflux_base_url",
        placeholder: "https://miniflux.territoriolinux.es",
      },
      {
        label: "Miniflux · API Key",
        name: "miniflux_api_key",
        placeholder: KEY_PLACEHOLDER,
        password: true,
      },
    ],
  },
];
