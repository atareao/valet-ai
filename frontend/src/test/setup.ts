import "@testing-library/jest-dom/vitest";

// antd 6 usa `@rc-component/resize-observer`, que dereferencia el global
// `ResizeObserver` al montar sus hooks. `jsdom` no lo implementa, así que
// cualquier componente que use un observador revienta al hacer commit de los
// efectos (`ReferenceError: ResizeObserver is not defined`). Lo doblamos aquí,
// de forma centralizada, porque el fallo es del entorno y afecta a toda la suite.
class ResizeObserverMock implements ResizeObserver {
  observe(): void {}
  unobserve(): void {}
  disconnect(): void {}
  takeRecords(): ResizeObserverEntry[] {
    return [];
  }
}

if (!("ResizeObserver" in globalThis)) {
  (globalThis as { ResizeObserver?: typeof ResizeObserver }).ResizeObserver =
    ResizeObserverMock as unknown as typeof ResizeObserver;
}
