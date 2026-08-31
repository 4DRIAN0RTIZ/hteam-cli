//! Cliente HTTP standalone para el notebook público de SharePad que respalda
//! `hteam objectives show`. A diferencia de `HteamClient`, no lleva cookies
//! ni headers de autenticación de hteam.mx: es una fuente pública distinta,
//! de sólo lectura (nunca hace POST/PUT/PATCH/DELETE).

use anyhow::{Context, Result};
use scraper::{Html, Selector};
use std::time::Duration;

/// Hace un `GET` simple a `url` (el notebook de SharePad) y devuelve el
/// markdown crudo del notebook, ya extraído del HTML de la respuesta.
///
/// SharePad es una SPA de Next.js; el HTML servido en el GET inicial ya
/// trae el contenido pre-renderizado (SSR). Verificado contra una captura
/// real de `https://sharepad.in/n/okr-semanales` (2026-08-31, ver
/// "Ambigüedad para el reviewer" en
/// `progress/impl_weekly_objectives_show.md`): dentro de un único
/// `<script>` con `self.__next_f.push([1,"..."])` viaja un payload RSC que,
/// tras des-escaparlo, contiene `"pages":[{...,"content":"<markdown>"}]`
/// con el notebook completo. Ese mismo chunk también serializa, mezclados,
/// varios pares `"content":"..."` que **no** son el del notebook (son los
/// `content` de los `<meta>` de SEO que Next.js emite en el mismo payload) —
/// por eso la extracción se acota al array `"pages"` (ver
/// `extract_markdown_from_rsc_payload`) en vez de matchear cualquier
/// `"content"` suelto. Si el array `pages` no aparece (cambio de formato del
/// lado de SharePad), se cae a parsear el `<div class="markdown-body">` ya
/// renderizado. Ambos caminos están probados con la misma captura real en
/// los tests de este módulo.
pub async fn fetch_weekly_objectives_markdown(url: &str) -> Result<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .context("Error al crear el cliente HTTP para SharePad")?;

    let response = client
        .get(url)
        .send()
        .await
        .with_context(|| format!("No se pudo conectar a {}", url))?;

    if !response.status().is_success() {
        anyhow::bail!(
            "SharePad respondió con error HTTP {} al pedir {}",
            response.status(),
            url
        );
    }

    let html = response
        .text()
        .await
        .context("No se pudo leer el cuerpo de la respuesta de SharePad")?;

    extract_markdown_from_html(&html)
}

/// Punto de entrada de extracción: intenta el payload RSC primero, y si no
/// matchea, cae al HTML ya renderizado.
fn extract_markdown_from_html(html: &str) -> Result<String> {
    if let Some(markdown) = extract_markdown_from_rsc_payload(html) {
        return Ok(markdown);
    }

    if let Some(markdown) = extract_markdown_from_rendered_html(html) {
        return Ok(markdown);
    }

    anyhow::bail!(
        "No se pudo extraer el contenido del notebook: el payload RSC y el HTML renderizado \
         no matchean el formato esperado por hteam-cli"
    )
}

/// Extrae el markdown embebido en los `self.__next_f.push([1,"..."])` del
/// HTML servido por Next.js. Cada `push` trae un fragmento de texto que, una
/// vez des-escapado un nivel, es JSON crudo con la forma
/// `{"notebook":{...},"pages":[{"id":...,"content":"# titulo\n## objetivo\n- [ ] tarea",...}]}`
/// — forma verificada byte a byte contra una captura real de sharepad.in
/// (2026-08-31), no sólo contra la descripción textual de la feature.
///
/// El mismo chunk RSC serializa, más arriba en el payload, varios `<meta
/// content="...">` de SEO (título, descripción, viewport, etc.) como pares
/// `"content":"..."` sueltos. Si se busca `"content"` en todo el chunk sin
/// acotar, esos matches se concatenan como basura *antes* del markdown real
/// (bug confirmado al parsear la captura real: 16 de 17 matches eran
/// metadatos, no el notebook). Por eso la búsqueda del campo `content` se
/// acota al texto que sigue a la primera aparición de `"pages":`, donde
/// SharePad sólo emite el `content` real de cada página del notebook.
///
/// Hay dos niveles de escaping a deshacer: el string del `push(...)` en sí
/// (nivel de string JS/JSON embebido en el HTML) y, dentro de él, el campo
/// `"content"` (que trae el markdown con sus saltos de línea escapados otra
/// vez). Cada nivel se deshace envolviendo el fragmento en comillas y
/// dejando que `serde_json` decodifique los escapes — es exactamente lo que
/// hace JSON con un string literal.
fn extract_markdown_from_rsc_payload(html: &str) -> Option<String> {
    let push_re = regex::Regex::new(r#"self\.__next_f\.push\(\[1,\s*"((?:\\.|[^"\\])*)"\]\)"#)
        .expect("regex de push RSC inválida");

    let decoded_chunks: String = push_re
        .captures_iter(html)
        .filter_map(|caps| unescape_json_string(&caps[1]))
        .collect();

    if decoded_chunks.is_empty() {
        return None;
    }

    // Acotar al array "pages": el mismo chunk trae también los `content` de
    // metadatos <meta> de SEO, que no son el markdown del notebook (ver
    // doc-comment de esta función).
    let pages_start = decoded_chunks.find("\"pages\":")?;
    let scoped = &decoded_chunks[pages_start..];

    let content_re = regex::Regex::new(r#""content"\s*:\s*"((?:\\.|[^"\\])*)""#)
        .expect("regex de content inválida");

    let markdown_sections: Vec<String> = content_re
        .captures_iter(scoped)
        .filter_map(|caps| unescape_json_string(&caps[1]))
        .filter(|s| !s.trim().is_empty())
        .collect();

    if markdown_sections.is_empty() {
        return None;
    }

    Some(markdown_sections.join("\n\n"))
}

/// Deshace un nivel de escaping estilo JSON (`\"`, `\\`, `\n`, `\uXXXX`, ...)
/// envolviendo el fragmento en comillas y reusando el parser de strings de
/// `serde_json`, en vez de reimplementar las reglas de escape a mano.
fn unescape_json_string(raw: &str) -> Option<String> {
    let wrapped = format!("\"{}\"", raw);
    serde_json::from_str::<String>(&wrapped).ok()
}

/// Fallback cuando el payload RSC no matchea: reconstruye el mismo markdown
/// sintético (`# `, `## `, `- [ ] `/`- [x] `) a partir del
/// `<div class="markdown-body">` ya renderizado por SharePad, para poder
/// reusar el mismo parser de markdown de `operations::objectives` sin
/// duplicar la lógica de dominio.
fn extract_markdown_from_rendered_html(html: &str) -> Option<String> {
    let document = Html::parse_document(html);
    let container_sel = Selector::parse(".markdown-body").ok()?;
    let container = document.select(&container_sel).next()?;

    let node_sel = Selector::parse("h1, h2, li").ok()?;
    let checkbox_sel = Selector::parse(r#"input[type="checkbox"]"#).ok()?;

    let mut lines: Vec<String> = Vec::new();

    for node in container.select(&node_sel) {
        let tag = node.value().name();
        match tag {
            "h1" => lines.push(format!("# {}", collect_text(&node))),
            "h2" => lines.push(format!("## {}", collect_text(&node))),
            "li" => {
                let is_task = node.select(&checkbox_sel).next();
                let text = collect_text(&node);
                match is_task {
                    Some(checkbox) => {
                        let checked = checkbox.value().attr("checked").is_some();
                        let mark = if checked { "x" } else { " " };
                        lines.push(format!("- [{}] {}", mark, text));
                    }
                    None => lines.push(format!("- {}", text)),
                }
            }
            _ => {}
        }
    }

    if lines.is_empty() {
        return None;
    }

    Some(lines.join("\n"))
}

fn collect_text(node: &scraper::ElementRef) -> String {
    node.text()
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    // Fixture sintético (no captura real) usado sólo para ejercitar casos
    // que la captura real de más abajo no cubre hoy: tareas ya marcadas
    // (`- [x]`) y el camino en el que `extract_markdown_from_rsc_payload`
    // no encuentra ningún `"pages":`. La cobertura contra el sitio real
    // vive en `real_sharepad_fixture_html()` y sus tests.
    fn rsc_fixture_html() -> String {
        r##"<!DOCTYPE html><html><head></head><body>
<div id="__next"></div>
<script>self.__next_f.push([1,"a:I[123,[],\"App\"]\n"])</script>
<script>self.__next_f.push([1,"3:{\"pages\":[{\"id\":1,\"content\":\"# Objetivos semanales\\n\\n## Demo CCL\\n- [ ] Tarea 1 xd\\n- [x] Tarea 2 lista\\n\\n## Otro objetivo\\n- [ ] Tarea 3\\n\"}]}"])</script>
</body></html>"##
            .to_string()
    }

    // Fixture sintético (no captura real), sólo para el caso de tareas ya
    // marcadas (`checked`), que la captura real no tiene. Ver
    // `real_sharepad_fixture_html()` para la cobertura contra el sitio real.
    fn rendered_fixture_html() -> String {
        r#"<!DOCTYPE html><html><body>
<div class="markdown-body">
<h1>Objetivos semanales</h1>
<h2>Demo CCL</h2>
<ul class="contains-task-list">
<li class="task-list-item"><input type="checkbox" disabled> Tarea 1 xd</li>
<li class="task-list-item"><input type="checkbox" checked disabled> Tarea 2 lista</li>
</ul>
<h2>Otro objetivo</h2>
<ul class="contains-task-list">
<li class="task-list-item"><input type="checkbox" disabled> Tarea 3</li>
</ul>
</div>
</body></html>"#
            .to_string()
    }

    #[test]
    fn extract_markdown_from_rsc_payload_reads_pages_content() {
        let markdown = extract_markdown_from_rsc_payload(&rsc_fixture_html())
            .expect("debería extraer el markdown del payload RSC");

        assert!(markdown.contains("# Objetivos semanales"));
        assert!(markdown.contains("## Demo CCL"));
        assert!(markdown.contains("- [ ] Tarea 1 xd"));
        assert!(markdown.contains("- [x] Tarea 2 lista"));
        assert!(markdown.contains("## Otro objetivo"));
    }

    #[test]
    fn extract_markdown_from_rsc_payload_returns_none_without_next_f_push() {
        let html = "<html><body><p>no hay payload acá</p></body></html>";

        assert_eq!(extract_markdown_from_rsc_payload(html), None);
    }

    #[test]
    fn extract_markdown_from_rendered_html_rebuilds_synthetic_markdown() {
        let markdown = extract_markdown_from_rendered_html(&rendered_fixture_html())
            .expect("debería reconstruir el markdown desde el HTML renderizado");

        assert!(markdown.contains("# Objetivos semanales"));
        assert!(markdown.contains("## Demo CCL"));
        assert!(markdown.contains("- [ ] Tarea 1 xd"));
        assert!(markdown.contains("- [x] Tarea 2 lista"));
    }

    #[test]
    fn extract_markdown_from_html_falls_back_to_rendered_when_rsc_missing() {
        let markdown = extract_markdown_from_html(&rendered_fixture_html())
            .expect("debería caer al fallback de HTML renderizado");

        assert!(markdown.contains("# Objetivos semanales"));
    }

    #[test]
    fn extract_markdown_from_html_fails_clearly_when_nothing_matches() {
        let html = "<html><body><p>página sin objetivos</p></body></html>";

        let err = extract_markdown_from_html(html).expect_err("debería fallar");

        assert!(err.to_string().contains("No se pudo extraer el contenido"));
    }

    #[tokio::test]
    async fn fetch_weekly_objectives_markdown_parses_mocked_html() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/n/okr-semanales")
            .with_status(200)
            .with_body(rsc_fixture_html())
            .create_async()
            .await;

        let url = format!("{}/n/okr-semanales", server.url());
        let markdown = fetch_weekly_objectives_markdown(&url)
            .await
            .expect("debería obtener y parsear el markdown");

        assert!(markdown.contains("# Objetivos semanales"));
        assert!(markdown.contains("- [x] Tarea 2 lista"));
    }

    #[tokio::test]
    async fn fetch_weekly_objectives_markdown_fails_on_http_error() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/n/okr-semanales")
            .with_status(404)
            .create_async()
            .await;

        let url = format!("{}/n/okr-semanales", server.url());
        let err = fetch_weekly_objectives_markdown(&url)
            .await
            .expect_err("debería fallar con 404");

        assert!(err.to_string().contains("404"));
    }

    #[tokio::test]
    async fn fetch_weekly_objectives_markdown_fails_on_unrecognized_payload() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/n/okr-semanales")
            .with_status(200)
            .with_body("<html><body><p>sin objetivos</p></body></html>")
            .create_async()
            .await;

        let url = format!("{}/n/okr-semanales", server.url());
        let err = fetch_weekly_objectives_markdown(&url)
            .await
            .expect_err("debería fallar con payload no reconocido");

        assert!(err.to_string().contains("No se pudo extraer el contenido"));
    }

    /// Captura verbatim (byte a byte, sin editar nada a mano) de
    /// `curl -sL https://sharepad.in/n/okr-semanales`, hecha el 2026-08-31
    /// (`HTTP_CODE:200`, 32556 bytes). Ver "Ambigüedad para el
    /// reviewer" en `progress/impl_weekly_objectives_show.md` para el
    /// detalle de lo que se verificó con esta captura real.
    fn real_sharepad_fixture_html() -> String {
        r##"<!DOCTYPE html><html lang="en" class="kalam_d4e22b50-module__bDco2q__variable architects_daughter_1181376-module__9FPgVW__variable source_serif_4_92371bb3-module__DWOoaG__variable inter_8db6fa51-module__MMaAbG__variable jetbrains_mono_9a2f2d6c-module__wsyXyG__variable h-full antialiased"><head><meta charSet="utf-8"/><meta name="viewport" content="width=device-width, initial-scale=1"/><link rel="stylesheet" href="/_next/static/immutable/chunks/0_a4irsykw_52.css" data-precedence="next"/><link rel="stylesheet" href="/_next/static/immutable/chunks/0klya-7dk1nq1.css" data-precedence="next"/><link rel="preload" as="script" fetchPriority="low" href="/_next/static/immutable/chunks/0o-76-3ck2_s9.js"/><script src="/_next/static/immutable/chunks/3hkepvwx0x6ez.js" async=""></script><script src="/_next/static/immutable/chunks/3rt__sc8jei53.js" async=""></script><script src="/_next/static/immutable/chunks/0j5gbf7rs3uw7.js" async=""></script><script src="/_next/static/immutable/chunks/turbopack-1xgp9ham97o16.js" async=""></script><script src="/_next/static/immutable/chunks/1re8l28ybwau4.js" async="" crossorigin=""></script><script src="/_next/static/immutable/chunks/42ib0lv26jz3v.js" async="" crossorigin=""></script><script src="/_next/static/immutable/chunks/2dgus6xh6x26g.js" async="" crossorigin=""></script><script src="/_next/static/immutable/chunks/2z7u4ewi6d7kj.js" async="" crossorigin=""></script><script src="/_next/static/immutable/chunks/2smipw1uu9qsx.js" async=""></script><script src="/_next/static/immutable/chunks/0r30p2j3i-crd.js" async=""></script><script src="/_next/static/immutable/chunks/376jf3zrtq-9q.js" async=""></script><script src="/_next/static/immutable/chunks/15hlxfei78flc.js" async=""></script><meta name="next-size-adjust" content=""/><title>Okr Semanales — SharePad</title><meta name="description" content="A shared notebook: Okr Semanales"/><meta name="application-name" content="SharePad"/><link rel="author" href="https://github.com/Varshithvhegde"/><meta name="author" content="Varshith Hegde"/><meta name="keywords" content="share notes,markdown notebook,no signup notepad,share markdown link,online notepad,paste and share text,temporary notes,markdown to pdf"/><meta name="creator" content="Varshith Hegde"/><meta name="robots" content="noindex, nofollow"/><meta name="category" content="productivity"/><link rel="canonical" href="https://sharepad.in/n/okr-semanales"/><meta property="og:title" content="Okr Semanales"/><meta property="og:description" content="A shared notebook: Okr Semanales"/><meta property="og:url" content="https://sharepad.in/n/okr-semanales"/><meta property="og:type" content="article"/><meta name="twitter:card" content="summary_large_image"/><meta name="twitter:title" content="Okr Semanales"/><meta name="twitter:description" content="A shared notebook: Okr Semanales"/><link rel="icon" href="/favicon.ico?favicon.261-8iwo0k35b.ico" sizes="256x256" type="image/x-icon"/><link rel="icon" href="/favicon.ico" sizes="48x48"/><link rel="icon" href="/icon.svg" type="image/svg+xml"/><link rel="apple-touch-icon" href="/apple-icon.png"/><script src="/_next/static/immutable/chunks/0c0hxoamwjsbw.js" noModule=""></script></head><body class="min-h-full"><div hidden=""><!--$--><!--/$--></div><a href="#main" class="skip-link">Skip to content</a><!--$--><!--/$--><div class="h-dvh flex flex-col overflow-hidden paper-dot"><div class="fixed bottom-5 right-5 z-[100] flex flex-col gap-2.5 pointer-events-none" role="status" aria-live="polite"></div><header class="relative z-40 flex items-center gap-2 px-3 sm:px-4 h-14 shrink-0 no-print" style="border-bottom:1.5px solid rgba(28,28,28,0.16);background:rgba(250,249,246,0.94);backdrop-filter:blur(6px)"><button class="btn-ghost lg:hidden !px-2" aria-label="Show pages"><svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="lucide lucide-menu" aria-hidden="true"><path d="M4 5h16"></path><path d="M4 12h16"></path><path d="M4 19h16"></path></svg></button><a class="hidden sm:block shrink-0 text-[1.15rem]" style="font-family:var(--font-sketch), serif" href="/">SharePad</a><span class="hidden sm:block h-5 w-px shrink-0" style="background:rgba(28,28,28,0.18)"></span><div class="flex-1 min-w-0 flex items-center gap-2"><svg xmlns="http://www.w3.org/2000/svg" width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" class="lucide lucide-notebook shrink-0" aria-hidden="true"><path d="M2 6h4"></path><path d="M2 10h4"></path><path d="M2 14h4"></path><path d="M2 18h4"></path><rect width="16" height="20" x="4" y="2" rx="2"></rect><path d="M16 2v20"></path></svg><span class="truncate text-[1rem]">Okr Semanales</span></div><div class="flex items-center gap-1 shrink-0"><span class="hidden sm:flex items-center text-[0.8rem] w-16 justify-end" style="color:var(--ink-3)"></span><div class="relative"><span class="relative inline-flex group"><button class="btn-ghost" aria-label="Download" aria-expanded="false" aria-haspopup="menu"><svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="lucide lucide-download" aria-hidden="true"><path d="M12 15V3"></path><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path><path d="m7 10 5 5 5-5"></path></svg></button><span aria-hidden="true" class="pointer-events-none absolute top-full mt-2 z-[60] flex items-center gap-1.5 whitespace-nowrap px-2 py-1 text-[0.78rem] opacity-0 transition-opacity duration-150 group-hover:opacity-100 group-focus-within:opacity-100 right-0" style="background:var(--ink);color:var(--paper);box-shadow:2px 2px 0 rgba(28,28,28,0.25)">Download</span></span></div></div></header><div class="flex flex-1 overflow-hidden"><aside class="hidden lg:relative lg:flex lg:z-auto shrink-0 no-print"><div class="relative flex flex-col h-full w-[min(18rem,82vw)] lg:w-72 paper-plain" style="border-right:1.5px solid rgba(28,28,28,0.16)"><div class="p-3 flex items-center gap-2" style="border-bottom:1.5px solid rgba(28,28,28,0.12)"><div class="relative flex-1"><svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="lucide lucide-search absolute left-2.5 top-1/2 -translate-y-1/2" aria-hidden="true" style="color:var(--ink-3)"><path d="m21 21-4.34-4.34"></path><circle cx="11" cy="11" r="8"></circle></svg><input placeholder="Find a page" aria-label="Search pages" class="field !py-1.5 !pl-8 text-[0.86rem]" value=""/></div><button class="btn-ghost lg:hidden !px-2" aria-label="Close"><svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="lucide lucide-x" aria-hidden="true"><path d="M18 6 6 18"></path><path d="m6 6 12 12"></path></svg></button></div><div class="flex-1 overflow-y-auto p-2.5 space-y-2"><div class="sk group sn-y" style="transform:rotate(0deg);box-shadow:4px 4px 0 rgba(28,28,28,0.18);transition:transform 0.15s, box-shadow 0.15s"><div class="sk-b" style="border-width:1.8px;opacity:1"></div><div class="sk-i"><button class="w-full flex items-center gap-2 px-3 py-2.5 text-left text-[0.92rem]"><svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" class="lucide lucide-file-text shrink-0" aria-hidden="true"><path d="M6 22a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h8a2.4 2.4 0 0 1 1.704.706l3.588 3.588A2.4 2.4 0 0 1 20 8v12a2 2 0 0 1-2 2z"></path><path d="M14 2v5a1 1 0 0 0 1 1h5"></path><path d="M10 9H8"></path><path d="M16 13H8"></path><path d="M16 17H8"></path></svg><span class="truncate flex-1">Objetivos semanales</span></button></div></div></div></div></aside><main class="flex-1 flex flex-col overflow-hidden min-w-0"><div class="flex items-center gap-2 px-3 sm:px-4 py-2.5 shrink-0 no-print" style="border-bottom:1.5px solid rgba(28,28,28,0.12)"><h1 class="flex-1 truncate text-[1.3rem] flex items-center gap-2" style="font-family:var(--font-sketch), serif"><svg xmlns="http://www.w3.org/2000/svg" width="17" height="17" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" class="lucide lucide-file-text shrink-0" aria-hidden="true"><path d="M6 22a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h8a2.4 2.4 0 0 1 1.704.706l3.588 3.588A2.4 2.4 0 0 1 20 8v12a2 2 0 0 1-2 2z"></path><path d="M14 2v5a1 1 0 0 0 1 1h5"></path><path d="M10 9H8"></path><path d="M16 13H8"></path><path d="M16 17H8"></path></svg><span class="truncate">Objetivos semanales</span></h1><span class="hidden md:inline text-[0.78rem] shrink-0" style="color:var(--ink-3)">30<!-- --> words · <!-- -->1<!-- --> min read</span><div class="flex items-center gap-0.5 shrink-0"><span class="relative inline-flex group"><button class="btn-ghost !px-1.5" aria-label="Copy markdown"><svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="lucide lucide-copy" aria-hidden="true"><rect width="14" height="14" x="8" y="8" rx="2" ry="2"></rect><path d="M4 16c-1.1 0-2-.9-2-2V4c0-1.1.9-2 2-2h10c1.1 0 2 .9 2 2"></path></svg></button><span aria-hidden="true" class="pointer-events-none absolute top-full mt-2 z-[60] flex items-center gap-1.5 whitespace-nowrap px-2 py-1 text-[0.78rem] opacity-0 transition-opacity duration-150 group-hover:opacity-100 group-focus-within:opacity-100 left-1/2 -translate-x-1/2" style="background:var(--ink);color:var(--paper);box-shadow:2px 2px 0 rgba(28,28,28,0.25)">Copy markdown</span></span><span class="relative inline-flex group"><button class="btn-ghost !px-1.5" aria-label="Copy link to this page"><svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="lucide lucide-link" aria-hidden="true"><path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"></path><path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"></path></svg></button><span aria-hidden="true" class="pointer-events-none absolute top-full mt-2 z-[60] flex items-center gap-1.5 whitespace-nowrap px-2 py-1 text-[0.78rem] opacity-0 transition-opacity duration-150 group-hover:opacity-100 group-focus-within:opacity-100 left-1/2 -translate-x-1/2" style="background:var(--ink);color:var(--paper);box-shadow:2px 2px 0 rgba(28,28,28,0.25)">Copy link to this page</span></span><span class="relative inline-flex group"><a href="/n/okr-semanales/print" target="_blank" rel="noopener noreferrer" class="btn-ghost !px-1.5" aria-label="Print or save as PDF"><svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="lucide lucide-printer" aria-hidden="true"><path d="M6 18H4a2 2 0 0 1-2-2v-5a2 2 0 0 1 2-2h16a2 2 0 0 1 2 2v5a2 2 0 0 1-2 2h-2"></path><path d="M6 9V3a1 1 0 0 1 1-1h10a1 1 0 0 1 1 1v6"></path><rect x="6" y="14" width="12" height="8" rx="1"></rect></svg></a><span aria-hidden="true" class="pointer-events-none absolute top-full mt-2 z-[60] flex items-center gap-1.5 whitespace-nowrap px-2 py-1 text-[0.78rem] opacity-0 transition-opacity duration-150 group-hover:opacity-100 group-focus-within:opacity-100 right-0" style="background:var(--ink);color:var(--paper);box-shadow:2px 2px 0 rgba(28,28,28,0.25)">Print or save as PDF</span></span></div></div><input type="file" accept=".md,.markdown,.txt" class="hidden"/><input type="file" accept="image/png,image/jpeg,image/webp,image/gif,image/avif" multiple="" class="hidden"/><div class="flex flex-1 overflow-hidden"><div class="flex-1 overflow-y-auto paper-plain font-mono-doc"><div class="margin-rule min-h-full mx-auto w-full max-w-3xl"><div class="pl-12 pr-5 sm:pl-16 sm:pr-10 py-8"><p class="text-[0.8rem] mb-6" style="color:var(--ink-3)">55<!-- --> <!-- -->views<!-- --> · updated 31 Aug 2026</p><div class="markdown-body "><h1 id="objetivos-semanales" node="[object Object]">Objetivos semanales</h1>
<h2 id="demo-ccl" node="[object Object]">Demo CCL</h2>
<ul class="contains-task-list">
<li class="task-list-item"><input type="checkbox" disabled=""/> Tarea 1 xd</li>
</ul>
<h2 id="presentacion-prototipos" node="[object Object]">Presentacion prototipos</h2>
<ul class="contains-task-list">
<li class="task-list-item"><input type="checkbox" disabled=""/> Tarea 2? xd</li>
</ul>
<h2 id="validar-ccl" node="[object Object]">Validar CCL</h2>
<ul class="contains-task-list">
<li class="task-list-item"><input type="checkbox" disabled=""/> Pos la 3</li>
</ul></div><div class="mt-12 pt-6 space-y-10" style="border-top:1.5px dashed var(--rule)"><nav aria-label="On this page"><h2 class="text-[1.05rem] mb-2" style="font-family:var(--font-sketch), serif">On this page</h2><ul class="space-y-1"><li style="padding-left:0"><a href="#objetivos-semanales" class="text-[0.9rem] hover:underline" style="color:var(--ink-2)">Objetivos semanales</a></li><li style="padding-left:14px"><a href="#demo-ccl" class="text-[0.9rem] hover:underline" style="color:var(--ink-2)">Demo CCL</a></li><li style="padding-left:14px"><a href="#presentacion-prototipos" class="text-[0.9rem] hover:underline" style="color:var(--ink-2)">Presentacion prototipos</a></li><li style="padding-left:14px"><a href="#validar-ccl" class="text-[0.9rem] hover:underline" style="color:var(--ink-2)">Validar CCL</a></li></ul></nav><section aria-label="Comments"><h2 class="text-[1.05rem] mb-3" style="font-family:var(--font-sketch), serif">Comments </h2><div class="py-4" style="color:var(--ink-3)"><svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="lucide lucide-loader-circle animate-spin" aria-hidden="true"><path d="M21 12a9 9 0 1 1-6.219-8.56"></path></svg></div><form class="space-y-2"><input placeholder="Your name (optional)" aria-label="Your name" class="field text-[0.9rem] !py-2" value=""/><div class="flex gap-2"><input placeholder="Add a comment" aria-label="Comment" class="field text-[0.9rem] !py-2" value=""/><button type="submit" disabled="" class="btn btn-b !px-3 shrink-0"><svg xmlns="http://www.w3.org/2000/svg" width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" class="lucide lucide-send" aria-hidden="true"><path d="M14.536 21.686a.5.5 0 0 0 .937-.024l6.5-19a.496.496 0 0 0-.635-.635l-19 6.5a.5.5 0 0 0-.024.937l7.93 3.18a2 2 0 0 1 1.112 1.11z"></path><path d="m21.854 2.147-10.94 10.939"></path></svg></button></div></form></section></div></div></div></div></div></main></div></div><!--$--><!--/$--><script type="application/ld+json">{"@context":"https://schema.org","@type":"WebApplication","name":"SharePad","url":"https://sharepad.in","description":"Write a notebook of markdown pages and share it with a single link. Password lock, expiry dates, comments and PDF export. Free, and no account needed.","applicationCategory":"ProductivityApplication","operatingSystem":"Any","browserRequirements":"Requires JavaScript","offers":{"@type":"Offer","price":"0","priceCurrency":"USD"},"featureList":["Multi-page markdown notebooks behind one link","No account or sign-up","Password protection and expiry dates","Export to PDF or Markdown","Anonymous comments","Version history"],"author":{"@type":"Person","name":"Varshith Hegde"}}</script><script src="/_next/static/immutable/chunks/0o-76-3ck2_s9.js" id="_R_" async=""></script><script>(self.__next_f=self.__next_f||[]).push([0])</script><script>self.__next_f.push([1,"1:\"$Sreact.fragment\"\n2:\"$Sreact.suspense\"\n3:I[42203,[\"/_next/static/immutable/chunks/1re8l28ybwau4.js\"],\"default\"]\n4:I[39756,[\"/_next/static/immutable/chunks/1re8l28ybwau4.js\"],\"default\"]\n5:I[8821,[\"/_next/static/immutable/chunks/1re8l28ybwau4.js\",\"/_next/static/immutable/chunks/42ib0lv26jz3v.js\"],\"default\"]\n6:I[37457,[\"/_next/static/immutable/chunks/1re8l28ybwau4.js\"],\"default\"]\nf:I[53348,[\"/_next/static/immutable/chunks/1re8l28ybwau4.js\",\"/_next/static/immutable/chunks/2dgus6xh6x26g.js\"],\"default\"]\n:HL[\"/_next/static/immutable/chunks/0_a4irsykw_52.css\",\"style\"]\n:HL[\"/_next/static/immutable/media/0fcab32fcfb2da9d-s.p.1xuyavnr-8o1k.woff2\",\"font\",{\"crossOrigin\":\"\",\"type\":\"font/woff2\"}]\n:HL[\"/_next/static/immutable/media/68d403cf9f2c68c5-s.p.42000xkkqj0am.woff2\",\"font\",{\"crossOrigin\":\"\",\"type\":\"font/woff2\"}]\n:HL[\"/_next/static/immutable/media/70bc3e132a0a741e-s.p.269kn9uafm0ti.woff2\",\"font\",{\"crossOrigin\":\"\",\"type\":\"font/woff2\"}]\n:HL[\"/_next/static/immutable/media/83afe278b6a6bb3c-s.p.45535valc9rzk.woff2\",\"font\",{\"crossOrigin\":\"\",\"type\":\"font/woff2\"}]\n:HL[\"/_next/static/immutable/media/84780b0176be2d72-s.p.15k2e1n_c36n0.woff2\",\"font\",{\"crossOrigin\":\"\",\"type\":\"font/woff2\"}]\n:HL[\"/_next/static/immutable/media/95accdfe7438af8b-s.p.35__u1_5x50s9.woff2\",\"font\",{\"crossOrigin\":\"\",\"type\":\"font/woff2\"}]\n:HL[\"/_next/static/immutable/media/970c428219233a3d-s.p.3p08-1dmxavj4.woff2\",\"font\",{\"crossOrigin\":\"\",\"type\":\"font/woff2\"}]\n:HL[\"/_next/static/immutable/chunks/0klya-7dk1nq1.css\",\"style\"]\nd:X\n0:{\"P\":null,\"c\":[\"\",\"n\",\"okr-semanales\"],\"q\":\"\",\"i\":false,\"f\":[[[\"\",{\"children\":[\"n\",{\"children\":[[\"slug\",\"okr-semanales\",\"d\",[]],{\"children\":[\"__PAGE__\",{},\"$undefined\",\"$undefined\",4096]},\"$undefined\",\"$undefined\",4096]},\"$undefined\",\"$undefined\",4096]},\"$undefined\",\"$undefined\",4112],[[\"$\",\"$1\",\"c\",{\"children\":[[[\"$\",\"link\",\"0\",{\"rel\":\"stylesheet\",\"href\":\"/_next/static/immutable/chunks/0_a4irsykw_52.css\",\"precedence\":\"next\",\"crossOrigin\":\"$undefined\",\"nonce\":\"$undefined\"}],[\"$\",\"script\",\"script-0\",{\"src\":\"/_next/static/immutable/chunks/1re8l28ybwau4.js\",\"async\":true,\"nonce\":\"$undefined\"}]],[\"$\",\"html\",null,{\"lang\":\"en\",\"className\":\"kalam_d4e22b50-module__bDco2q__variable architects_daughter_1181376-module__9FPgVW__variable source_serif_4_92371bb3-module__DWOoaG__variable inter_8db6fa51-module__MMaAbG__variable jetbrains_mono_9a2f2d6c-module__wsyXyG__variable h-full antialiased\",\"children\":[\"$\",\"body\",null,{\"className\":\"min-h-full\",\"children\":[[\"$\",\"a\",null,{\"href\":\"#main\",\"className\":\"skip-link\",\"children\":\"Skip to content\"}],[\"$\",\"$2\",null,{\"fallback\":null,\"children\":[\"$\",\"$L3\",null,{}]}],[\"$\",\"$L4\",null,{\"parallelRouterKey\":\"children\",\"error\":\"$5\",\"errorStyles\":[],\"errorScripts\":[[\"$\",\"script\",\"script-0\",{\"src\":\"/_next/static/immutable/chunks/42ib0lv26jz3v.js\",\"async\":true}]],\"template\":[\"$\",\"$L6\",null,{}],\"templateStyles\":\"$undefined\",\"templateScripts\":\"$undefined\",\"notFound\":[[\"$\",\"main\",null,{\"className\":\"min-h-dvh paper-dot flex items-center justify-center px-5 py-14\",\"children\":[\"$\",\"div\",null,{\"className\":\"w-full max-w-lg\",\"children\":[[\"$\",\"div\",null,{\"className\":\"relative mx-auto w-full max-w-[19rem] note-enter\",\"children\":[[\"$\",\"span\",null,{\"className\":\"tape tape-o\",\"style\":{\"top\":-10,\"left\":\"50%\",\"transform\":\"translateX(-50%) rotate(-4deg)\",\"width\":74,\"height\":19}}],[\"$\",\"div\",null,{\"className\":\"sk\",\"style\":{\"transform\":\"rotate(-1.5deg)\"},\"children\":[[\"$\",\"div\",null,{\"className\":\"sk-b\"}],[\"$\",\"div\",null,{\"className\":\"sk-i\",\"children\":[\"$\",\"div\",null,{\"className\":\"margin-rule paper-ruled pt-8 pb-10 pl-14 pr-6\",\"style\":{\"clipPath\":\"polygon(0 0, 100% 0, 100% 86%, 92% 92%, 84% 85%, 75% 93%, 66% 86%, 57% 94%, 48% 87%, 39% 95%, 30% 88%, 21% 95%, 12% 88%, 4% 94%, 0 88%)\"},\"children\":[[\"$\",\"p\",null,{\"className\":\"text-[3.6rem] leading-none mb-2\",\"style\":{\"fontFamily\":\"var(--font-sketch), serif\",\"color\":\"var(--red)\"},\"children\":\"404\"}],[\"$\",\"svg\",null,{\"width\":\"150\",\"height\":\"12\",\"viewBox\":\"0 0 150 12\",\"preserveAspectRatio\":\"none\",\"aria-hidden\":true,\"children\":[\"$\",\"path\",null,{\"d\":\"M1,7 C26,2 50,10 74,6 C98,2 124,9 149,6\",\"stroke\":\"var(--ink)\",\"strokeWidth\":\"2\",\"fill\":\"none\",\"strokeLinecap\":\"round\"}]}],[\"$\",\"p\",null,{\"className\":\"mt-4 text-[0.95rem]\",\"style\":{\"color\":\"var(--ink-2)\"},\"children\":\"nothing here\"}]]}]}]]}]]}],[\"$\",\"div\",null,{\"className\":\"mt-10 text-center\",\"children\":[[\"$\",\"h1\",null,{\"className\":\"text-[1.9rem] leading-tight mb-2\",\"style\":{\"fontFamily\":\"var(--font-sketch), serif\"},\"children\":\"This page isn't in the notebook\"}],[\"$\",\"p\",null,{\"className\":\"text-[1rem] mb-8\",\"style\":{\"color\":\"var(--ink-2)\"},\"children\":\"Four of the usual suspects:\"}]]}],[\"$\",\"ul\",null,{\"className\":\"space-y-3 mb-10\",\"children\":[[\"$\",\"li\",\"It ran out of time. Notebooks delete themselves on a schedule.\",{\"className\":\"relative\",\"children\":[[\"$\",\"span\",null,{\"className\":\"tape tape-y\",\"style\":{\"top\":-7,\"left\":20,\"transform\":\"rotate(-3deg)\",\"width\":42,\"height\":13}}],[\"$\",\"div\",null,{\"className\":\"sk\",\"style\":{\"transform\":\"rotate(-0.4deg)\"},\"children\":[[\"$\",\"div\",null,{\"className\":\"sk-b\",\"style\":{\"borderWidth\":\"1.2px\"}}],[\"$\",\"div\",null,{\"className\":\"sk-i px-4 py-3 text-[0.94rem]\",\"children\":\"It ran out of time. Notebooks delete themselves on a schedule.\"}]]}]]}],[\"$\",\"li\",\"Someone deleted it on purpose.\",{\"className\":\"relative\",\"children\":[[\"$\",\"span\",null,{\"className\":\"tape tape-b\",\"style\":{\"top\":-7,\"left\":20,\"transform\":\"rotate(-3deg)\",\"width\":42,\"height\":13}}],[\"$\",\"div\",null,{\"className\":\"sk\",\"style\":{\"transform\":\"rotate(0.4deg)\"},\"children\":[[\"$\",\"div\",null,{\"className\":\"sk-b\",\"style\":{\"borderWidth\":\"1.2px\"}}],[\"$\",\"div\",null,{\"className\":\"sk-i px-4 py-3 text-[0.94rem]\",\"children\":\"Someone deleted it on purpose.\"}]]}]]}],[\"$\",\"li\",\"It was a read-once link and has already been opened.\",{\"className\":\"relative\",\"children\":[[\"$\",\"span\",null,{\"className\":\"tape tape-p\",\"style\":{\"top\":-7,\"left\":20,\"transform\":\"rotate(-3deg)\",\"width\":42,\"height\":13}}],[\"$\",\"div\",null,{\"className\":\"sk\",\"style\":{\"transform\":\"rotate(-0.4deg)\"},\"children\":[[\"$\",\"div\",null,{\"className\":\"sk-b\",\"style\":{\"borderWidth\":\"1.2px\"}}],[\"$\",\"div\",null,{\"className\":\"sk-i px-4 py-3 text-[0.94rem]\",\"children\":\"It was a read-once link and has already been opened.\"}]]}]]}],\"$L7\"]}],\"$L8\"]}]}],[]],\"forbidden\":\"$undefined\",\"unauthorized\":\"$undefined\"}],\"$L9\"]}]}]]}],{\"children\":[\"$La\",{\"children\":[\"$Lb\",{\"children\":[\"$Lc\",{},null,false,null]},null,false,\"$d\"]},null,false,\"$d\"]},null,false,null],\"$Le\",false]],\"m\":\"$undefined\",\"G\":[\"$f\",[\"$L10\"]],\"S\":false,\"h\":null,\"r\":\"$undefined\",\"s\":\"$undefined\",\"a\":\"$undefined\",\"l\":\"$undefined\",\"p\":\"$undefined\",\"d\":\"$undefined\",\"b\":\"YLlO2IYa2IdABLT5-iPaA\"}\n11:I[22016,[\"/_next/static/immutable/chunks/1re8l28ybwau4.js\",\"/_next/static/immutable/chunks/2z7u4ewi6d7kj.js\"],\"\"]\n12:I[5014,[\"/_next/static/immutable/chunks/1re8l28ybwau4.js\",\"/_next/static/immutable/chunks/2z7u4ewi6d7kj.js\"],\"default\"]\n14:I[97367,[\"/_next/static/immutable/chunks/1re8l28ybwau4.js\"],\"OutletBoundary\"]\n16:I[97367,[\"/_next/static/immutable/chunks/1re8l28ybwau4.js\"],\"ViewportBoundary\"]\n18:I[97367,[\"/_next/static/immutable/chunks/1re8l28ybwau4.js\"],\"MetadataBoundary\"]\n7:[\"$\",\"li\",\"The address has a typo in it.\",{\"className\":\"relative\",\"children\":[[\"$\",\"span\",null,{\"className\":\"tape tape-g\",\"style\":{\"top\":-7,\"left\":20,\"transform\":\"rotate(-3deg)\",\"width\":42,\"height\":13}}],[\"$\",\"div\",null,{\"className\":\"sk\",\"style\":{\"transform\":\"rotate(0.4deg)\"},\"children\":[[\"$\",\"div\",null,{\"className\":\"sk-b\",\"style\":{\"borderWidth\":\"1.2px\"}}],[\"$\",\"div\",null,{\"className\":\"sk-i px-4 py-3 text-[0.94rem]\",\"children\":\"The address has a typo in it.\"}]]}]]}]\n8:[\"$\",\"div\",null,{\"className\":\"flex flex-wrap justify-center gap-3\",\"children\":[[\"$\",\"$L11\",null,{\"href\":\"/quick\",\"className\":\"btn btn-ink text-[1.02rem]\",\"children\":[\"Write a new one \",[\"$\",\"$L12\",null,{\"ref\":\"$undefined\",\"iconNode\":[[\"path\",{\"d\":\"M5 12h14\",\"key\":\"1ays0h\"}],[\"path\",{\"d\":\"m12 5 7 7-7 7\",\"key\":\"xquz4c\"}]],\"className\":\"lucide-arrow-right\",\"size\":15}]]}],[\"$\",\"$L11\",null,{\"href\":\"/recover\",\"className\":\"btn text-[1.02rem]\",\"children\":[[\"$\",\"$L12\",null,{\"ref\":\"$undefined\",\"iconNode\":[[\"path\",{\"d\":\"M2.586 17.414A2 2 0 0 0 2 18.828V21a1 1 0 0 0 1 1h3a1 1 0 0 0 1-1v-1a1 1 0 0 1 1-1h1a1 1 0 0 0 1-1v-1a1 1 0 0 1 1-1h.172a2 2 0 0 0 1.414-.586l.814-.814a6.5 6.5 0 1 0-4-4z\",\"key\":\"1s6t7t\"}],[\"circle\",{\"cx\":\"16.5\",\"cy\":\"7.5\",\"r\":\".5\",\"fill\":\"currentColor\",\"key\":\"w0ekpg\"}]],\"className\":\"lucide-key-round\",\"size\":14}],\" I have an edit link\"]}]]}]\n9:[\"$\",\"script\",null,{\"type\":\"application/ld+json\",\"dangerouslySetInnerHTML\":{\"__html\":\"{\\\"@context\\\":\\\"https://schema.org\\\",\\\"@type\\\":\\\"WebApplication\\\",\\\"name\\\":\\\"SharePad\\\",\\\"url\\\":\\\"https://sharepad.in\\\",\\\"description\\\":\\\"Write a notebook of markdown pages and share it with a single link. Password lock, expiry dates, comments and PDF export. Free, and no account needed.\\\",\\\"applicationCategory\\\":\\\"ProductivityApplication\\\",\\\"operatingSystem\\\":\\\"Any\\\",\\\"browserRequirements\\\":\\\"Requires JavaScript\\\",\\\"offers\\\":{\\\"@type\\\":\\\"Offer\\\",\\\"price\\\":\\\"0\\\",\\\"priceCurrency\\\":\\\"USD\\\"},\\\"featureList\\\":[\\\"Multi-page markdown notebooks behind one link\\\",\\\"No account or sign-up\\\",\\\"Password protection and expiry dates\\\",\\\"Export to PDF or Markdown\\\",\\\"Anonymous comments\\\",\\\"Version history\\\"],\\\"author\\\":{\\\"@type\\\":\\\"Person\\\",\\\"name\\\":\\\"Varshith Hegde\\\"}}\"}}]\na:[\"$\",\"$1\",\"c\",{\"children\":[null,[\"$\",\"$L4\",null,{\"parallelRouterKey\":\"children\",\"error\":\"$undefined\",\"errorStyles\":\"$undefined\",\"errorScripts\":\"$undefined\",\"template\":[\"$\",\"$L6\",null,{}],\"templateStyles\":\"$undefined\",\"templateScripts\":\"$undefined\",\"notFound\":\"$undefined\",\"forbidden\":\"$undefined\",\"unauthorized\":\"$undefined\"}]]}]\nb:[\"$\",\"$1\",\"c\",{\"children\":[null,[\"$\",\"$L4\",null,{\"parallelRouterKey\":\"children\",\"error\":\"$undefined\",\"errorStyles\":\"$undefined\",\"errorScripts\":\"$undefined\",\"template\":[\"$\",\"$L6\",null,{}],\"templateStyles\":\"$undefined\",\"templateScripts\":\"$undefined\",\"notFound\":\"$undefined\",\"forbidden\":\"$undefined\",\"unauthorized\":\"$undefined\"}]]}]\nc:[\"$\",\"$1\",\"c\",{\"children\":[\"$L13\",[[\"$\",\"link\",\"0\",{\"rel\":\"stylesheet\",\"href\":\"/_next/static/immutable/chunks/0klya-7dk1nq1.css\",\"precedence\":\"next\",\"crossOrigin\":\"$undefined\",\"nonce\":\"$undefined\"}],[\"$\",\"script\",\"script-0\",{\"src\":\"/_next/static/immutable/chunks/2smipw1uu9qsx.js\",\"async\":true,\"nonce\":\"$undefined\"}],[\"$\",\"script\",\"script-1\",{\"src\":\"/_next/static/immutable/chunks/0r30p2j3i-crd.js\",\"async\":true,\"nonce\":\"$undefined\"}],[\"$\",\"script\",\"script-2\",{\"src\":\"/_next/static/immutable/chunks/376jf3zrtq-9q.js\",\"async\":true,\"nonce\":\"$undefined\"}],[\"$\",\"script\",\"script-3\",{\"src\":\"/_next/static/immutable/chunks/15hlxfei78flc.js\",\"async\":true,\"nonce\":\"$undefined\"}]],[\"$\",\"$L14\",null,{\"children\":[\"$\",\"$2\",null,{\"name\":\"Next.MetadataOutlet\",\"children\":\"$@15\"}]}]]}]\ne:[\"$\",\"$1\",\"h\",{\"children\":[null,[\"$\",\"$L16\",null,{\"children\":\"$L17\"}],[\"$\",\"div\",null,{\"hidden\":true,\"children\":[\"$\",\"$L18\",null,{\"children\":[\"$\",\"$2\",null,{\"name\":\"Next.Metadata\",\"children\":\"$L19\"}]}]}],[\"$\",\"meta\",null,{\"name\":\"next-size-adjust\",\"content\":\"\"}]]}]\n10:[\"$\",\"link\",\"0\",{\"rel\":\"stylesheet\",\"href\":\"/_next/static/immutable/chunks/0_a4irsykw_52.css\",\"precedence\":\"next\",\"crossOrigin\":\"$undefined\",\"nonce\":\"$undefined\"}]\nd:C\n17:[[\"$\",\"meta\",\"0\",{\"charSet\":\"utf-8\"}],[\"$\",\"meta\",\"1\",{\"name\":\"viewport\",\"content\":\"width=device-width, initial-scale=1\"}]]\n1a:I[27201,[\"/_next/static/immutable/chunks/1re8l28ybwau4.js\"],\"IconMark\"]\n15:null\n19:[[\"$\",\"title\",\"0\",{\"children\":\"Okr Semanales — SharePad\"}],[\"$\",\"meta\",\"1\",{\"name\":\"description\",\"content\":\"A shared notebook: Okr Semanales\"}],[\"$\",\"meta\",\"2\",{\"name\":\"application-name\",\"content\":\"SharePad\"}],[\"$\",\"link\",\"3\",{\"rel\":\"author\",\"href\":\"https://github.com/Varshithvhegde\"}],[\"$\",\"meta\",\"4\",{\"name\":\"author\",\"content\":\"Varshith Hegde\"}],[\"$\",\"meta\",\"5\",{\"name\":\"keywords\",\"content\":\"share notes,markdown notebook,no signup notepad,share markdown link,online notepad,paste and share text,temporary notes,markdown to pdf\"}],[\"$\",\"meta\",\"6\",{\"name\":\"creator\",\"content\":\"Varshith Hegde\"}],[\"$\",\"meta\",\"7\",{\"name\":\"robots\",\"content\":\"noindex, nofollow\"}],[\"$\",\"meta\",\"8\",{\"name\":\"category\",\"content\":\"productivity\"}],[\"$\",\"link\",\"9\",{\"rel\":\"canonical\",\"href\":\"https://sharepad.in/n/okr-semanales\"}],[\"$\",\"meta\",\"10\",{\"property\":\"og:title\",\"content\":\"Okr Semanales\"}],[\"$\",\"meta\",\"11\",{\"property\":\"og:description\",\"content\":\"A shared notebook: Okr Semanales\"}],[\"$\",\"meta\",\"12\",{\"property\":\"og:url\",\"content\":\"https://sharepad.in/n/okr-semanales\"}],[\"$\",\"meta\",\"13\",{\"property\":\"og:type\",\"content\":\"article\"}],[\"$\",\"meta\",\"14\",{\"name\":\"twitter:card\",\"content\":\"summary_large_image\"}],[\"$\",\"meta\",\"15\",{\"name\":\"twitter:title\",\"content\":\"Okr Semanales\"}],[\"$\",\"meta\",\"16\",{\"name\":\"twitter:description\",\"content\":\"A shared notebook: Okr Semanales\"}],[\"$\",\"link\",\"17\",{\"rel\":\"icon\",\"href\":\"/favicon.ico?favicon.261-8iwo0k35b.ico\",\"sizes\":\"256x256\",\"type\":\"image/x-icon\"}],[\"$\",\"link\",\"18\",{\"rel\":\"icon\",\"href\":\"/favicon.ico\",\"sizes\":\"48x48\"}],[\"$\",\"link\",\"19\",{\"rel\":\"icon\",\"href\":\"/icon.svg\",\"type\":\"image/svg+xml\"}],[\"$\",\"link\",\"20\",{\"rel\":\"apple-touch-icon\",\"href\":\"/apple-icon.png\"}],[\"$\",\"$L1a\",\"21\",{}]]\n1b:I[24304,[\"/_next/static/immutable/chunks/1re8l28ybwau4.js\",\"/_next/static/immutable/chunks/2smipw1uu9qsx.js\",\"/_next/static/immutable/chunks/0r30p2j3i-crd.js\",\"/_next/static/immutable/chunks/376jf3zrtq-9q.js\",\"/_next/static/immutable/chunks/15hlxfei78flc.js\"],\"default\"]\n13:[\"$\",\"$L1b\",null,{\"notebook\":{\"id\":\"cdf9c65d-6c6d-4b8b-827f-6f7cbecee231\",\"slug\":\"okr-semanales\",\"title\":\"Okr Semanales\",\"description\":null,\"emoji\":\"notebook\",\"theme\":\"plain\",\"visibility\":\"unlisted\",\"read_only\":false,\"burn_after_read\":false,\"burn_consumed\":false,\"expires_at\":null,\"view_count\":55,\"allow_comments\":true,\"created_at\":\"2026-08-29T04:20:30.857196+00:00\",\"updated_at\":\"2026-08-31T15:25:44.608551+00:00\",\"allow_public_edit\":false,\"font\":\"mono\",\"has_password\":false},\"pages\":[{\"id\":\"e9303419-ef0f-442b-84c3-5c7066c8cae3\",\"notebook_id\":\"cdf9c65d-6c6d-4b8b-827f-6f7cbecee231\",\"slug\":\"ok\",\"title\":\"Objetivos semanales\",\"content\":\"# Objetivos semanales\\n## Demo CCL\\n- [ ] Tarea 1 xd\\n## Presentacion prototipos\\n- [ ] Tarea 2? xd\\n## Validar CCL\\n- [ ] Pos la 3\",\"icon\":\"file\",\"sort_order\":0,\"pinned\":false,\"created_at\":\"2026-08-29T04:20:31.134911+00:00\",\"updated_at\":\"2026-08-31T14:32:49.663438+00:00\"}],\"editToken\":\"\",\"mode\":\"view\"}]\n"])</script></body></html>"##
            .to_string()
    }

    #[test]
    fn extract_markdown_from_rsc_payload_reads_pages_content_from_real_capture() {
        let markdown = extract_markdown_from_rsc_payload(&real_sharepad_fixture_html())
            .expect("debería extraer el markdown del payload RSC real de sharepad.in");

        assert!(markdown.contains("# Objetivos semanales"));
        assert!(markdown.contains("## Demo CCL"));
        assert!(markdown.contains("- [ ] Tarea 1 xd"));
        assert!(markdown.contains("## Presentacion prototipos"));
        assert!(markdown.contains("- [ ] Tarea 2? xd"));
        assert!(markdown.contains("## Validar CCL"));
        assert!(markdown.contains("- [ ] Pos la 3"));
        // La misma captura trae, en el mismo chunk RSC, varios `<meta
        // content="...">` de SEO. Si la extracción no se acota al array
        // "pages", esta basura terminaría concatenada antes del markdown
        // real (bug confirmado y corregido en esta ronda de review).
        assert!(!markdown.contains("width=device-width"));
        assert!(!markdown.contains("A shared notebook"));
    }

    #[test]
    fn extract_markdown_from_rendered_html_rebuilds_synthetic_markdown_from_real_capture() {
        let markdown = extract_markdown_from_rendered_html(&real_sharepad_fixture_html())
            .expect("debería reconstruir el markdown desde el HTML renderizado real");

        assert!(markdown.contains("# Objetivos semanales"));
        assert!(markdown.contains("## Demo CCL"));
        assert!(markdown.contains("- [ ] Tarea 1 xd"));
        assert!(markdown.contains("## Presentacion prototipos"));
        assert!(markdown.contains("- [ ] Tarea 2? xd"));
        assert!(markdown.contains("## Validar CCL"));
        assert!(markdown.contains("- [ ] Pos la 3"));
    }

    #[tokio::test]
    async fn fetch_weekly_objectives_markdown_parses_real_sharepad_capture() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/n/okr-semanales")
            .with_status(200)
            .with_body(real_sharepad_fixture_html())
            .create_async()
            .await;

        let url = format!("{}/n/okr-semanales", server.url());
        let markdown = fetch_weekly_objectives_markdown(&url)
            .await
            .expect("debería obtener y parsear el markdown real de sharepad.in");

        assert!(markdown.contains("# Objetivos semanales"));
        assert!(markdown.contains("## Demo CCL"));
        assert!(markdown.contains("- [ ] Tarea 1 xd"));
        assert!(markdown.contains("## Presentacion prototipos"));
        assert!(markdown.contains("- [ ] Tarea 2? xd"));
        assert!(markdown.contains("## Validar CCL"));
        assert!(markdown.contains("- [ ] Pos la 3"));
    }
}
