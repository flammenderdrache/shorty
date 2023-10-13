use std::collections::HashMap;
use actix_files::NamedFile;
use actix_web::{get, HttpRequest, HttpResponse, post, Responder, web};
use static_files::Resource;
use tracing::{debug, info, warn};

use crate::CONFIG;
use crate::error::ShortyError;
use crate::LinkConfig;
use crate::LinkStore;
use crate::util::uri_to_url;

// The function is async because the actix-web macro requires it.
#[allow(clippy::unused_async)]
#[get("/")]
pub async fn index(req: HttpRequest) -> Result<impl Responder, Box<dyn std::error::Error>> {
	debug!("Got request for Index");
	if let Some(ref path) = CONFIG.frontend_location {
		let path = format!("{path}/index.html");
		return Ok(NamedFile::open(path)?.into_response(&req));
	}

	let response = get_embedded_file("index.html").unwrap();
	Ok(
		HttpResponse::Ok()
			.content_type(response.0)
			.body(response.1)
	)
}

#[get("/{shortened_url:.*}")]
async fn get_shortened(
	params: web::Path<String>,
	link_store: web::Data<LinkStore>,
) -> Result<impl Responder, ShortyError> {
	let shortened_url = params.into_inner();
	debug!("Got request for {shortened_url}");


	if let Some(link) = link_store.get(shortened_url.as_str()).await {
		info!("Return url for {shortened_url} is {link}");
		Ok(
			HttpResponse::TemporaryRedirect()
				.append_header(("Location", link.redirect_to.as_str()))
				.finish()
		)
	} else {
		Ok(HttpResponse::NotFound().finish())
	}
}

// The function is async because the actix-web macro requires it.
#[allow(clippy::unused_async)]
#[get("/config")]
async fn get_config() -> impl Responder {
	HttpResponse::Ok()
		.content_type("application/json; charset=utf-8")
		.body(CONFIG.json_string())
}

// The function is async because the actix-web macro requires it.
#[allow(clippy::unused_async)]
#[get("/documentation")]
pub async fn api_docs() -> impl Responder {
	const DOCUMENTATION_YAML: &str = include_str!("../../meta/docs/api.yaml");

	HttpResponse::Ok()
		.content_type("text/x-yaml")
		.body(DOCUMENTATION_YAML)
}

/// Creates a shortened link by taking the requested uri and turning it into a shortened link.
#[post("/{url:.*}")]
#[allow(clippy::similar_names)]
async fn create_shortened(
	req: HttpRequest,
	link_store: web::Data<LinkStore>,
) -> Result<impl Responder, ShortyError> {
	let uri = req.uri();
	debug!("URI is {uri}");
	let url = uri_to_url(uri);

	let link = link_store.create_link(url).await?;
	let formatted = link.formatted();
	info!("Shortening URL {} to {}", link.redirect_to, formatted);


	Ok(
		HttpResponse::Ok()
			.content_type("text/plain; charset=utf-8")
			.body(formatted)
	)
}

/// Custom shortened URL, configured via Json.
/// Also see [`LinkConfig`].
#[post("/custom")]
async fn create_shortened_custom(
	link_store: web::Data<LinkStore>,
	link_config: web::Json<LinkConfig>,
) -> Result<impl Responder, ShortyError> {
	let link_config = link_config.into_inner();

	let link = link_store.create_link_with_config(link_config).await?;
	let formatted = link.formatted();
	info!("Shortening URL {} to {}", link.redirect_to, formatted);


	Ok(
		HttpResponse::Ok()
			.content_type("text/plain; charset=utf-8")
			.body(formatted)
	)
}

#[allow(clippy::unused_async)]
#[get("/favicon.ico")]
async fn get_favicon() -> Result<impl Responder, ShortyError> {
	debug!("Got request for favicon");
	Ok(HttpResponse::NotFound().finish())
}

// The function is async because the actix-web macro requires it.
#[allow(clippy::unused_async)]
#[get("/assets/{asset:.*}")]
pub async fn serve_file(asset: web::Path<String>, req: HttpRequest) -> Result<impl Responder, Box<dyn std::error::Error>> {
	let asset = asset.into_inner();
	debug!("Got request for file: {asset}");

	if let Some(ref path) = CONFIG.frontend_location {
		let path = format!("{path}/{asset}");
		return Ok(NamedFile::open(path)?.into_response(&req));
	}

	// Tuple of MIME Type and Content.
	let response_opt: Option<(&str, &[u8])> = get_embedded_file(asset.as_str());


	if let Some(response) = response_opt {
		Ok(
			HttpResponse::Ok()
				.content_type(response.0)
				.body(response.1)
		)
	} else {
		Ok(HttpResponse::NotFound().finish())
	}
}

include!(concat!(env!("OUT_DIR"), "/generated.rs"));

/// Returns a Tuple of Mime Type (as &str) and file content (as &[u8]).
fn get_embedded_file(file: &str) -> Option<(&'static str, &'static [u8])> {
	let resources: HashMap<&str, Resource> = generate();

	debug!("Getting embedded file: {file}");

	resources.get(file).map(|file| {
		(file.mime_type, file.data)
	}).or_else(|| {
		warn!("Got request for {file} but couldn't find embedded asset.");
		None
	})
}

