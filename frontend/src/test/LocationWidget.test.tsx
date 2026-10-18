import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import type { ReactNode } from "react";
import { LocationWidget } from "../components/widgets/LocationWidget";
import type { LocationData } from "../components/widgets/types";

// jsdom no renderiza mapas reales: sustituimos react-leaflet por dobles planos.
vi.mock("react-leaflet", () => ({
  MapContainer: ({ children }: { children?: ReactNode }) => (
    <div data-testid="map">{children}</div>
  ),
  TileLayer: () => null,
  CircleMarker: ({ children }: { children?: ReactNode }) => (
    <div data-testid="marker">{children}</div>
  ),
  Popup: ({ children }: { children?: ReactNode }) => <div>{children}</div>,
}));

// El CSS de Leaflet no aporta nada en jsdom.
vi.mock("leaflet/dist/leaflet.css", () => ({}));

const UA_DESKTOP =
  "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36";
const UA_ANDROID =
  "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Mobile Safari/537.36";
const UA_IOS =
  "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.0 Mobile/15E148 Safari/604.1";

// `userAgent` y `maxTouchPoints` son de solo lectura en jsdom: se redefinen como
// propiedades propias de la instancia y se borran en `afterEach`.
const setNavigator = (userAgent: string, maxTouchPoints = 0) => {
  Object.defineProperty(window.navigator, "userAgent", {
    value: userAgent,
    configurable: true,
  });
  Object.defineProperty(window.navigator, "maxTouchPoints", {
    value: maxTouchPoints,
    configurable: true,
  });
};

const clearNavigatorOverrides = () => {
  const nav = window.navigator as unknown as Record<string, unknown>;
  delete nav.userAgent;
  delete nav.maxTouchPoints;
  delete nav.clipboard;
};

beforeEach(() => {
  Object.defineProperty(window, "matchMedia", {
    writable: true,
    value: vi.fn().mockImplementation((query: string) => ({
      matches: false,
      media: query,
      onchange: null,
      addListener: vi.fn(),
      removeListener: vi.fn(),
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
      dispatchEvent: vi.fn(),
    })),
  });
  setNavigator(UA_DESKTOP);
});

afterEach(() => {
  vi.restoreAllMocks();
  clearNavigatorOverrides();
});

const data: LocationData = {
  title: "Oficina central",
  latitude: 40.416775,
  longitude: -3.70379,
  address: "Puerta del Sol, Madrid",
};

const COORDS = "40.416775,-3.70379";

const getAbrir = () => screen.getByRole("link", { name: "Abrir" });

const getAbrirHref = () => getAbrir().getAttribute("href") ?? "";

// En el `geo:` de Android la etiqueta viaja como decoración entre paréntesis
// dentro del propio `q` (`q={lat},{lon}({etiqueta})`), así que la consulta de
// verdad —la que debe ser siempre la coordenada— es lo que antecede al primer
// paréntesis.
const consultaDe = (href: string): string =>
  (new URL(href).searchParams.get("q") ?? "").split("(")[0];

describe("LocationWidget", () => {
  it("pinta el título, la dirección y el mapa con datos válidos", () => {
    render(<LocationWidget data={data} onAction={vi.fn()} />);

    expect(screen.getByText("Oficina central")).toBeInTheDocument();
    expect(screen.getByText("Puerta del Sol, Madrid")).toBeInTheDocument();
    expect(screen.getByTestId("map")).toBeInTheDocument();
  });

  it("no ofrece «Guardar»: sus acciones son Abrir, Cómo llegar y Copiar coordenadas", () => {
    const { container } = render(<LocationWidget data={data} onAction={vi.fn()} />);

    const acciones = Array.from(container.querySelectorAll("a, button")).map((el) =>
      el.textContent?.trim(),
    );

    expect(acciones).toEqual(["Abrir", "Cómo llegar", "Copiar coordenadas"]);
    expect(screen.queryByText("Guardar")).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Guardar" })).not.toBeInTheDocument();
  });

  it("no invoca onAction en ningún caso", async () => {
    const user = userEvent.setup();
    const onAction = vi.fn();

    // «Cómo llegar» llama a `window.open` de verdad: se espía para acallar a
    // jsdom, que no lo implementa. El clic en «Abrir» sí provoca el aviso
    // «navigation to another Document» de jsdom —nada afirma sobre él—, pero lo
    // que este test demuestra es que «Abrir» navega con el `href` del ancla sin
    // pasar por JavaScript: por eso se comprueba que `window.open` no se ha
    // llamado todavía al llegar a ese punto.
    const open = vi.spyOn(window, "open").mockReturnValue({} as Window);

    render(<LocationWidget data={data} onAction={onAction} />);

    await user.click(getAbrir());
    expect(open).not.toHaveBeenCalled();

    await user.click(screen.getByRole("button", { name: "Cómo llegar" }));
    await user.click(screen.getByRole("button", { name: "Copiar coordenadas" }));

    expect(onAction).not.toHaveBeenCalled();
  });

  it("«Abrir» resuelve a geo: sin fijar paquete en Android", () => {
    setNavigator(UA_ANDROID);
    render(<LocationWidget data={data} onAction={vi.fn()} />);

    const href = getAbrirHref();

    expect(href.startsWith("geo:")).toBe(true);
    expect(href).toContain(COORDS);
    expect(href).not.toContain("package=");
  });

  // --- Etiqueta del pin: cableado `title ?? address` del widget ---

  it("la etiqueta viaja codificada desde el widget (Android)", () => {
    setNavigator(UA_ANDROID);
    render(
      <LocationWidget
        data={{
          latitude: data.latitude,
          longitude: data.longitude,
          title: "Café (esquina)",
        }}
        onAction={vi.fn()}
      />,
    );

    const href = getAbrirHref();

    // El codificador es estricto RFC 3986: los paréntesis del título van
    // escapados para no cerrar la decoración del `geo:`.
    expect(href).toContain("Caf%C3%A9%20%28esquina%29");
    // Y la consulta sigue siendo la coordenada, no la etiqueta.
    expect(consultaDe(href)).toBe(COORDS);
  });

  it("sin title se usa la address como etiqueta (Android)", () => {
    setNavigator(UA_ANDROID);
    render(
      <LocationWidget
        data={{
          latitude: data.latitude,
          longitude: data.longitude,
          address: "Puerta del Sol, Madrid",
        }}
        onAction={vi.fn()}
      />,
    );

    const href = getAbrirHref();

    // `encodeURIComponent` escapa la coma como `%2C` y el espacio como `%20`.
    expect(href).toContain("(Puerta%20del%20Sol%2C%20Madrid)");
    expect(consultaDe(href)).toBe(COORDS);
  });

  it("sin title ni address el destino no lleva paréntesis", () => {
    setNavigator(UA_ANDROID);
    render(
      <LocationWidget
        data={{ latitude: data.latitude, longitude: data.longitude }}
        onAction={vi.fn()}
      />,
    );

    const href = getAbrirHref();

    expect(href).toBe(`geo:${COORDS}?q=${COORDS}`);
    expect(href).not.toContain("(");
    expect(href).not.toContain(")");
  });

  it("el title tiene prioridad sobre la address como etiqueta", () => {
    setNavigator(UA_ANDROID);
    render(<LocationWidget data={data} onAction={vi.fn()} />);

    const href = getAbrirHref();

    expect(href).toContain("(Oficina%20central)");
    expect(href).not.toContain("Puerta");
  });

  it("«Abrir» resuelve a Apple Maps en iOS", () => {
    setNavigator(UA_IOS);
    render(<LocationWidget data={data} onAction={vi.fn()} />);

    const href = getAbrirHref();

    expect(href).toContain("maps.apple.com");
    expect(href).toContain(COORDS);
  });

  it("«Abrir» resuelve a Google Maps en escritorio", () => {
    setNavigator(UA_DESKTOP);
    render(<LocationWidget data={data} onAction={vi.fn()} />);

    const href = getAbrirHref();

    expect(href.startsWith("https://www.google.com/maps/search/?api=1&query=")).toBe(true);
    expect(href).toContain(COORDS);
  });

  it("«Abrir» no abre una pestaña nueva", () => {
    render(<LocationWidget data={data} onAction={vi.fn()} />);

    expect(getAbrir()).not.toHaveAttribute("target");
  });

  it("«Abrir» respeta disabled: antd retira el href del ancla", () => {
    render(<LocationWidget data={data} onAction={vi.fn()} disabled />);

    const ancla = screen.getByText("Abrir").closest("a");

    expect(ancla).not.toBeNull();
    expect(ancla).not.toHaveAttribute("href");
    expect(ancla).toHaveAttribute("aria-disabled", "true");
    expect(screen.queryByRole("link", { name: "Abrir" })).not.toBeInTheDocument();
  });

  it("«Cómo llegar» abre Google Maps en una pestaña nueva", async () => {
    const user = userEvent.setup();
    const open = vi.spyOn(window, "open").mockReturnValue({} as Window);

    render(<LocationWidget data={data} onAction={vi.fn()} />);

    await user.click(screen.getByRole("button", { name: "Cómo llegar" }));

    expect(open).toHaveBeenCalledWith(
      `https://www.google.com/maps/search/?api=1&query=${COORDS}`,
      "_blank",
      "noopener",
    );
  });

  it("«Copiar coordenadas» las copia al portapapeles", async () => {
    const user = userEvent.setup();
    const writeText = vi.fn().mockResolvedValue(undefined);

    Object.defineProperty(window.navigator, "clipboard", {
      value: { writeText },
      configurable: true,
    });

    render(<LocationWidget data={data} onAction={vi.fn()} />);

    await user.click(screen.getByRole("button", { name: "Copiar coordenadas" }));

    expect(writeText).toHaveBeenCalledWith("40.416775, -3.70379");
  });

  it("sin coordenadas muestra un aviso y no lanza", () => {
    const sinCoords: LocationData = {
      title: "Ubicación desconocida",
      address: "Dónde?",
    };

    render(<LocationWidget data={sinCoords} onAction={vi.fn()} />);

    expect(screen.getByText(/coordenadas/i)).toBeInTheDocument();
  });

  it("con latitud NaN muestra un aviso y no lanza", () => {
    render(
      <LocationWidget
        data={{ title: "X", latitude: Number.NaN, longitude: -3.7 }}
        onAction={vi.fn()}
      />,
    );

    expect(screen.getByText(/coordenadas/i)).toBeInTheDocument();
    expect(screen.queryByTestId("map")).not.toBeInTheDocument();
  });

  it("con longitud Infinity muestra un aviso y no lanza", () => {
    render(
      <LocationWidget
        data={{ title: "X", latitude: 40.4, longitude: Number.POSITIVE_INFINITY }}
        onAction={vi.fn()}
      />,
    );

    expect(screen.getByText(/coordenadas/i)).toBeInTheDocument();
    expect(screen.queryByTestId("map")).not.toBeInTheDocument();
  });

  it("con data null muestra un aviso y no lanza", () => {
    render(
      <LocationWidget
        data={null as unknown as LocationData}
        onAction={vi.fn()}
      />,
    );

    expect(screen.getByText(/coordenadas/i)).toBeInTheDocument();
  });

  it("con data undefined muestra un aviso y no lanza", () => {
    render(
      <LocationWidget
        data={undefined as unknown as LocationData}
        onAction={vi.fn()}
      />,
    );

    expect(screen.getByText(/coordenadas/i)).toBeInTheDocument();
  });
});
