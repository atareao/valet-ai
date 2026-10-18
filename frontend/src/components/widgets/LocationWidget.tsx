import type { FC } from "react";
import { Alert, App, Button, Typography } from "antd";
import {
  MapContainer,
  TileLayer,
  CircleMarker,
  Popup,
} from "react-leaflet";
import "leaflet/dist/leaflet.css";
import "./LocationWidget.css";
import type { LocationData, WidgetProps } from "./types";
import { TILE_URL, TILE_ATTRIBUTION } from "./locationTiles";
import { openLocationUrl } from "./locationLinks";

const { Text } = Typography;

const isFiniteNumber = (value: unknown): value is number =>
  typeof value === "number" && Number.isFinite(value);

/**
 * Mapa de una ubicación con sus acciones: «Abrir», «Cómo llegar» y «Copiar
 * coordenadas». Componente presentacional: no emite ninguna acción hacia el
 * backend (el prop `onAction` del tipo se conserva por compatibilidad con el
 * registry, pero ya no se usa). «Abrir» resuelve su destino según la plataforma
 * del navegador —`geo:` en Android, Apple Maps en iOS y Google Maps en el
 * resto— delegando en `locationLinks`; navega en el contexto actual, sin abrir
 * pestaña nueva. Tolera datos incompletos: si las coordenadas no son números
 * finitos, muestra un aviso en lugar del mapa y no lanza.
 */
export const LocationWidget: FC<WidgetProps<LocationData>> = ({ data, disabled }) => {
  const { message } = App.useApp();
  const { latitude, longitude, title, address, description } = data ?? {};

  if (!isFiniteNumber(latitude) || !isFiniteNumber(longitude)) {
    return (
      <Alert
        type="warning"
        showIcon
        message="Ubicación sin coordenadas válidas"
        description="No es posible mostrar el mapa."
      />
    );
  }

  const lat = latitude;
  const lon = longitude;

  // Destino de «Abrir» según la plataforma del navegador. Se calcula una vez
  // por render: es una función pura y barata.
  const openHref = openLocationUrl(
    { latitude: lat, longitude: lon, label: title ?? address },
    navigator.userAgent,
    navigator.maxTouchPoints,
  );

  const handleDirections = () => {
    const opened = window.open(
      `https://www.google.com/maps/search/?api=1&query=${lat},${lon}`,
      "_blank",
      "noopener",
    );
    if (opened) {
      message.success?.("Abriendo la ruta en Google Maps");
    } else {
      message.error?.("No se pudo abrir el mapa");
    }
  };

  const handleCopy = async () => {
    try {
      await navigator.clipboard?.writeText(`${lat}, ${lon}`);
      message.success?.("Coordenadas copiadas");
    } catch {
      message.error?.("No se pudieron copiar las coordenadas");
    }
  };

  return (
    <div
      style={{
        padding: 12,
        border: "1px solid rgba(255,255,255,0.12)",
        borderRadius: 8,
        background: "rgba(255,255,255,0.03)",
        maxWidth: 420,
      }}
    >
      <Text strong style={{ display: "block" }}>
        {title ?? address ?? "Ubicación"}
      </Text>
      {address && (
        <Text type="secondary" style={{ display: "block", marginBottom: 8 }}>
          {address}
        </Text>
      )}
      <div
        role="img"
        className="valet-map-dark"
        aria-label={`Mapa de la ubicación: ${title ?? address ?? `${lat}, ${lon}`}`}
        style={{
          height: 192,
          width: "100%",
          marginTop: 8,
          borderRadius: 6,
          overflow: "hidden",
        }}
      >
        <MapContainer
          key={`${lat},${lon}`}
          center={[lat, lon]}
          zoom={15}
          scrollWheelZoom={false}
          style={{ height: "100%", width: "100%" }}
        >
          <TileLayer url={TILE_URL} attribution={TILE_ATTRIBUTION} />
          <CircleMarker center={[lat, lon]} radius={8}>
            <Popup>
              {address
                ? `${title ?? "Ubicación"} — ${address}`
                : title ?? "Ubicación"}
            </Popup>
          </CircleMarker>
        </MapContainer>
      </div>
      {description && (
        <Text style={{ display: "block", marginTop: 8 }}>{description}</Text>
      )}
      <div style={{ display: "flex", gap: 8, marginTop: 12, flexWrap: "wrap" }}>
        <Button type="primary" href={openHref} disabled={disabled}>
          Abrir
        </Button>
        <Button onClick={handleDirections} disabled={disabled}>
          Cómo llegar
        </Button>
        <Button
          onClick={() => {
            void handleCopy();
          }}
          disabled={disabled}
        >
          Copiar coordenadas
        </Button>
      </div>
    </div>
  );
};