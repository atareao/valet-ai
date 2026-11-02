### Entorno de producción real (`co1`) — verificado el 2026-10-26

El despliegue de producción **no** usa el `docker-compose.prod.yml` del repo: lo gestiona **Dockge** en el host `co1` (`ssh co1`).

- **Stack:** `~/docker/dockge/stacks/valet/compose.yaml` (más su `.env`). Ese directorio se monta en el contenedor de Dockge como `/opt/stacks` (`DOCKGE_STACKS_DIR=/opt/stacks`), así que los labels del contenedor dicen `/opt/stacks/valet` **aunque esa ruta no exista en el host** — no confundirlo con un compose perdido. Para comprobar que un `compose.yaml` concreto gobierna el contenedor: el `com.docker.compose.config-hash` del contenedor debe coincidir con `docker compose config --hash` de ese fichero.
- **Servicio:** `valet` (`container_name: valet`), imagen `ghcr.io/atareao/valet-ai` (tag `latest`), volumen `valet_valet_data` montado en `/app/data/valet.db`.
- **Ruta pública:** `https://valet.territoriolinux.es` tras Traefik (red externa `proxy`), con `AUTH_ENABLED=true` contra el PocketID ya existente. `/api/health` responde `200` sin sesión; `/api/events` exige sesión (`401`).
- **Desplegar un release:** publicar la imagen (tag `vX.Y.Z` sobre `main`, workflow `image.yml`) y **recrear el stack desde Dockge** (equivalente a `docker compose pull && docker compose up -d` en ese directorio). Sin ese paso, el contenedor sigue con la imagen antigua.
