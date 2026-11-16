use crate::{Cancel, File, Listing, Pending, safe_name, state::State};
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{HashMap, HashSet},
    future::Future,
};
use url::Url;

#[derive(Debug)]
struct IntegrityConflict(&'static str);
impl std::fmt::Display for IntegrityConflict {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for IntegrityConflict {}

/// The API can contain a lone UTF-16 surrogate in captions. JSON permits these
/// escapes, but Rust strings cannot represent them. Replace only lone surrogate
/// escapes inside strings; preserve paired surrogates and literal backslashes.
pub fn parse_json(data: &[u8]) -> Result<Value> {
    let mut result = Vec::with_capacity(data.len());
    let mut i = 0;
    let mut string = false;
    let unit = |at: usize| -> Option<u16> {
        if data.get(at..at + 2)? != b"\\u" {
            return None;
        }
        u16::from_str_radix(std::str::from_utf8(data.get(at + 2..at + 6)?).ok()?, 16).ok()
    };
    while i < data.len() {
        if data[i] == b'"' {
            string = !string;
        }
        if string && data[i] == b'\\' {
            if let Some(code) = unit(i) {
                if (0xD800..=0xDBFF).contains(&code)
                    && unit(i + 6).is_some_and(|n| (0xDC00..=0xDFFF).contains(&n))
                {
                    result.extend_from_slice(&data[i..i + 12]);
                    i += 12;
                    continue;
                }
                if (0xD800..=0xDFFF).contains(&code) {
                    result.extend_from_slice(b"\\ufffd");
                    i += 6;
                    continue;
                }
            }
            if let Some(next) = data.get(i + 1) {
                result.extend_from_slice(&[data[i], *next]);
                i += 2;
                continue;
            }
        }
        result.push(data[i]);
        i += 1;
    }
    serde_json::from_slice(&result)
        .map_err(|e| anyhow::anyhow!("JSON no reconocido en {}:{}", e.line(), e.column()))
}

fn secure(u: &Url) -> bool {
    u.scheme() == "https"
        && u.port().is_none()
        && u.username().is_empty()
        && u.password().is_none()
        && u.fragment().is_none()
}
fn identity(input: &str) -> Result<(String, String)> {
    let u = Url::parse(input)?;
    if !secure(&u) || u.host_str() != Some("cum.st") {
        bail!("Introduce un creador HTTPS de cum.st")
    }
    let p = u
        .path()
        .trim_end_matches('/')
        .split('/')
        .collect::<Vec<_>>();
    if p.len() != 4
        || p[1] != "creators"
        || !matches!(p[2], "fansly" | "onlyfans")
        || p[3].is_empty()
        || p[3].len() > 64
        || !p[3]
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
    {
        bail!("Enlace de creador no válido")
    }
    let mut keys = HashSet::new();
    for (k, v) in u.query_pairs() {
        if !keys.insert(k.to_string())
            || !match k.as_ref() {
                "display" => matches!(v.as_ref(), "collages" | "media"),
                "ppsort" => matches!(
                    v.as_ref(),
                    "popular"
                        | "recent"
                        | "oldest"
                        | "imported"
                        | "attachments"
                        | "videos"
                        | "relevance"
                ),
                _ => false,
            }
        {
            bail!("Se admite el perfil completo sin filtros de contenido")
        }
    }
    Ok((p[2].into(), p[3].into()))
}
pub fn normalize(input: &str) -> Result<String> {
    let (s, id) = identity(input)?;
    Ok(format!("https://cum.st/creators/{s}/{id}"))
}
fn api(input: &str, suffix: &str) -> Result<String> {
    let (s, id) = identity(input)?;
    Ok(format!("https://cum.st/api/v1/{s}/user/{id}/{suffix}"))
}
fn page_url(input: &str, offset: usize) -> Result<String> {
    Ok(format!(
        "{}?o={offset}&n=50&sort=recent",
        api(input, "posts")?
    ))
}
fn hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn ext(value: &str) -> bool {
    !value.is_empty() && value.len() <= 12 && value.bytes().all(|b| b.is_ascii_alphanumeric())
}
fn media_parts(url: &Url) -> Result<(String, String, bool)> {
    if !secure(url) || url.query().is_some() {
        bail!("Petición de medio insegura")
    }
    let p = url.path().split('/').collect::<Vec<_>>();
    if p.len() != 4 || !p[0].is_empty() || !hash(p[2]) {
        bail!("Ruta de medio no válida")
    }
    match url.host_str() {
        Some("cdn.cum.st") if p[1] == "media" => {
            let e = p[3]
                .strip_prefix("original.")
                .filter(|e| ext(e))
                .ok_or_else(|| anyhow::anyhow!("Se requiere una variante original"))?;
            Ok((p[2].into(), e.into(), false))
        }
        Some("img.cum.st") if p[1] == "thumbnail" && p[3] == "preview.webp" => {
            Ok((p[2].into(), "webp".into(), true))
        }
        _ => bail!("Servidor de medios no admitido"),
    }
}
pub fn validate_media(url: &Url, input: &str) -> Result<()> {
    normalize(input)?;
    media_parts(url)?;
    Ok(())
}
pub fn validate_request(url: &Url, input: &str) -> Result<()> {
    normalize(input)?;
    if !secure(url) {
        bail!("Petición insegura")
    }
    if url.host_str() == Some("cum.st") {
        if url.as_str() == api(input, "profile")? {
            return Ok(());
        }
        let mut plain = url.clone();
        plain.set_query(None);
        let q = url.query_pairs().collect::<HashMap<_, _>>();
        if plain.as_str() == api(input, "posts")?
            && q.len() == 3
            && url.query_pairs().count() == 3
            && q.get("n").is_some_and(|v| v == "50")
            && q.get("sort").is_some_and(|v| v == "recent")
            && q.get("o").is_some_and(|v| {
                v.parse::<usize>()
                    .is_ok_and(|n| n <= 5_000_000 && n % 50 == 0)
            })
        {
            return Ok(());
        }
        bail!("La API no corresponde al creador o contiene filtros")
    }
    validate_media(url, input)
}
pub fn validate_redirect(url: &Url, original: &Url, input: &str) -> Result<()> {
    validate_request(original, input)?;
    if url != original {
        bail!("La redirección cambia el recurso o su calidad")
    };
    Ok(())
}
pub fn source_identity(file: &File) -> Result<String> {
    normalize(&file.post)?;
    let (h, e, preview) = media_parts(&Url::parse(&file.url)?)?;
    if preview {
        if file.id != format!("thumbnail:{h}") || !file.sha256.is_empty() {
            bail!("Identidad de miniatura no válida")
        };
        return Ok(format!("cum:thumbnail:{h}"));
    }
    if file.id != format!("{h}.{e}") || file.sha256 != h || file.expected_size.is_none() {
        bail!("Identidad de original no válida")
    }
    Ok(format!("cum:original:{h}:{e}:{:?}", file.expected_size))
}
fn preview(input: &str, title: &str, h: &str) -> File {
    File {
        id: format!("thumbnail:{h}"),
        post: input.into(),
        url: format!("https://img.cum.st/thumbnail/{h}/preview.webp"),
        path: format!("{title}/{h}.thumbnail.webp"),
        ..Default::default()
    }
}
fn attachment(a: &Value, input: &str, title: &str) -> Result<File> {
    let h = a["sha256"]
        .as_str()
        .filter(|h| hash(h))
        .ok_or_else(|| anyhow::anyhow!("Adjunto sin SHA-256 válido"))?;
    if a.get("storageKey").is_some_and(|s| s.as_str() != Some(h)) {
        return Err(IntegrityConflict("La clave de almacenamiento no coincide con el hash").into());
    }
    let variants = a["variants"]
        .as_array()
        .ok_or_else(|| anyhow::anyhow!("Faltan variantes originales; no se usan collages"))?;
    let originals = variants
        .iter()
        .filter_map(|v| {
            v["name"].as_str().and_then(|name| {
                name.strip_prefix("original.")
                    .filter(|e| ext(e))
                    .map(|e| (v, e))
            })
        })
        .collect::<Vec<_>>();
    let mut declarations = HashMap::new();
    for (v, e) in &originals {
        if declarations
            .insert(*e, &v["bytes"])
            .is_some_and(|before| before != &v["bytes"])
        {
            return Err(IntegrityConflict("Variantes originales contradictorias").into());
        }
    }
    let preferred = a["originalFilename"]
        .as_str()
        .and_then(|s| s.rsplit_once('.').map(|(_, e)| e));
    let selected = originals
        .iter()
        .find(|(_, e)| Some(*e) == preferred)
        .or_else(|| {
            if originals.len() == 1 {
                originals.first()
            } else {
                None
            }
        })
        .ok_or_else(|| anyhow::anyhow!("No hay una variante original inequívoca"))?;
    let bytes = selected.0["bytes"]
        .as_u64()
        .filter(|n| *n <= i64::MAX as u64)
        .ok_or_else(|| anyhow::anyhow!("Falta tamaño del original"))?;
    if a["bytes"].as_u64().is_some_and(|n| n != bytes) {
        return Err(IntegrityConflict("Tamaños originales contradictorios").into());
    }
    let e = selected.1;
    let id = format!("{h}.{e}");
    Ok(File {
        id: id.clone(),
        post: input.into(),
        url: format!("https://cdn.cum.st/media/{h}/original.{e}"),
        path: format!("{title}/{id}"),
        expected_size: Some(bytes),
        sha256: h.into(),
        fallback: Some(Box::new(preview(input, title, h))),
        ..Default::default()
    })
}
#[derive(Serialize, Deserialize)]
struct Snapshot {
    version: u8,
    input: String,
    posts: Vec<String>,
    observed_count: usize,
    listing: Listing,
}
fn valid(snapshot: &Snapshot, input: &str) -> Result<()> {
    if snapshot.version != 1
        || snapshot.input != input
        || snapshot.observed_count > snapshot.posts.len()
        || safe_name(&snapshot.listing.title) != snapshot.listing.title
    {
        bail!("Catálogo guardado no válido")
    }
    let mut posts = HashSet::new();
    for id in &snapshot.posts {
        if id.is_empty() || !posts.insert(id) {
            bail!("Publicaciones guardadas repetidas")
        }
    }
    let mut files = HashSet::new();
    for f in &snapshot.listing.files {
        source_identity(f)?;
        if f.post != input
            || f.id.starts_with("thumbnail:")
            || f.path != format!("{}/{}", snapshot.listing.title, f.id)
            || !files.insert(f.id.clone())
        {
            bail!("Destino o identidad guardados no válidos")
        }
        if let Some(backup) = &f.fallback {
            source_identity(backup)?;
            let expected = preview(input, &snapshot.listing.title, &f.sha256);
            if backup.fallback.is_some()
                || backup.post != input
                || backup.id != expected.id
                || backup.url != expected.url
                || backup.path != expected.path
                || backup.expected_size.is_some()
            {
                bail!("Respaldo guardado no válido")
            }
        }
    }
    Ok(())
}
pub async fn resolve<F, Fut>(
    state: &State,
    input: &str,
    refresh: bool,
    incremental: bool,
    cancel: &Cancel,
    fetch: &F,
) -> Result<Listing>
where
    F: Fn(String) -> Fut,
    Fut: Future<Output = Result<Value>>,
{
    let input = normalize(input)?;
    cancel.check()?;
    let old = state
        .post_snapshot(&input)?
        .map(|v| -> Result<Snapshot> {
            if v.len() != 1 {
                bail!("Catálogo guardado incompleto")
            };
            let s = serde_json::from_str(&v[0])?;
            valid(&s, &input)?;
            Ok(s)
        })
        .transpose();
    let old = if refresh { old.unwrap_or(None) } else { old? };
    if !refresh
        && !incremental
        && let Some(s) = &old
        && s.listing.pending.is_empty()
    {
        state.save_listing(&input, &s.listing)?;
        return Ok(s.listing.clone());
    }
    let (service, user) = identity(&input)?;
    let profile = fetch(api(&input, "profile")?).await?;
    cancel.check()?;
    if profile["id"].as_str() != Some(&user) || profile["service"].as_str() != Some(&service) {
        bail!("La API devolvió otro creador")
    }
    let name = profile["name"]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("Perfil sin nombre"))?;
    let title = old
        .as_ref()
        .map(|s| s.listing.title.clone())
        .unwrap_or_else(|| {
            format!(
                "{} [{service}-{user}]",
                safe_name(name)
                    .chars()
                    .take(24.min(80 - service.len() - user.len()))
                    .collect::<String>()
            )
        });
    let first = fetch(page_url(&input, 0)?).await?;
    cancel.check()?;
    let total = first["total"]
        .as_u64()
        .filter(|n| *n <= 5_000_000)
        .ok_or_else(|| anyhow::anyhow!("Total de publicaciones no válido"))?
        as usize;
    let use_frontier =
        incremental && !refresh && old.as_ref().is_some_and(|s| s.listing.pending.is_empty());
    let known = old
        .as_ref()
        .map(|s| s.posts.iter().cloned().collect::<HashSet<_>>())
        .unwrap_or_default();
    let required = total.saturating_sub(old.as_ref().map_or(0, |s| s.observed_count));
    let mut posts = Vec::new();
    let mut seen = HashSet::new();
    let mut files = HashMap::<String, File>::new();
    let mut order = Vec::new();
    let mut pending = Vec::new();
    let mut new = 0;
    let mut last = None;
    let mut stopped = false;
    let mut ordered = true;
    for offset in (0..total.max(1)).step_by(50) {
        cancel.check()?;
        let page = if offset == 0 {
            first.clone()
        } else {
            fetch(page_url(&input, offset)?).await?
        };
        cancel.check()?;
        if page["total"].as_u64() != Some(total as u64) {
            bail!("El catálogo cambió durante la consulta")
        }
        let batch = page["posts"]
            .as_array()
            .filter(|p| p.len() == 50.min(total - offset))
            .ok_or_else(|| {
                anyhow::anyhow!("Página incompleta; se conserva el catálogo anterior")
            })?;
        let mut all_known = true;
        for p in batch {
            if p["service"].as_str() != Some(&service)
                || p.get("user").is_some_and(|v| v.as_str() != Some(&user))
            {
                bail!("Publicación de otro creador o servicio")
            }
            let id = p["id"]
                .as_str()
                .filter(|s| {
                    !s.is_empty()
                        && s.len() <= 128
                        && s.bytes()
                            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
                })
                .ok_or_else(|| anyhow::anyhow!("Publicación sin ID válido"))?;
            if !seen.insert(id.to_owned()) {
                bail!("Publicaciones repetidas entre páginas")
            }
            if !known.contains(id) {
                all_known = false;
                new += 1
            }
            posts.push(id.to_owned());
            match p["published"].as_i64() {
                Some(n) => {
                    if last.is_some_and(|before| n > before) {
                        bail!("La página no respeta el orden reciente")
                    };
                    last = Some(n)
                }
                None => ordered = false,
            }
            let assets = p["attachments"]
                .as_array()
                .filter(|a| a.len() <= 10000)
                .ok_or_else(|| anyhow::anyhow!("Adjuntos incompletos"))?;
            for a in assets {
                match attachment(a, &input, &title) {
                    Ok(f) => {
                        if let Some(previous) = files.get(&f.id) {
                            if previous.url != f.url
                                || previous.expected_size != f.expected_size
                                || previous.sha256 != f.sha256
                            {
                                bail!("Originales contradictorios")
                            }
                        } else {
                            order.push(f.id.clone());
                            files.insert(f.id.clone(), f);
                        }
                    }
                    Err(e) if e.is::<IntegrityConflict>() => return Err(e),
                    Err(e) => pending.push(Pending {
                        url: format!("{input}/post/{id}"),
                        error: e.to_string(),
                    }),
                }
            }
        }
        if use_frontier && all_known && new >= required && ordered && pending.is_empty() {
            stopped = true;
            break;
        }
    }
    if !stopped && posts.len() != total {
        bail!("No se confirmó el catálogo completo")
    }
    if incremental
        && !refresh
        && let Some(old) = old
    {
        for id in old.posts {
            if seen.insert(id.clone()) {
                posts.push(id)
            }
        }
        for f in old.listing.files {
            if !files.contains_key(&f.id) {
                order.push(f.id.clone());
                files.insert(f.id.clone(), f);
            }
        }
    }
    let listing = Listing {
        title,
        files: order
            .into_iter()
            .map(|id| files.remove(&id).unwrap())
            .collect(),
        pending,
    };
    let snapshot = Snapshot {
        version: 1,
        input: input.clone(),
        posts,
        observed_count: total,
        listing,
    };
    valid(&snapshot, &input)?;
    cancel.check()?;
    state.save_post(&input, &[serde_json::to_string(&snapshot)?])?;
    state.save_listing(&input, &snapshot.listing)?;
    Ok(snapshot.listing)
}
