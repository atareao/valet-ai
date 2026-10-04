/**
 * Fuente de tiles del mapa de ubicación. Se mantienen fuera del componente
 * para no romper la regla de Fast Refresh (`only-export-components`).
 */
export const TILE_URL = "https://tile.openstreetmap.org/{z}/{x}/{y}.png";
export const TILE_ATTRIBUTION =
  '&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> contributors';
