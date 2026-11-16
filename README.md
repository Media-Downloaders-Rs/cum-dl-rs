# Cum.st DL en Rust

Aplicacion grafica independiente para creadores HTTPS de cum.st, Fansly y OnlyFans. Admite varios enlaces, uno por linea. Abre abrir.cmd para iniciar cum-dl.exe.

Los parametros de presentacion display=collages y ppsort=popular se aceptan, pero se consulta el catalogo completo por fecha reciente. Los collages no se descargan como originales. Los filtros de contenido, mensajes y enlaces de otros sitios se rechazan.

Los archivos se guardan directamente bajo downloads/nombre [servicio-ID]/hash.ext. SQLite, parciales y seguimiento quedan en downloads/.cum-dl/. Listar y Descargar reutilizan el catalogo guardado; Buscar nuevos consulta las paginas recientes hasta la frontera conocida; Revisar todo vuelve a comprobar tambien publicaciones antiguas. Se conserva la carpeta aunque cambie el nombre publico. Ninguna actualizacion borra medios.

## Fuentes e integridad

Se descargan las variantes originales declaradas por la API desde cdn.cum.st, comprobando tamano y SHA-256 antes de publicarlas. Si un original falla, su vista previa puede guardarse como hash.thumbnail.webp desde img.cum.st, con etiqueta y seguimiento separados. Una miniatura nunca completa un original: al volver a descargar se intenta obtener el original y se conserva la miniatura existente. No requiere navegador ni cuenta personal.

Los metadatos incompletos quedan pendientes; datos contradictorios o consultas canceladas no sustituyen un catalogo valido. Buscar nuevos conserva el historial y evita revisar todo en cada uso; para cambios antiguos usa Revisar todo. El escaneo incremental depende del orden reciente publicado por el sitio.

## Compilar

Rust estable MSVC y herramientas C++/Windows SDK:

```powershell
cargo build --release --locked
.\target\release\cum-dl.exe
```

Todas las fuentes estan incluidas; no necesita otros repositorios, Python ni navegador. Distribuye un unico ejecutable grafico. Tras compilar, cierra la aplicacion y ejecuta actualizar.cmd para instalarlo conservando una copia anterior y comprobando el SHA-256.

## Desarrollo

```powershell
cargo test --locked -- --test-threads=1
cargo fmt --check
cargo clippy --locked --all-targets --all-features -- -D warnings
powershell -NoProfile -File tools/verify-standalone.ps1
```

src/main.rs inicia la GUI; gui.rs contiene los controles; engine.rs coordina transferencias; transport.rs gestiona HTTP y reintentos; state.rs conserva SQLite y hashes; sites/cum.rs descubre originales y mantiene el catalogo incremental. Los demas adaptadores forman parte de la copia local del motor. ENGINE_MAINTENANCE.md explica la sincronizacion de repos independientes.

## Licencia y uso responsable

El código propio se distribuye bajo la [licencia MIT](LICENSE). El propósito educativo y las responsabilidades de uso se describen en [USAGE_NOTICE.md](USAGE_NOTICE.md). Esta licencia no concede derechos sobre contenido ajeno.
