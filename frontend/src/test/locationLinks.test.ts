import { describe, it, expect } from "vitest";
import {
  detectPlatform,
  buildOpenLocationUrl,
  openLocationUrl,
  type OpenLocationInput,
} from "../components/widgets/locationLinks";

const ANDROID_UA =
  "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Mobile Safari/537.36";
const IOS_UA =
  "Mozilla/5.0 (iPhone; CPU iPhone OS 17_5 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Mobile/15E148 Safari/604.1";
const MAC_UA =
  "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.5 Safari/605.1.15";
const DESKTOP_UA =
  "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

const LAT = 40.416775;
const LON = -3.70379;

const PUNTO: OpenLocationInput = { latitude: LAT, longitude: LON };

describe("detectPlatform", () => {
  it("detecta Android por la UA", () => {
    expect(detectPlatform(ANDROID_UA)).toBe("android");
  });

  it("detecta iOS por la UA del iPhone", () => {
    expect(detectPlatform(IOS_UA)).toBe("ios");
  });

  it("degrada a «other» en un navegador de escritorio", () => {
    expect(detectPlatform(DESKTOP_UA)).toBe("other");
  });

  it("reconoce iPadOS declarado como Macintosh por el táctil", () => {
    expect(detectPlatform(MAC_UA, 5)).toBe("ios");
  });

  it("un Mac de verdad (sin táctil) degrada a «other»", () => {
    expect(detectPlatform(MAC_UA, 0)).toBe("other");
  });

  it("por defecto maxTouchPoints es 0", () => {
    expect(detectPlatform(MAC_UA)).toBe("other");
  });
});

describe("buildOpenLocationUrl", () => {
  it("Android usa el esquema geo: sin fijar paquete", () => {
    const href = buildOpenLocationUrl(
      { ...PUNTO, label: "Oficina central" },
      "android",
    );

    expect(href.startsWith(`geo:${LAT},${LON}?q=${LAT},${LON}`)).toBe(true);
    expect(href).toContain(encodeURIComponent("Oficina central"));
    expect(href).not.toContain("package=");
    expect(href).not.toContain("intent://");
    expect(href).not.toContain("browser_fallback_url");
  });

  it("iOS usa la URL de Apple Maps y contiene las coordenadas", () => {
    const href = buildOpenLocationUrl({ ...PUNTO, label: "Oficina central" }, "ios");

    expect(href).toBe(`https://maps.apple.com/?q=${LAT},${LON}`);
  });

  it("las demás plataformas usan la URL oficial de Google Maps", () => {
    const href = buildOpenLocationUrl({ ...PUNTO, label: "Oficina central" }, "other");

    expect(href).toBe(`https://www.google.com/maps/search/?api=1&query=${LAT},${LON}`);
  });

  it("las coordenadas se interpolan tal cual, sin redondear", () => {
    const href = buildOpenLocationUrl(
      { latitude: 40.416775123, longitude: -3.703791234 },
      "other",
    );

    expect(href).toContain("40.416775123,-3.703791234");
  });

  it("la etiqueta viaja codificada, no como consulta", () => {
    const href = buildOpenLocationUrl({ ...PUNTO, label: "Café esquina" }, "android");

    expect(href).toContain(encodeURIComponent("Café esquina"));
    expect(href).toContain(`${LAT},${LON}`);
  });

  it("los paréntesis de la etiqueta no alteran la estructura del destino", () => {
    const href = buildOpenLocationUrl({ ...PUNTO, label: "Café (esquina)" }, "android");

    expect(href).toContain("Caf%C3%A9%20%28esquina%29");
    expect(href.match(/\(/g)).toHaveLength(1);
    expect(href.match(/\)/g)).toHaveLength(1);
    expect(href).not.toContain("esquina)");
    expect(href).not.toContain("(esquina");
  });

  it("sin etiqueta no se añaden paréntesis", () => {
    const href = buildOpenLocationUrl(PUNTO, "android");

    expect(href).not.toContain("(");
    expect(href).not.toContain(")");
    expect(href).toBe(`geo:${LAT},${LON}?q=${LAT},${LON}`);
  });

  it("una etiqueta vacía no añade paréntesis", () => {
    const href = buildOpenLocationUrl({ ...PUNTO, label: "" }, "android");

    expect(href).not.toContain("(");
    expect(href).not.toContain(")");
  });
});

describe("openLocationUrl", () => {
  it("resuelve a geo: desde un navegador de Android", () => {
    const href = openLocationUrl({ ...PUNTO, label: "Oficina central" }, ANDROID_UA);

    expect(href.startsWith(`geo:${LAT},${LON}?q=${LAT},${LON}`)).toBe(true);
    expect(href).not.toContain("package=");
    expect(href).not.toContain("intent://");
  });

  it("resuelve a Apple Maps desde un iPhone", () => {
    const href = openLocationUrl({ ...PUNTO, label: "Oficina central" }, IOS_UA);

    expect(href).toBe(`https://maps.apple.com/?q=${LAT},${LON}`);
  });

  it("resuelve a Apple Maps desde un iPad declarado Macintosh", () => {
    const href = openLocationUrl({ ...PUNTO, label: "Oficina central" }, MAC_UA, 5);

    expect(href).toBe(`https://maps.apple.com/?q=${LAT},${LON}`);
  });

  it("se usan las coordenadas, no la dirección", () => {
    // Android es la única rama que traslada la etiqueta al destino: en `ios` y
    // `other` el literal «Plaza» no aparecería ni con la implementación
    // degradada, así que allí el assert no podría fallar por el motivo que
    // enuncia. Se comprueba donde la etiqueta sí llega: el `q` del `geo:` tiene
    // que arrancar en las coordenadas.
    const href = openLocationUrl({ ...PUNTO, label: "Plaza Mayor" }, ANDROID_UA);

    const { pathname, searchParams } = new URL(href);
    const q = searchParams.get("q") ?? "";

    // El «documento» del esquema geo: son las coordenadas, no la etiqueta.
    expect(pathname).toBe(`${LAT},${LON}`);

    // La etiqueta viaja como decoración entre paréntesis dentro del propio `q`
    // (`q={lat},{lon}({etiqueta})`), así que sobre el `q` entero el literal
    // «Plaza» sí aparece y un `not.toContain` ahí sería falso. Lo que demuestra
    // que la consulta son las coordenadas es que todo lo anterior al paréntesis
    // sea exactamente la coordenada.
    const consulta = q.split("(")[0];
    expect(consulta).toBe(`${LAT},${LON}`);
    expect(consulta).not.toContain("Plaza");
  });

  it("en iOS la etiqueta no llega al destino", () => {
    const href = openLocationUrl({ ...PUNTO, label: "Plaza Mayor" }, IOS_UA);

    expect(href).toBe(`https://maps.apple.com/?q=${LAT},${LON}`);
    expect(href).not.toContain("Plaza");
  });

  it("en escritorio la etiqueta no llega al destino", () => {
    const href = openLocationUrl({ ...PUNTO, label: "Plaza Mayor" }, DESKTOP_UA);

    expect(href).toBe(`https://www.google.com/maps/search/?api=1&query=${LAT},${LON}`);
    expect(href).not.toContain("Plaza");
  });

  it("en escritorio usa la URL oficial de Google Maps", () => {
    const href = openLocationUrl(PUNTO, DESKTOP_UA);

    expect(href).toBe(`https://www.google.com/maps/search/?api=1&query=${LAT},${LON}`);
  });
});
