import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { act, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { App as AntdApp } from "antd";
import type { ReactNode } from "react";
import dayjs from "dayjs";
import type { CalendarEvent } from "../types";

// Se mockea solo el cliente HTTP: el hook `useEvents` y la vista corren de
// verdad (spec `frontend/agenda`). `CalendarView` necesita `matchMedia` (antd)
// y el cliente real pegaría contra la red, así que el mock es un objeto propio
// (hoisted), como en el RED de `NotesView`/`JournalView`.
const mockApi = vi.hoisted(() => ({
  listEvents: vi.fn(),
  createEvent: vi.fn(),
  updateEvent: vi.fn(),
  deleteEvent: vi.fn(),
}));

vi.mock("../api/client", () => ({
  api: mockApi,
  BASE_URL: "/api",
}));

import { CalendarView } from "./CalendarView";

beforeEach(() => {
  vi.clearAllMocks();
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

/**
 * Un evento de hoy, para que caiga siempre en el mes que el calendario pinta.
 * La hora se compone en local (`format`, no `toISOString`) para no desplazar el
 * día al oeste de UTC.
 */
const event = (overrides: Partial<CalendarEvent> = {}): CalendarEvent => ({
  id: "ev1",
  profile_id: "profile-id",
  title: "Reunión",
  start_time: dayjs().hour(10).minute(0).second(0).format("YYYY-MM-DDTHH:mm:ss"),
  end_time: dayjs().hour(11).minute(0).second(0).format("YYYY-MM-DDTHH:mm:ss"),
  scope: "personal",
  category: "work",
  all_day: false,
  created_at: "2026-10-18T09:00:00",
  updated_at: "2026-10-18T09:00:00",
  ...overrides,
});

// `CalendarView` usa `App.useApp()` para el aviso de error (`message.error`),
// así que el render necesita el contexto de `<AntdApp>` (patrón del repo).
const AppWrapper = ({ children }: { children: ReactNode }) => (
  <AntdApp>{children}</AntdApp>
);

/** Abre el detalle de un evento clicando su insignia en el calendario. */
const openDetail = async (
  user: ReturnType<typeof userEvent.setup>,
  title: string,
) => {
  await user.click(await screen.findByText(title));
  return await screen.findByRole("dialog");
};

/** Confirma el `Popconfirm` del botón «Delete» del detalle. */
const confirmDelete = async (user: ReturnType<typeof userEvent.setup>) => {
  const dialog = screen.getByRole("dialog");
  // El icono de antd suma su `aria-label` al nombre accesible del botón
  // («delete Delete»), así que se compara sin anclas.
  await user.click(within(dialog).getByRole("button", { name: /Delete/ }));

  // El popconfirm abre su burbuja con otro botón «Delete» (el `okText`).
  // El del portal se añade después del trigger en el DOM.
  expect(
    await screen.findByText("Are you sure you want to delete this event?"),
  ).toBeInTheDocument();
  const deleteButtons = screen.getAllByRole("button", { name: /Delete/ });
  await user.click(deleteButtons[deleteButtons.length - 1]);
};

describe("CalendarView — borrado de eventos", () => {
  it("borra de verdad: llama a deleteEvent con el id y recarga el listado", async () => {
    const user = userEvent.setup();
    mockApi.listEvents.mockResolvedValue([event()]);
    mockApi.deleteEvent.mockResolvedValue(undefined);

    render(<CalendarView />, { wrapper: AppWrapper });

    await openDetail(user, "Reunión");
    expect(mockApi.listEvents).toHaveBeenCalledTimes(1);

    await confirmDelete(user);

    await waitFor(() => {
      expect(mockApi.deleteEvent).toHaveBeenCalledWith("ev1");
    });
    // Tras un borrado con éxito, el listado se recarga.
    await waitFor(() => {
      expect(mockApi.listEvents).toHaveBeenCalledTimes(2);
    });
    // Y el detalle se cierra (spec `frontend/agenda`, «Delete event from
    // calendar»). Ojo: en jsdom el nodo del `Modal` de antd NO desaparece del
    // DOM. `rc-motion` aguarda un `transitionend`/`animationend` que jsdom nunca
    // emite y `Modal` no pasa `motionDeadline`, así que la animación de salida
    // no «termina» y el nodo queda montado (misma limitación documentada en
    // `SettingsDialog.test.tsx`). La señal equivalente es la de `rc-motion`: al
    // procesar antd `open=false`, el nodo recibe `ant-zoom-leave`. Si el borrado
    // con éxito no cerrase el detalle, seguiría en `ant-zoom-enter-active` y
    // esta aserción fallaría.
    await waitFor(() => {
      const dialog = screen.queryByRole("dialog");
      if (dialog) expect(dialog).toHaveClass("ant-zoom-leave");
    });
  });

  it("un borrado fallido avisa y deja el detalle abierto", async () => {
    const user = userEvent.setup();
    mockApi.listEvents.mockResolvedValue([event()]);
    mockApi.deleteEvent.mockRejectedValue(new Error("boom"));

    render(<CalendarView />, { wrapper: AppWrapper });

    await openDetail(user, "Reunión");

    await confirmDelete(user);

    // Aviso de error visible (patrón `message.error` del repo: el mensaje).
    expect(await screen.findByText("boom")).toBeInTheDocument();
    // El detalle sigue abierto con el evento (no se finge el borrado).
    expect(
      within(screen.getByRole("dialog")).getByText("Reunión"),
    ).toBeInTheDocument();
  });
});

// ---------------------------------------------------------------------------
// Change `calendar-range-boundaries` — el rango consultado se envía en hora
// local (RFC 3339 con offset) y como intervalo semiabierto. El fallo original:
// `end` se formateaba como `YYYY-MM-DD` y, cuando el último día del mes caía en
// sábado (octubre de 2026), todo evento que **terminaba** ese día desaparecía
// de la rejilla porque el backend comparaba las cadenas como texto.
// ---------------------------------------------------------------------------

/** RFC 3339 con offset de zona (`+HH:MM` o `-HH:MM`), nunca una fecha suelta. */
const RFC3339_WITH_OFFSET = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}[+-]\d{2}:\d{2}$/;

/**
 * Devuelve los dos límites que la vista pasa a `useEvents`. Se aserta en
 * `api.listEvents` porque es el borde observable: `useEvents` reenvía sus
 * argumentos tal cual (`useEvents(start, end)` → `api.listEvents(start, end)`),
 * así que equivale a observar lo que recibe el hook sin mockearlo.
 */
const requestedRange = (): [string, string] => {
  const call = mockApi.listEvents.mock.calls[0] as [string, string] | undefined;
  if (!call) throw new Error("CalendarView no ha consultado los eventos");
  return call;
};

describe("CalendarView — límites del rango consultado", () => {
  it("envía los dos límites como RFC 3339 con el offset local, no como YYYY-MM-DD", async () => {
    mockApi.listEvents.mockResolvedValue([]);

    render(<CalendarView />, { wrapper: AppWrapper });

    await waitFor(() => {
      expect(mockApi.listEvents).toHaveBeenCalledTimes(1);
    });

    const [start, end] = requestedRange();

    expect(start).toMatch(RFC3339_WITH_OFFSET);
    expect(end).toMatch(RFC3339_WITH_OFFSET);
    // El formato viejo (fecha suelta) no SHALL volver: el offset es obligatorio.
    expect(start).not.toMatch(/^\d{4}-\d{2}-\d{2}$/);
    expect(end).not.toMatch(/^\d{4}-\d{2}-\d{2}$/);
    // Tampoco se fabrica un `23:59:59` (perdería el último segundo del día).
    expect(dayjs(end).format("HH:mm:ss")).toBe("00:00:00");
  });

  it("usa un intervalo semiabierto: el extremo superior es el inicio del día siguiente al último visible", async () => {
    mockApi.listEvents.mockResolvedValue([]);

    render(<CalendarView />, { wrapper: AppWrapper });

    await waitFor(() => {
      expect(mockApi.listEvents).toHaveBeenCalledTimes(1);
    });

    const [start, end] = requestedRange();

    // Los valores esperados se recalculan con `dayjs` (la misma librería y la
    // misma lógica de calendario que la vista) en lugar de fijar una fecha:
    // `CalendarView` arranca con `useState(dayjs())`, así que el mes visible es
    // «ahora». Fijarlo con temporizadores falsos haría frágil toda la suite
    // (los tests de borrado usan `userEvent`); la invariante que se aserta
    // —inicio del primer día visible / inicio del día siguiente al último— se
    // cumple sea cual sea el mes en que se ejecuten las pruebas.
    const now = dayjs();
    const firstVisibleDay = now.startOf("month").startOf("week").startOf("day");
    const lastVisibleDay = now.endOf("month").endOf("week").startOf("day");

    expect(start).toBe(firstVisibleDay.format());
    expect(end).toBe(lastVisibleDay.add(1, "day").format());

    // Semiabierto y cubriendo el último día del mes visible: el inicio de ese
    // día cae estrictamente dentro del rango pedido, así que ningún evento de
    // ese día puede quedar fuera por el borde superior.
    const lastDayOfMonth = now.endOf("month").startOf("day");
    expect(dayjs(start).isBefore(lastDayOfMonth)).toBe(true);
    expect(dayjs(end).isAfter(lastDayOfMonth)).toBe(true);
  });

  describe("regresión de octubre de 2026 (el día 31 cae en sábado)", () => {
    // Solo se falsea `Date`: el mes visible se fija con `useState(dayjs())`, así
    // que congelar el reloj basta para que la vista arranque en octubre de 2026.
    // Los temporizadores se dejan reales para no romper `waitFor`/`userEvent`
    // (falsearlos obligaría a `advanceTimers` y colgaría los tests de borrado).
    beforeEach(() => {
      vi.useFakeTimers({ toFake: ["Date"] });
      vi.setSystemTime(new Date(2026, 9, 15, 12, 0, 0));
    });

    afterEach(() => {
      vi.useRealTimers();
    });

    it("el extremo superior es posterior al inicio del 31 de octubre y el evento de ese día aparece en la rejilla", async () => {
      const oct31Start = dayjs("2026-10-31T09:00:00");
      mockApi.listEvents.mockResolvedValue([
        event({
          id: "ev-oct31",
          title: "Rodaje cómodo — 8 km",
          start_time: oct31Start.format("YYYY-MM-DDTHH:mm:ss"),
          end_time: oct31Start.add(1, "hour").format("YYYY-MM-DDTHH:mm:ss"),
        }),
      ]);

      render(<CalendarView />, { wrapper: AppWrapper });

      // Se vacía la cola de microtasks con `act` en lugar de `waitFor`: con
      // `Date` falseado el reloj no avanza y `waitFor` no podría caducar.
      await act(async () => {});

      const [start, end] = requestedRange();
      expect(start).toMatch(RFC3339_WITH_OFFSET);
      expect(end).toMatch(RFC3339_WITH_OFFSET);

      // Regresión: con el código viejo `end` era "2026-10-31" (fecha suelta) y
      // el backend descartaba todo evento que terminase ese día.
      expect(end).not.toBe("2026-10-31");
      expect(dayjs(end).isAfter(dayjs("2026-10-31T00:00:00"))).toBe(true);
      // Exactamente el inicio del día siguiente al último día visible.
      expect(dayjs(end).isSame(dayjs("2026-11-01T00:00:00"))).toBe(true);

      // Y el evento del 31 sigue apareciendo en la celda del calendario.
      expect(screen.getByText("Rodaje cómodo — 8 km")).toBeInTheDocument();
    });
  });
});
