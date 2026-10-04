import { describe, it, expect, vi, beforeEach } from "vitest";
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
});

const data: LocationData = {
  title: "Oficina central",
  latitude: 40.416775,
  longitude: -3.70379,
  address: "Puerta del Sol, Madrid",
};

describe("LocationWidget", () => {
  it("pinta el título, la dirección y el mapa con datos válidos", () => {
    render(<LocationWidget data={data} onAction={vi.fn()} />);

    expect(screen.getByText("Oficina central")).toBeInTheDocument();
    expect(screen.getByText("Puerta del Sol, Madrid")).toBeInTheDocument();
    expect(screen.getByTestId("map")).toBeInTheDocument();
  });

  it("al pulsar «Guardar» llama a onAction con las coordenadas", async () => {
    const user = userEvent.setup();
    const onAction = vi.fn();

    render(<LocationWidget data={data} onAction={onAction} />);

    await user.click(screen.getByRole("button", { name: "Guardar" }));

    expect(onAction).toHaveBeenCalledWith(
      "save_place",
      expect.objectContaining({
        latitude: 40.416775,
        longitude: -3.70379,
      }),
    );
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