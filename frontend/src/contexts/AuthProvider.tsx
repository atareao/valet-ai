import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { api, UNAUTHORIZED_EVENT } from "../api/client";
import {
  AuthContext,
  type AuthContextValue,
  type AuthUser,
} from "./AuthContext";

/**
 * Base pública de la app. `vite.config.ts` usa `base: ''`, que Vite normaliza a
 * `"./"` (o `""` según la versión): en ese caso queremos navegar a la raíz
 * absoluta `/`, no resolver una ruta relativa al path actual. Si algún día se
 * despliega bajo un subpath con un `base` real (p. ej. `/valet/`), se conserva.
 */
const APP_BASE_URL =
  import.meta.env.BASE_URL && import.meta.env.BASE_URL !== "./"
    ? import.meta.env.BASE_URL
    : "/";

/**
 * El `end_session_url` del proveedor solo se sigue si es una URL absoluta
 * `https:`. Cualquier otro valor (`http:`, `javascript:`, relativa o vacía) se
 * descarta para evitar un open redirect y se cae a {@link APP_BASE_URL}.
 */
function isSafeEndSessionUrl(url: string | null | undefined): url is string {
  if (!url) return false;
  try {
    return new URL(url).protocol === "https:";
  } catch {
    return false;
  }
}

/**
 * Resuelve la sesión OIDC al montar (`GET /api/auth/me`), expone el usuario y
 * reacciona a los 401 globales emitidos por el cliente HTTP.
 *
 * Mientras `me` está en curso `loading` es `true`; ante un 401 (o cualquier
 * fallo de red) el usuario queda como no autenticado.
 */
export const AuthProvider: React.FC<{ children: React.ReactNode }> = ({
  children,
}) => {
  const [user, setUser] = useState<AuthUser | null>(null);
  const [loading, setLoading] = useState(true);
  const [loggingOut, setLoggingOut] = useState(false);
  const meRequestRef = useRef<Promise<AuthUser> | null>(null);

  useEffect(() => {
    // La petición se cachea para que el doble montaje de StrictMode no lance
    // dos `GET /api/auth/me`: ambos efectos se suscriben a la misma promesa.
    if (meRequestRef.current === null) {
      meRequestRef.current = api.getMe();
    }

    let active = true;
    meRequestRef.current
      .then((me) => {
        if (active) setUser(me);
      })
      .catch(() => {
        if (active) setUser(null);
      })
      .finally(() => {
        if (active) setLoading(false);
      });

    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    const onUnauthorized = () => {
      setUser(null);
      setLoading(false);
    };
    window.addEventListener(UNAUTHORIZED_EVENT, onUnauthorized);
    return () => window.removeEventListener(UNAUTHORIZED_EVENT, onUnauthorized);
  }, []);

  const logout = useCallback(async () => {
    setLoggingOut(true);
    let endSessionUrl: string | null = null;
    try {
      const result = await api.logout();
      endSessionUrl = result.end_session_url;
    } catch {
      // Si el cierre remoto falla, al menos liberamos la sesión local.
      endSessionUrl = null;
    } finally {
      setLoggingOut(false);
    }

    setUser(null);
    window.location.assign(
      isSafeEndSessionUrl(endSessionUrl) ? endSessionUrl : APP_BASE_URL,
    );
  }, []);

  const value = useMemo<AuthContextValue>(
    () => ({ user, loading, loggingOut, logout }),
    [user, loading, loggingOut, logout],
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
};
