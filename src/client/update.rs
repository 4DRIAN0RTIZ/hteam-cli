//! Cliente HTTP standalone para la API de GitHub Releases, usado por
//! `operations::update`. A diferencia de `HteamClient`, no lleva cookies ni
//! headers de autenticación de hteam.mx: consulta releases de este mismo
//! repo (`Ditr4drian/hteam-cli`) y, cuando corresponde, descarga el asset
//! binario publicado por `.github/workflows/release.yml`.
//!
//! El repo es **privado** (verificado con `gh repo view` — la API de GitHub
//! devuelve 404 tanto para el repo como para sus releases sin
//! autenticación), así que toda request lleva `Authorization: Bearer` con
//! el token que devuelva `gh auth token` cuando esté disponible.

use anyhow::{Context, Result};
use reqwest::StatusCode;
use serde::Deserialize;
use std::process::Command;
use std::time::Duration;

/// Nombre del asset que el pipeline de release publica para Linux x86_64
/// (ver `.github/workflows/release.yml`, step "Package binary"). `hteam
/// update` sólo sabe instalar este asset — si la plataforma actual no es
/// linux-x86_64, falla con un error claro en vez de asumir que existe.
pub const RELEASE_ASSET_NAME: &str = "hteam-linux-x86_64";

/// Base de la API de GitHub para este repo. Se pasa como parámetro (no se
/// hardcodea dentro de las funciones) para poder apuntar a un servidor
/// mockeado en tests, mismo patrón que `source_url` en
/// `client::objectives::fetch_weekly_objectives_markdown`.
pub const GITHUB_API_BASE: &str = "https://api.github.com/repos/Ditr4drian/hteam-cli";

#[derive(Debug, Clone, Deserialize)]
pub struct GithubReleaseAsset {
    pub name: String,
    /// Endpoint de la API del asset (`/releases/assets/{id}`) — **no** el
    /// `browser_download_url` público que ve un humano en la UI de GitHub.
    /// Verificado contra la release real de este repo (privado): pedir
    /// `browser_download_url` con `Authorization: Bearer` devuelve 404 (ese
    /// link espera una cookie de sesión de browser, no un token de API);
    /// `url` con `Accept: application/octet-stream` + el mismo token sí
    /// sirve el binario (200, tamaño y contenido coinciden con el asset
    /// real).
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GithubRelease {
    pub tag_name: String,
    #[serde(default)]
    pub assets: Vec<GithubReleaseAsset>,
}

impl GithubRelease {
    /// Versión sin el prefijo `v` del tag (ej. `v0.7.0` -> `0.7.0`), mismo
    /// formato que `env!("CARGO_PKG_VERSION")` y las secciones de
    /// `CHANGELOG.md`.
    pub fn version(&self) -> &str {
        self.tag_name.strip_prefix('v').unwrap_or(&self.tag_name)
    }

    /// URL de la API para descargar el asset de linux-x86_64, si la release
    /// lo publicó (ver doc-comment de `GithubReleaseAsset::url`).
    pub fn asset_download_url(&self) -> Option<&str> {
        self.assets
            .iter()
            .find(|asset| asset.name == RELEASE_ASSET_NAME)
            .map(|asset| asset.url.as_str())
    }
}

fn http_client(timeout: Duration) -> Result<reqwest::Client> {
    reqwest::Client::builder()
        .user_agent(concat!("hteam-cli/", env!("CARGO_PKG_VERSION")))
        .timeout(timeout)
        .build()
        .context("Error al crear el cliente HTTP para GitHub")
}

/// Token de autenticación para la API de GitHub, obtenido del `gh` CLI ya
/// logueado en esta máquina (`gh auth token`) — el repo es privado, así que
/// sin esto las requests devuelven 404. No es un unit test candidate (es un
/// wrapper directo sobre un proceso externo y credenciales reales del
/// entorno, mismo motivo por el que `Config::load/save` tampoco se testean
/// contra el filesystem real, ver `docs/verification.md`): si `gh` no está
/// instalado, no está logueado, o falla por cualquier motivo, devuelve
/// `None` sin panickear — los callers hacen la request sin auth, que
/// fallará con un error claro (nunca un panic).
fn gh_auth_token() -> Option<String> {
    let output = Command::new("gh").args(["auth", "token"]).output().ok()?;
    if !output.status.success() {
        return None;
    }
    let token = String::from_utf8(output.stdout).ok()?;
    let token = token.trim();
    if token.is_empty() {
        None
    } else {
        Some(token.to_string())
    }
}

/// Sugerencia agregada al error cuando la falla pinta a "repo privado sin
/// autenticar" (401/403/404 sin haber mandado token).
fn auth_hint(token: &Option<String>, status: StatusCode) -> &'static str {
    if token.is_none() && matches!(status.as_u16(), 401 | 403 | 404) {
        " (¿hteam-cli es un repo privado? corré 'gh auth login' para autenticar)"
    } else {
        ""
    }
}

/// `GET {api_base}/releases/latest` — devuelve la última release publicada.
pub async fn fetch_latest_release(api_base: &str) -> Result<GithubRelease> {
    let url = format!("{}/releases/latest", api_base);
    let client = http_client(Duration::from_secs(10))?;
    let token = gh_auth_token();

    let mut request = client.get(&url);
    if let Some(token) = &token {
        request = request.bearer_auth(token);
    }

    let response = request
        .send()
        .await
        .with_context(|| format!("No se pudo conectar a {}", url))?;

    if !response.status().is_success() {
        anyhow::bail!(
            "GitHub respondió con error HTTP {} al pedir {}{}",
            response.status(),
            url,
            auth_hint(&token, response.status())
        );
    }

    response
        .json::<GithubRelease>()
        .await
        .context("No se pudo parsear la respuesta de GitHub releases/latest")
}

/// Descarga los bytes crudos del asset en `url` (binario de la release).
pub async fn download_asset(url: &str) -> Result<Vec<u8>> {
    let client = http_client(Duration::from_secs(120))?;
    let token = gh_auth_token();

    let mut request = client
        .get(url)
        .header(reqwest::header::ACCEPT, "application/octet-stream");
    if let Some(token) = &token {
        request = request.bearer_auth(token);
    }

    let response = request
        .send()
        .await
        .with_context(|| format!("No se pudo descargar {}", url))?;

    if !response.status().is_success() {
        anyhow::bail!(
            "Error HTTP {} al descargar {}{}",
            response.status(),
            url,
            auth_hint(&token, response.status())
        );
    }

    let bytes = response
        .bytes()
        .await
        .with_context(|| format!("No se pudo leer el binario descargado de {}", url))?;

    Ok(bytes.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release_json(tag: &str, assets: &str) -> String {
        format!(r#"{{"tag_name":"{}","assets":{}}}"#, tag, assets)
    }

    #[test]
    fn version_strips_v_prefix() {
        let release = GithubRelease {
            tag_name: "v0.7.0".to_string(),
            assets: vec![],
        };

        assert_eq!(release.version(), "0.7.0");
    }

    #[test]
    fn version_returns_tag_as_is_without_v_prefix() {
        let release = GithubRelease {
            tag_name: "0.7.0".to_string(),
            assets: vec![],
        };

        assert_eq!(release.version(), "0.7.0");
    }

    #[test]
    fn asset_download_url_finds_linux_asset() {
        let release = GithubRelease {
            tag_name: "v0.7.0".to_string(),
            assets: vec![
                GithubReleaseAsset {
                    name: "hteam-macos-arm64".to_string(),
                    url: "https://example.com/macos".to_string(),
                },
                GithubReleaseAsset {
                    name: RELEASE_ASSET_NAME.to_string(),
                    url: "https://example.com/linux".to_string(),
                },
            ],
        };

        assert_eq!(
            release.asset_download_url(),
            Some("https://example.com/linux")
        );
    }

    #[test]
    fn asset_download_url_returns_none_without_a_matching_asset() {
        let release = GithubRelease {
            tag_name: "v0.7.0".to_string(),
            assets: vec![GithubReleaseAsset {
                name: "hteam-macos-arm64".to_string(),
                url: "https://example.com/macos".to_string(),
            }],
        };

        assert_eq!(release.asset_download_url(), None);
    }

    #[tokio::test]
    async fn fetch_latest_release_parses_tag_and_assets() {
        let mut server = mockito::Server::new_async().await;
        let body = release_json(
            "v0.7.0",
            r#"[{"name":"hteam-linux-x86_64","url":"https://example.com/hteam-linux-x86_64"}]"#,
        );
        let _m = server
            .mock("GET", "/releases/latest")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(body)
            .create_async()
            .await;

        let release = fetch_latest_release(&server.url())
            .await
            .expect("debería parsear la release");

        assert_eq!(release.version(), "0.7.0");
        assert_eq!(
            release.asset_download_url(),
            Some("https://example.com/hteam-linux-x86_64")
        );
    }

    #[tokio::test]
    async fn fetch_latest_release_fails_on_http_error() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/releases/latest")
            .with_status(404)
            .create_async()
            .await;

        let err = fetch_latest_release(&server.url())
            .await
            .expect_err("debería fallar con 404");

        assert!(err.to_string().contains("404"));
    }

    #[tokio::test]
    async fn fetch_latest_release_fails_on_invalid_json() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/releases/latest")
            .with_status(200)
            .with_body("no es json")
            .create_async()
            .await;

        let err = fetch_latest_release(&server.url())
            .await
            .expect_err("debería fallar con JSON inválido");

        assert!(err.to_string().contains("No se pudo parsear la respuesta"));
    }

    #[tokio::test]
    async fn download_asset_returns_bytes() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/hteam-linux-x86_64")
            .with_status(200)
            .with_body(vec![0x7f, b'E', b'L', b'F'])
            .create_async()
            .await;

        let url = format!("{}/hteam-linux-x86_64", server.url());
        let bytes = download_asset(&url)
            .await
            .expect("debería descargar el binario");

        assert_eq!(bytes, vec![0x7f, b'E', b'L', b'F']);
    }

    #[tokio::test]
    async fn download_asset_fails_on_http_error() {
        let mut server = mockito::Server::new_async().await;
        let _m = server
            .mock("GET", "/missing")
            .with_status(404)
            .create_async()
            .await;

        let url = format!("{}/missing", server.url());
        let err = download_asset(&url)
            .await
            .expect_err("debería fallar con 404");

        assert!(err.to_string().contains("404"));
    }
}
